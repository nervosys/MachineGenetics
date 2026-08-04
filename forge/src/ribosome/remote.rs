//! Remote execution over TCP.
//!
//! This is the transport under the [`Executor`] seam. A worker process advertises
//! a platform and a tool set; a [`RemoteExecutor`] ships actions to it and gets
//! results back. To the scheduler it is indistinguishable from a local worker,
//! which was the point of making the seam narrow.
//!
//! ## Why hand-rolled and synchronous
//!
//! One thread per connection over `std::net`, with a length-prefixed JSON frame.
//! No async runtime, no HTTP, no dependencies.
//!
//! That is a deliberate trade rather than a shortcut. Build actions are coarse —
//! milliseconds to minutes each — so a fleet is hundreds of concurrent
//! connections, not hundreds of thousands. Thread-per-connection is entirely
//! adequate at that scale, and it keeps this crate free of an async runtime and
//! its transitive tree. If the fleet ever outgrows it, the thing to replace is
//! this file; nothing above [`Executor`] would change.
//!
//! **Wire format.** A 4-byte big-endian length followed by JSON. Payload bytes
//! are hex-encoded inside the JSON, which doubles them on the wire — acceptable
//! for source and IR (kilobytes), wasteful for large model artifacts. A framed
//! binary side-channel is the fix and is a contained change to
//! [`Frame`](self::Frame); it is noted here rather than pre-built because the
//! current payloads do not justify it.
//!
//! ## What this does not do
//!
//! No TLS, and no authentication of the *connection* — a worker will execute
//! what it is told. That is safe on a trusted network segment and unsafe on an
//! open one, which is the same posture as an unauthenticated build cache and
//! should be deployed the same way. What *is* authenticated is the result:
//! workers sign their outputs ([`provenance`](super::provenance)) so a claim is
//! attributable even though the channel is not private.

use super::exec::{ExecError, Executor, Inputs, ToolOutput};
use super::provenance::{Provenance, Signer};
use super::{Action, Platform};
use crate::mac::{hex_decode, hex_encode};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// One message on the wire.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "frame", rename_all = "snake_case")]
pub enum Frame {
    /// Client asks what the worker is.
    Describe,
    /// Worker's advertisement.
    Advertisement { worker: String, platform: Platform, tools: Vec<String> },
    /// Client asks for an action to be run. Inputs are hex-encoded.
    Execute { action: Box<Action>, inputs: BTreeMap<String, String> },
    /// Worker's result. Outputs are hex-encoded.
    Completed {
        outputs: BTreeMap<String, String>,
        stderr: String,
        provenance: Option<Provenance>,
    },
    /// Worker could not run it. `transient` decides whether the healer retries.
    Failed { message: String, transient: bool },
    /// Liveness probe.
    Ping,
    Pong,
}

fn write_frame(stream: &mut TcpStream, frame: &Frame) -> std::io::Result<()> {
    let body = serde_json::to_vec(frame)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    stream.write_all(&(body.len() as u32).to_be_bytes())?;
    stream.write_all(&body)?;
    stream.flush()
}

/// Frames larger than this are refused rather than allocated.
///
/// A length prefix read from the network is attacker-controlled: without a cap,
/// `0xFFFFFFFF` is a 4 GB allocation request from one packet.
const MAX_FRAME: u32 = 256 * 1024 * 1024;

fn read_frame(stream: &mut TcpStream) -> std::io::Result<Frame> {
    let mut len = [0u8; 4];
    stream.read_exact(&mut len)?;
    let len = u32::from_be_bytes(len);
    if len > MAX_FRAME {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("frame of {len} bytes exceeds the {MAX_FRAME}-byte cap"),
        ));
    }
    let mut body = vec![0u8; len as usize];
    stream.read_exact(&mut body)?;
    serde_json::from_slice(&body)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

fn encode(inputs: &Inputs) -> BTreeMap<String, String> {
    inputs.iter().map(|(k, v)| (k.clone(), hex_encode(v))).collect()
}

fn decode(map: &BTreeMap<String, String>) -> Result<Inputs, ExecError> {
    map.iter()
        .map(|(k, v)| {
            hex_decode(v)
                .map(|b| (k.clone(), b))
                .map_err(|_| ExecError::Transient(format!("malformed payload for `{k}`")))
        })
        .collect()
}

/// A worker process: serves actions to whoever connects.
pub struct WorkerServer {
    inner: Arc<dyn Executor>,
    signer: Option<Arc<Signer>>,
    shutdown: Arc<AtomicBool>,
}

impl WorkerServer {
    pub fn new(inner: Arc<dyn Executor>) -> Self {
        WorkerServer { inner, signer: None, shutdown: Arc::new(AtomicBool::new(false)) }
    }

    /// Sign results, so cache entries this worker produces are attributable.
    pub fn with_signer(mut self, signer: Arc<Signer>) -> Self {
        self.signer = Some(signer);
        self
    }

    /// A handle that stops the accept loop.
    pub fn shutdown_handle(&self) -> Arc<AtomicBool> {
        self.shutdown.clone()
    }

    /// Serve until shut down. Blocks; callers run it on its own thread.
    pub fn serve(&self, listener: TcpListener) -> std::io::Result<()> {
        // A short accept timeout so shutdown is observed promptly rather than
        // blocking until the next client happens to connect.
        listener.set_nonblocking(true)?;
        while !self.shutdown.load(Ordering::Relaxed) {
            match listener.accept() {
                Ok((stream, _)) => {
                    stream.set_nonblocking(false)?;
                    let inner = self.inner.clone();
                    let signer = self.signer.clone();
                    std::thread::spawn(move || {
                        let _ = Self::handle(stream, inner, signer);
                    });
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    fn handle(
        mut stream: TcpStream,
        inner: Arc<dyn Executor>,
        signer: Option<Arc<Signer>>,
    ) -> std::io::Result<()> {
        loop {
            let frame = match read_frame(&mut stream) {
                Ok(f) => f,
                // A closed connection is the normal end of a session.
                Err(_) => return Ok(()),
            };
            let reply = match frame {
                Frame::Ping => Frame::Pong,
                Frame::Describe => Frame::Advertisement {
                    worker: inner.name().to_string(),
                    platform: inner.platform().clone(),
                    tools: Vec::new(),
                },
                Frame::Execute { action, inputs } => match decode(&inputs) {
                    Err(e) => Frame::Failed { message: e.to_string(), transient: e.is_transient() },
                    Ok(inputs) => match inner.run(&action, &inputs) {
                        Ok(out) => {
                            let provenance = signer.as_ref().map(|s| {
                                let mut result = super::cas::ActionResult::ok(inner.name());
                                for (path, bytes) in &out.outputs {
                                    result.outputs.insert(path.clone(), super::Digest::of(bytes));
                                }
                                s.sign(&action.key(), &result)
                            });
                            Frame::Completed {
                                outputs: out
                                    .outputs
                                    .iter()
                                    .map(|(k, v)| (k.clone(), hex_encode(v)))
                                    .collect(),
                                stderr: out.stderr,
                                provenance,
                            }
                        }
                        Err(e) => {
                            Frame::Failed { message: e.to_string(), transient: e.is_transient() }
                        }
                    },
                },
                other => Frame::Failed {
                    message: format!("unexpected frame from client: {other:?}"),
                    transient: false,
                },
            };
            write_frame(&mut stream, &reply)?;
        }
    }
}

/// A worker reached over the network. Implements [`Executor`], so the scheduler
/// cannot tell it apart from a local one.
#[derive(Debug)]
pub struct RemoteExecutor {
    name: String,
    addr: String,
    platform: Platform,
    timeout: Duration,
}

impl RemoteExecutor {
    /// Connect and ask the worker what it is.
    ///
    /// Querying rather than being told keeps the fleet's view of a worker's
    /// capabilities sourced from the worker itself; a hand-maintained roster
    /// drifts, and a drifted roster routes GPU work to a machine without one.
    pub fn connect(addr: impl Into<String>, timeout: Duration) -> Result<Self, ExecError> {
        let addr = addr.into();
        let mut stream = Self::dial(&addr, timeout)?;
        write_frame(&mut stream, &Frame::Describe)
            .map_err(|e| ExecError::Transient(format!("describe failed: {e}")))?;
        match read_frame(&mut stream) {
            Ok(Frame::Advertisement { worker, platform, .. }) => {
                Ok(RemoteExecutor { name: worker, addr, platform, timeout })
            }
            Ok(other) => Err(ExecError::Transient(format!("unexpected reply: {other:?}"))),
            Err(e) => Err(ExecError::Transient(format!("no advertisement: {e}"))),
        }
    }

    fn dial(addr: &str, timeout: Duration) -> Result<TcpStream, ExecError> {
        let sock = addr
            .to_socket_addrs()
            .map_err(|e| ExecError::Transient(format!("bad address `{addr}`: {e}")))?
            .next()
            .ok_or_else(|| ExecError::Transient(format!("address `{addr}` resolved to nothing")))?;
        let stream = TcpStream::connect_timeout(&sock, timeout)
            .map_err(|e| ExecError::Transient(format!("connect to {addr}: {e}")))?;
        // Both directions time out: a worker that accepts and then goes quiet
        // must not hang a build thread forever.
        stream
            .set_read_timeout(Some(timeout))
            .and_then(|_| stream.set_write_timeout(Some(timeout)))
            .map_err(|e| ExecError::Transient(e.to_string()))?;
        Ok(stream)
    }

    pub fn addr(&self) -> &str {
        &self.addr
    }

    /// Round-trip liveness check.
    pub fn ping(&self) -> bool {
        let Ok(mut s) = Self::dial(&self.addr, self.timeout) else { return false };
        write_frame(&mut s, &Frame::Ping).is_ok() && matches!(read_frame(&mut s), Ok(Frame::Pong))
    }
}

impl Executor for RemoteExecutor {
    fn name(&self) -> &str {
        &self.name
    }

    fn platform(&self) -> &Platform {
        &self.platform
    }

    fn run(&self, action: &Action, inputs: &Inputs) -> Result<ToolOutput, ExecError> {
        let mut stream = Self::dial(&self.addr, self.timeout)?;
        write_frame(
            &mut stream,
            &Frame::Execute { action: Box::new(action.clone()), inputs: encode(inputs) },
        )
        .map_err(|e| ExecError::Transient(format!("send failed: {e}")))?;

        match read_frame(&mut stream) {
            Ok(Frame::Completed { outputs, stderr, .. }) => {
                let decoded = decode(&outputs)?;
                Ok(ToolOutput { outputs: decoded.into_iter().collect(), stderr })
            }
            // A remote failure keeps its transient/deterministic classification:
            // losing it across the wire would make the healer retry compile
            // errors and give up on network blips.
            Ok(Frame::Failed { message, transient: true }) => Err(ExecError::Transient(message)),
            Ok(Frame::Failed { message, transient: false }) => {
                Err(ExecError::Deterministic { exit_code: 1, stderr: message })
            }
            Ok(other) => Err(ExecError::Transient(format!("unexpected reply: {other:?}"))),
            // A dropped connection is infrastructure, not a build error.
            Err(e) => Err(ExecError::Transient(format!("no reply from {}: {e}", self.addr))),
        }
    }
}

/// Liveness state for one registered worker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerRecord {
    pub name: String,
    pub addr: String,
    pub platform: Platform,
    pub healthy: bool,
    pub consecutive_failures: u32,
}

/// Tracks which workers are usable.
///
/// Separate from [`PoolExecutor`](super::exec::PoolExecutor) because liveness and
/// routing change on different timescales: routing decides per action, liveness
/// changes per heartbeat. Conflating them means a dead worker keeps being chosen
/// until something notices mid-dispatch.
#[derive(Debug, Default)]
pub struct WorkerRegistry {
    workers: Vec<WorkerRecord>,
    /// Failures tolerated before a worker is marked unhealthy. Not 1: a single
    /// missed heartbeat is usually a hiccup, and evicting on it makes the fleet
    /// flap.
    pub failure_threshold: u32,
}

impl WorkerRegistry {
    pub fn new() -> Self {
        WorkerRegistry { workers: Vec::new(), failure_threshold: 3 }
    }

    pub fn register(&mut self, name: &str, addr: &str, platform: Platform) {
        if let Some(w) = self.workers.iter_mut().find(|w| w.addr == addr) {
            // Re-registration is how a restarted worker rejoins; treat it as
            // recovery rather than as a duplicate.
            w.name = name.to_string();
            w.platform = platform;
            w.healthy = true;
            w.consecutive_failures = 0;
            return;
        }
        self.workers.push(WorkerRecord {
            name: name.to_string(),
            addr: addr.to_string(),
            platform,
            healthy: true,
            consecutive_failures: 0,
        });
    }

    /// Record the outcome of a heartbeat.
    pub fn heartbeat(&mut self, addr: &str, alive: bool) {
        if let Some(w) = self.workers.iter_mut().find(|w| w.addr == addr) {
            if alive {
                w.consecutive_failures = 0;
                w.healthy = true;
            } else {
                w.consecutive_failures += 1;
                if w.consecutive_failures >= self.failure_threshold {
                    w.healthy = false;
                }
            }
        }
    }

    pub fn healthy(&self) -> Vec<&WorkerRecord> {
        self.workers.iter().filter(|w| w.healthy).collect()
    }

    pub fn all(&self) -> &[WorkerRecord] {
        &self.workers
    }

    /// Healthy workers that could take this action.
    pub fn capable_for(&self, action: &Action) -> Vec<&WorkerRecord> {
        self.workers
            .iter()
            .filter(|w| w.healthy && w.platform.satisfies(&action.platform))
            .collect()
    }

    /// Probe every worker and update health.
    pub fn sweep(&mut self, probe: &dyn Fn(&str) -> bool) {
        let addrs: Vec<String> = self.workers.iter().map(|w| w.addr.clone()).collect();
        for addr in addrs {
            let alive = probe(&addr);
            self.heartbeat(&addr, alive);
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(&self.workers).unwrap_or_default()
    }
}

/// Wait for a predicate, polling. Used by tests and by callers bringing a fleet
/// up; sleeping a fixed duration and hoping is the usual alternative and it is
/// both slower and flakier.
pub fn wait_until(timeout: Duration, mut f: impl FnMut() -> bool) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if f() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    f()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ribosome::exec::{LocalExecutor, ToolRegistry};
    use crate::ribosome::Digest;

    fn tools() -> ToolRegistry {
        let mut r = ToolRegistry::new();
        r.register("upper@1", |action, inputs| {
            let src = inputs.values().next().cloned().unwrap_or_default();
            let up = String::from_utf8_lossy(&src).to_uppercase().into_bytes();
            let mut out = ToolOutput::new();
            for o in &action.outputs {
                out.outputs.insert(o.clone(), up.clone());
            }
            Ok(out)
        });
        r.register("boom@1", |_, _| {
            Err(ExecError::Deterministic { exit_code: 2, stderr: "it exploded".into() })
        });
        r.register("flaky@1", |_, _| Err(ExecError::Transient("network wobbled".into())));
        r
    }

    /// Start a worker on an ephemeral port; returns its address and a shutdown.
    fn spawn_worker(platform: Platform, name: &str) -> (String, Arc<AtomicBool>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        let exec: Arc<dyn Executor> = Arc::new(LocalExecutor::new(name, platform, tools()));
        let server = WorkerServer::new(exec)
            .with_signer(Arc::new(Signer::new(name, b"fleet key".to_vec())));
        let shutdown = server.shutdown_handle();
        std::thread::spawn(move || {
            let _ = server.serve(listener);
        });
        (addr, shutdown)
    }

    fn timeout() -> Duration {
        Duration::from_secs(5)
    }

    #[test]
    fn an_action_executes_on_a_remote_worker() {
        let (addr, stop) = spawn_worker(Platform::any(), "remote-1");
        let remote = RemoteExecutor::connect(&addr, timeout()).unwrap();
        assert_eq!(remote.name(), "remote-1", "capabilities come from the worker itself");

        let action = Action::new("t", "upper@1").input("a", Digest::of(b"x")).output("o");
        let mut inputs = Inputs::new();
        inputs.insert("a".into(), b"hello fleet".to_vec());

        let out = remote.run(&action, &inputs).unwrap();
        assert_eq!(out.outputs["o"], b"HELLO FLEET");
        stop.store(true, Ordering::Relaxed);
    }

    #[test]
    fn a_remote_worker_advertises_its_platform() {
        let (addr, stop) = spawn_worker(
            Platform::host("linux", "x86_64").with_accelerator("cuda"),
            "gpu-node",
        );
        let remote = RemoteExecutor::connect(&addr, timeout()).unwrap();
        assert_eq!(remote.platform().accelerator.as_deref(), Some("cuda"));

        let cpu_action = Action::new("t", "upper@1").output("o");
        assert!(remote.can_run(&cpu_action), "a GPU node can still take portable work");
        stop.store(true, Ordering::Relaxed);
    }

    #[test]
    fn failure_classification_survives_the_wire() {
        let (addr, stop) = spawn_worker(Platform::any(), "w");
        let remote = RemoteExecutor::connect(&addr, timeout()).unwrap();

        let det = remote.run(&Action::new("t", "boom@1").output("o"), &Inputs::new()).unwrap_err();
        assert!(
            !det.is_transient(),
            "a compile error must not come back as retryable: {det:?}"
        );

        let tr = remote.run(&Action::new("t", "flaky@1").output("o"), &Inputs::new()).unwrap_err();
        assert!(tr.is_transient(), "a network blip must stay retryable: {tr:?}");
        stop.store(true, Ordering::Relaxed);
    }

    #[test]
    fn a_dead_worker_reports_a_transient_error_not_a_build_failure() {
        let (addr, stop) = spawn_worker(Platform::any(), "w");
        let remote = RemoteExecutor::connect(&addr, Duration::from_millis(300)).unwrap();
        stop.store(true, Ordering::Relaxed);
        assert!(wait_until(timeout(), || !remote.ping()), "worker should stop accepting");

        let err = remote.run(&Action::new("t", "upper@1").output("o"), &Inputs::new()).unwrap_err();
        assert!(
            err.is_transient(),
            "infrastructure loss must be healable, not reported as broken code: {err:?}"
        );
    }

    #[test]
    fn connecting_to_nothing_fails_transiently_rather_than_hanging() {
        // Port 1 on loopback: nothing listens, and it refuses fast.
        let err = RemoteExecutor::connect("127.0.0.1:1", Duration::from_millis(300)).unwrap_err();
        assert!(err.is_transient());
    }

    #[test]
    fn results_from_a_remote_worker_carry_provenance() {
        let (addr, stop) = spawn_worker(Platform::any(), "signer-node");
        let mut stream = RemoteExecutor::dial(&addr, timeout()).unwrap();
        let action = Action::new("t", "upper@1").input("a", Digest::of(b"x")).output("o");
        let mut inputs = Inputs::new();
        inputs.insert("a".into(), b"data".to_vec());
        write_frame(
            &mut stream,
            &Frame::Execute { action: Box::new(action.clone()), inputs: encode(&inputs) },
        )
        .unwrap();

        match read_frame(&mut stream).unwrap() {
            Frame::Completed { provenance, outputs, .. } => {
                let p = provenance.expect("a signing worker must attach provenance");
                assert_eq!(p.worker, "signer-node");
                assert_eq!(p.action_key, action.key());

                // And it verifies against the fleet key over the real outputs.
                let signer = Signer::new("signer-node", b"fleet key".to_vec());
                let mut result = super::super::cas::ActionResult::ok("signer-node");
                for (path, hex) in &outputs {
                    result.outputs.insert(path.clone(), Digest::of(&hex_decode(hex).unwrap()));
                }
                assert!(signer.verify(&p, &action.key(), &result));
            }
            other => panic!("expected Completed, got {other:?}"),
        }
        stop.store(true, Ordering::Relaxed);
    }

    #[test]
    fn oversized_frames_are_refused_rather_than_allocated() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        std::thread::spawn(move || {
            if let Ok((mut s, _)) = listener.accept() {
                // Claim a 4 GB body and send nothing.
                let _ = s.write_all(&u32::MAX.to_be_bytes());
                std::thread::sleep(Duration::from_millis(200));
            }
        });
        let mut client = TcpStream::connect(&addr).unwrap();
        client.set_read_timeout(Some(timeout())).unwrap();
        assert!(read_frame(&mut client).is_err(), "a hostile length prefix must not be honoured");
    }

    // ---- registry

    fn record_names(rs: Vec<&WorkerRecord>) -> Vec<&str> {
        rs.into_iter().map(|r| r.name.as_str()).collect()
    }

    #[test]
    fn the_registry_routes_by_advertised_capability() {
        let mut reg = WorkerRegistry::new();
        reg.register("cpu", "10.0.0.1:9", Platform::host("linux", "x86_64"));
        reg.register("gpu", "10.0.0.2:9", Platform::host("linux", "x86_64").with_accelerator("cuda"));

        let gpu_action =
            Action::new("k", "t@1").platform(Platform::any().with_accelerator("cuda"));
        assert_eq!(record_names(reg.capable_for(&gpu_action)), vec!["gpu"]);
        assert_eq!(reg.capable_for(&Action::new("k", "t@1")).len(), 2);
    }

    #[test]
    fn a_worker_is_evicted_only_after_repeated_failures() {
        let mut reg = WorkerRegistry::new();
        reg.register("w", "10.0.0.1:9", Platform::any());
        reg.heartbeat("10.0.0.1:9", false);
        assert_eq!(reg.healthy().len(), 1, "one missed beat is a hiccup, not a death");
        reg.heartbeat("10.0.0.1:9", false);
        reg.heartbeat("10.0.0.1:9", false);
        assert!(reg.healthy().is_empty(), "three in a row is a death");
    }

    #[test]
    fn a_recovered_worker_rejoins() {
        let mut reg = WorkerRegistry::new();
        reg.register("w", "10.0.0.1:9", Platform::any());
        for _ in 0..3 {
            reg.heartbeat("10.0.0.1:9", false);
        }
        assert!(reg.healthy().is_empty());
        reg.heartbeat("10.0.0.1:9", true);
        assert_eq!(reg.healthy().len(), 1, "a worker that comes back must be usable again");
    }

    #[test]
    fn re_registration_after_restart_is_recovery_not_duplication() {
        let mut reg = WorkerRegistry::new();
        reg.register("w", "10.0.0.1:9", Platform::any());
        for _ in 0..3 {
            reg.heartbeat("10.0.0.1:9", false);
        }
        reg.register("w", "10.0.0.1:9", Platform::any());
        assert_eq!(reg.all().len(), 1, "no duplicate entry");
        assert_eq!(reg.healthy().len(), 1);
    }

    #[test]
    fn unhealthy_workers_are_not_routed_to() {
        let mut reg = WorkerRegistry::new();
        reg.register("w", "10.0.0.1:9", Platform::any());
        reg.sweep(&|_| false);
        reg.sweep(&|_| false);
        reg.sweep(&|_| false);
        assert!(reg.capable_for(&Action::new("k", "t@1")).is_empty());
    }

    #[test]
    fn a_live_sweep_marks_real_workers_healthy() {
        let (addr, stop) = spawn_worker(Platform::any(), "live");
        let remote = RemoteExecutor::connect(&addr, timeout()).unwrap();
        let mut reg = WorkerRegistry::new();
        reg.register(remote.name(), remote.addr(), remote.platform().clone());

        reg.sweep(&|a| a == remote.addr() && remote.ping());
        assert_eq!(reg.healthy().len(), 1);

        stop.store(true, Ordering::Relaxed);
        assert!(wait_until(timeout(), || !remote.ping()));
        for _ in 0..3 {
            reg.sweep(&|_| remote.ping());
        }
        assert!(reg.healthy().is_empty(), "the sweep must notice a worker that went away");
    }
}
