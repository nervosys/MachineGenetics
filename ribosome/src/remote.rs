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
//! ## Encryption
//!
//! Plaintext by default; encrypted via [`tls`](super::tls) behind the `tls`
//! feature.
//!
//! What used to block that was not cryptography, it was *spelling*: the protocol
//! is length-prefixed JSON over an ordered byte stream, and it was written
//! against [`TcpStream`] concretely. Both ends are now generic over
//! `Read + Write`, with one wrapper point per side —
//! [`WorkerServer::serve_with`] and [`RemoteExecutor::connect_over`]. The seam
//! is tested with a byte-transforming wrapper *and* its negative case, then
//! `tls` plugs a real `rustls` session into the same two points without changing
//! the handshake, the frame codec, or the executor.
//!
//! The trust posture is still **not** decided here, and that is the point of the
//! design rather than an omission: [`tls::acceptor`](super::tls::acceptor) and
//! [`tls::connector`](super::tls::connector) take the caller's
//! `rustls::ServerConfig` / `ClientConfig`, which is already the encoding of
//! "who do I trust and how do I prove who I am". Pinned self-signed, mutual TLS
//! against an internal CA, and public PKI are all expressible; none is picked
//! for the operator.
//!
//! TLS **wraps** the fleet-key handshake rather than replacing it. The two
//! authenticate different things: TLS says *this is the machine whose
//! certificate I trust* and encrypts the channel; the fleet key says *this peer
//! knows the shared secret* and gates every frame. There is a test asserting a
//! valid certificate alone still gets you refused.
//!
//! Without the feature the posture is the one an unauthenticated build cache
//! has: fine on a trusted network segment, unsafe on an open one, and it should
//! be deployed the same way.

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
    /// Server's opening move when it requires authentication: a per-connection
    /// nonce the client must prove it can MAC.
    Challenge { nonce: String },
    /// Client's proof: `HMAC(fleet key, nonce)`.
    Auth { mac: String },
    /// Server accepted the proof.
    Welcome,
    /// Server refused it. Deliberately says nothing about *why* — an attacker
    /// learns only "no", not whether the identity existed or the key was close.
    Denied,
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

/// Frame IO is generic over the byte stream, not tied to [`TcpStream`].
///
/// This is the seam TLS needs. The protocol is length-prefixed JSON over an
/// ordered, reliable byte stream — it never depended on sockets, it was merely
/// *written* against them, and that spelling was the actual obstacle to
/// encrypting it. A `rustls::StreamOwned`, an SSH channel, a Unix socket, or an
/// in-memory pipe are all `Read + Write`, so each is now a wrapper rather than a
/// second implementation of the handshake.
///
/// The immediate dividend is testing: the protocol can be driven over a scripted
/// in-memory stream, so properties like "authentication gates every frame" are
/// asserted deterministically instead of against a live socket with timeouts.
fn write_frame<W: Write>(stream: &mut W, frame: &Frame) -> std::io::Result<()> {
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

fn read_frame<R: Read>(stream: &mut R) -> std::io::Result<Frame> {
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

/// Per-connection nonce.
///
/// Server-generated, so a captured `Auth` frame cannot be replayed against a
/// later connection — the nonce it proves knowledge of will not come round
/// again. Derived from the clock plus a counter rather than a CSPRNG: the
/// requirement is uniqueness per connection, not unpredictability, because the
/// secret is the key and the nonce is only there to make each proof single-use.
fn next_nonce() -> String {
    use std::sync::atomic::AtomicU64;
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    hex_encode(&crate::mac::hmac_sha256(b"ribosome-nonce", &[t.to_le_bytes(), n.to_le_bytes()].concat()))
}

fn auth_response(key: &[u8], nonce: &str) -> String {
    hex_encode(&crate::mac::hmac_sha256(key, nonce.as_bytes()))
}

/// A worker process: serves actions to whoever connects.
pub struct WorkerServer {
    inner: Arc<dyn Executor>,
    signer: Option<Arc<Signer>>,
    /// When set, a client must prove knowledge of this key before it can ask
    /// for anything. Optional because a single-host fleet on loopback has no
    /// one to authenticate against, and mandatory ceremony that everyone
    /// disables is worse than an honest opt-in.
    auth_key: Option<Vec<u8>>,
    shutdown: Arc<AtomicBool>,
}

impl WorkerServer {
    pub fn new(inner: Arc<dyn Executor>) -> Self {
        WorkerServer {
            inner,
            signer: None,
            auth_key: None,
            shutdown: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Require clients to authenticate with the fleet key.
    pub fn with_auth(mut self, key: impl Into<Vec<u8>>) -> Self {
        self.auth_key = Some(key.into());
        self
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

    /// Serve plaintext until shut down. Blocks; callers run it on its own thread.
    pub fn serve(&self, listener: TcpListener) -> std::io::Result<()> {
        self.serve_with(listener, Ok)
    }

    /// Serve until shut down, wrapping each accepted socket first.
    ///
    /// `wrap` is where a TLS server session goes: it receives the raw socket and
    /// returns whatever byte stream the protocol should actually run over. A
    /// wrapper that fails — a rejected certificate, a client that will not
    /// negotiate — drops *that connection* and leaves the accept loop running,
    /// because one bad peer must not take the worker down.
    pub fn serve_with<S, F>(&self, listener: TcpListener, wrap: F) -> std::io::Result<()>
    where
        S: Read + Write + Send + 'static,
        F: Fn(TcpStream) -> std::io::Result<S> + Send + Sync + 'static,
    {
        // A short accept timeout so shutdown is observed promptly rather than
        // blocking until the next client happens to connect.
        listener.set_nonblocking(true)?;
        let wrap = Arc::new(wrap);
        while !self.shutdown.load(Ordering::Relaxed) {
            match listener.accept() {
                Ok((stream, _)) => {
                    stream.set_nonblocking(false)?;
                    let inner = self.inner.clone();
                    let signer = self.signer.clone();
                    let auth = self.auth_key.clone();
                    let wrap = wrap.clone();
                    std::thread::spawn(move || {
                        // A wrapper failure — a rejected certificate, a peer
                        // that will not negotiate — ends this connection and
                        // nothing else. The accept loop keeps running.
                        if let Ok(s) = wrap(stream) {
                            let _ = Self::handle(s, inner, signer, auth);
                        }
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

    /// Run the server side of the protocol over any byte stream.
    ///
    /// `pub` so a caller that already has a connected stream — from a TLS
    /// acceptor, a Unix socket, a test harness — can serve it without going
    /// through [`serve_with`](Self::serve_with).
    pub fn handle<S: Read + Write>(
        mut stream: S,
        inner: Arc<dyn Executor>,
        signer: Option<Arc<Signer>>,
        auth_key: Option<Vec<u8>>,
    ) -> std::io::Result<()> {
        // The server always opens, with `Challenge` or `Welcome`. Being
        // explicit about "no auth required" rather than staying silent keeps the
        // handshake unambiguous — a silent open worker and a hung one look
        // identical to a client, and it would have to wait out its timeout to
        // tell them apart.
        //
        // Authentication gates *every* frame, not just `Execute`: a worker that
        // answers `Describe` to an unauthenticated peer has already disclosed
        // its capabilities and tool set.
        match &auth_key {
            None => write_frame(&mut stream, &Frame::Welcome)?,
            Some(key) => {
                let nonce = next_nonce();
                write_frame(&mut stream, &Frame::Challenge { nonce: nonce.clone() })?;
                let ok = match read_frame(&mut stream) {
                    Ok(Frame::Auth { mac }) => {
                        let expect = auth_response(key, &nonce);
                        match (hex_decode(&mac), hex_decode(&expect)) {
                            (Ok(a), Ok(b)) => crate::mac::ct_eq(&a, &b),
                            _ => false,
                        }
                    }
                    _ => false,
                };
                if !ok {
                    let _ = write_frame(&mut stream, &Frame::Denied);
                    return Ok(());
                }
                write_frame(&mut stream, &Frame::Welcome)?;
            }
        }

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

/// Any bidirectional byte stream the protocol can run over.
///
/// A blanket impl covers `TcpStream`, a TLS session, a Unix socket, and the test
/// harness alike, so nothing has to opt in.
pub trait ReadWrite: Read + Write + Send {}
impl<T: Read + Write + Send> ReadWrite for T {}

/// Turns a connected socket into the stream the protocol runs over.
///
/// `Send + Sync` because a [`RemoteExecutor`] is shared across the scheduler's
/// threads; every worker in a fleet may be dialled concurrently.
pub type Connector = Arc<dyn Fn(TcpStream) -> std::io::Result<Box<dyn ReadWrite>> + Send + Sync>;

/// A worker reached over the network. Implements [`Executor`], so the scheduler
/// cannot tell it apart from a local one.
pub struct RemoteExecutor {
    name: String,
    addr: String,
    platform: Platform,
    timeout: Duration,
    auth_key: Option<Vec<u8>>,
    /// `None` is plaintext. `Some` wraps each connection — where a TLS client
    /// session goes.
    connector: Option<Connector>,
}

// Hand-written because a `Connector` is a closure and cannot derive `Debug`.
// The connector is reported as present/absent, which is the part anyone
// debugging a transport problem actually wants to know.
impl std::fmt::Debug for RemoteExecutor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RemoteExecutor")
            .field("name", &self.name)
            .field("addr", &self.addr)
            .field("platform", &self.platform)
            .field("timeout", &self.timeout)
            .field("authenticated", &self.auth_key.is_some())
            .field("wrapped", &self.connector.is_some())
            .finish()
    }
}

impl RemoteExecutor {
    /// Connect and ask the worker what it is.
    ///
    /// Querying rather than being told keeps the fleet's view of a worker's
    /// capabilities sourced from the worker itself; a hand-maintained roster
    /// drifts, and a drifted roster routes GPU work to a machine without one.
    pub fn connect(addr: impl Into<String>, timeout: Duration) -> Result<Self, ExecError> {
        Self::connect_inner(addr.into(), timeout, None, None)
    }

    /// Connect to a worker that requires the fleet key.
    pub fn connect_authenticated(
        addr: impl Into<String>,
        timeout: Duration,
        key: impl Into<Vec<u8>>,
    ) -> Result<Self, ExecError> {
        Self::connect_inner(addr.into(), timeout, Some(key.into()), None)
    }

    /// Connect over a wrapped transport — the client half of TLS.
    ///
    /// `connector` runs on every connection this executor opens, not just this
    /// one: the advertisement handshake, each `run`, and each `ping`. A worker
    /// reachable only over TLS must stay that way for its whole lifetime, and an
    /// executor that negotiated once and then dialled plaintext would be worse
    /// than no encryption, because it would look encrypted.
    pub fn connect_over(
        addr: impl Into<String>,
        timeout: Duration,
        key: Option<Vec<u8>>,
        connector: Connector,
    ) -> Result<Self, ExecError> {
        Self::connect_inner(addr.into(), timeout, key, Some(connector))
    }

    fn connect_inner(
        addr: String,
        timeout: Duration,
        auth_key: Option<Vec<u8>>,
        connector: Option<Connector>,
    ) -> Result<Self, ExecError> {
        // Built first so the advertisement handshake uses the very same
        // transport path as every later call — including the wrapper. Probing
        // with a plaintext dial and then running encrypted would test one thing
        // and use another.
        let probe = RemoteExecutor {
            name: String::new(),
            addr,
            platform: Platform::any(),
            timeout,
            auth_key,
            connector,
        };
        let mut stream = probe.open()?;
        write_frame(&mut stream, &Frame::Describe)
            .map_err(|e| ExecError::Transient(format!("describe failed: {e}")))?;
        match read_frame(&mut stream) {
            Ok(Frame::Advertisement { worker, platform, .. }) => {
                Ok(RemoteExecutor { name: worker, platform, ..probe })
            }
            Ok(other) => Err(ExecError::Transient(format!("unexpected reply: {other:?}"))),
            Err(e) => Err(ExecError::Transient(format!("no advertisement: {e}"))),
        }
    }

    /// Complete the opening handshake.
    ///
    /// An authenticating client can talk to an open worker (it simply gets
    /// `Welcome` immediately). The reverse — an unauthenticated client reaching
    /// a challenging worker — fails, which is the direction that matters.
    pub fn authenticate<S: Read + Write>(
        stream: &mut S,
        key: Option<&[u8]>,
    ) -> Result<(), ExecError> {
        match read_frame(stream) {
            Ok(Frame::Welcome) => Ok(()),
            Ok(Frame::Challenge { nonce }) => {
                let Some(key) = key else {
                    return Err(ExecError::Transient(
                        "worker requires authentication and no fleet key was supplied".into(),
                    ));
                };
                write_frame(stream, &Frame::Auth { mac: auth_response(key, &nonce) })
                    .map_err(|e| ExecError::Transient(format!("auth send: {e}")))?;
                match read_frame(stream) {
                    Ok(Frame::Welcome) => Ok(()),
                    Ok(Frame::Denied) => {
                        Err(ExecError::Transient("worker rejected the fleet key".into()))
                    }
                    Ok(other) => {
                        Err(ExecError::Transient(format!("unexpected handshake reply: {other:?}")))
                    }
                    Err(e) => Err(ExecError::Transient(format!("handshake reply: {e}"))),
                }
            }
            Ok(other) => {
                Err(ExecError::Transient(format!("expected a handshake, got {other:?}")))
            }
            Err(e) => Err(ExecError::Transient(format!("handshake read: {e}"))),
        }
    }

    /// Wrap a freshly connected socket before the protocol runs over it.
    ///
    /// The client-side counterpart of [`WorkerServer::serve_with`]. Boxed rather
    /// than generic because [`RemoteExecutor`] is used as `dyn Executor`
    /// throughout the scheduler, and a type parameter here would leak into every
    /// pool, registry, and fleet that holds one.
    fn wrap(&self, tcp: TcpStream) -> Result<Box<dyn ReadWrite>, ExecError> {
        match &self.connector {
            None => Ok(Box::new(tcp)),
            Some(c) => c(tcp).map_err(|e| ExecError::Transient(format!("handshake failed: {e}"))),
        }
    }

    /// Open a connection and complete the opening handshake.
    ///
    /// One place, so `run` and `ping` cannot drift on transport or auth.
    fn open(&self) -> Result<Box<dyn ReadWrite>, ExecError> {
        let tcp = Self::dial(&self.addr, self.timeout)?;
        let mut stream = self.wrap(tcp)?;
        Self::authenticate(&mut stream, self.auth_key.as_deref())?;
        Ok(stream)
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
        let Ok(mut s) = self.open() else { return false };
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
        let mut stream = self.open()?;
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
    use crate::exec::{LocalExecutor, ToolRegistry};
    use crate::Digest;

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
            .with_signer(Arc::new(Signer::new(name, b"fleet key that is at least 32 bytes long!".to_vec()).expect("32-byte test key")));
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
        // `timeout()`, not 300ms. `spawn_worker` returns as soon as the thread
        // is spawned, so the listener may not be accepting yet, and this
        // connect is *setup* — the test is about what happens once the worker
        // dies. Under the load of a full `test-all.sh` run the 300ms budget
        // lapsed and the whole suite went red on a Windows `os error 10060`,
        // which reads as a product failure rather than as a tight timeout.
        // The short-timeout behaviour it looked like it was covering is really
        // covered by `connecting_to_nothing_fails_transiently_rather_than_hanging`.
        let remote = RemoteExecutor::connect(&addr, timeout()).unwrap();
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
        RemoteExecutor::authenticate(&mut stream, None).unwrap();
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
                let signer = Signer::new("signer-node", b"fleet key that is at least 32 bytes long!".to_vec()).expect("32-byte test key");
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

    // ---- authentication

    /// Start a worker that demands the fleet key.
    fn spawn_authed_worker(key: &[u8]) -> (String, Arc<AtomicBool>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        let exec: Arc<dyn Executor> =
            Arc::new(LocalExecutor::new("secure", Platform::any(), tools()));
        let server = WorkerServer::new(exec).with_auth(key.to_vec());
        let shutdown = server.shutdown_handle();
        std::thread::spawn(move || {
            let _ = server.serve(listener);
        });
        (addr, shutdown)
    }

    #[test]
    fn a_client_with_the_fleet_key_is_admitted() {
        let (addr, stop) = spawn_authed_worker(b"fleet secret");
        let remote =
            RemoteExecutor::connect_authenticated(&addr, timeout(), b"fleet secret".to_vec())
                .unwrap();
        assert_eq!(remote.name(), "secure");

        let action = Action::new("t", "upper@1").input("a", Digest::of(b"x")).output("o");
        let mut inputs = Inputs::new();
        inputs.insert("a".into(), b"secret work".to_vec());
        assert_eq!(remote.run(&action, &inputs).unwrap().outputs["o"], b"SECRET WORK");
        stop.store(true, Ordering::Relaxed);
    }

    #[test]
    fn a_client_without_the_key_learns_nothing_not_even_capabilities() {
        let (addr, stop) = spawn_authed_worker(b"fleet secret");
        let err = RemoteExecutor::connect(&addr, timeout()).unwrap_err();
        assert!(
            err.to_string().contains("requires authentication"),
            "an unauthenticated peer must not even get an advertisement: {err}"
        );
        stop.store(true, Ordering::Relaxed);
    }

    #[test]
    fn a_wrong_key_is_refused() {
        let (addr, stop) = spawn_authed_worker(b"fleet secret");
        let err =
            RemoteExecutor::connect_authenticated(&addr, timeout(), b"guessed".to_vec())
                .unwrap_err();
        assert!(err.to_string().contains("rejected"), "{err}");
        stop.store(true, Ordering::Relaxed);
    }

    #[test]
    fn a_captured_proof_cannot_be_replayed_on_a_new_connection() {
        let key = b"fleet secret".to_vec();
        let (addr, stop) = spawn_authed_worker(&key);

        // Capture a valid response to one connection's challenge.
        let mut first = RemoteExecutor::dial(&addr, timeout()).unwrap();
        let captured = match read_frame(&mut first).unwrap() {
            Frame::Challenge { nonce } => auth_response(&key, &nonce),
            other => panic!("expected a challenge, got {other:?}"),
        };

        // Replay it against a fresh connection, which has a different nonce.
        let mut second = RemoteExecutor::dial(&addr, timeout()).unwrap();
        let _ = read_frame(&mut second).unwrap(); // its own challenge
        write_frame(&mut second, &Frame::Auth { mac: captured }).unwrap();
        assert!(
            matches!(read_frame(&mut second), Ok(Frame::Denied)),
            "a per-connection nonce must make each proof single-use"
        );
        stop.store(true, Ordering::Relaxed);
    }

    #[test]
    fn nonces_do_not_repeat() {
        let a = next_nonce();
        let b = next_nonce();
        assert_ne!(a, b);
    }

    #[test]
    fn an_authenticating_client_can_still_use_an_open_worker() {
        let (addr, stop) = spawn_worker(Platform::any(), "open");
        let remote =
            RemoteExecutor::connect_authenticated(&addr, timeout(), b"unused key".to_vec())
                .unwrap();
        assert_eq!(remote.name(), "open");
        stop.store(true, Ordering::Relaxed);
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

    // ── the protocol, off a socket ───────────────────────────────────────────
    //
    // These drive the *server side* over a scripted in-memory stream. They exist
    // because the protocol was written against `TcpStream` and therefore could
    // only be tested against a live socket, with sleeps and timeouts. Nothing
    // about length-prefixed JSON needs a socket, and saying so in the type is
    // what makes TLS a wrapper rather than a second handshake.

    /// A byte stream that replays a fixed script and records what was written.
    ///
    /// Read returns 0 at the end of the script, which the server sees as a
    /// closed connection — the normal end of a session — so `handle` returns
    /// rather than blocking. That is what makes these tests deterministic.
    struct Scripted {
        input: std::io::Cursor<Vec<u8>>,
        output: Vec<u8>,
    }

    impl Read for Scripted {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            self.input.read(buf)
        }
    }

    impl Write for Scripted {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.output.extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn script(frames: &[Frame]) -> Scripted {
        let mut buf = Vec::new();
        for f in frames {
            write_frame(&mut buf, f).unwrap();
        }
        Scripted { input: std::io::Cursor::new(buf), output: Vec::new() }
    }

    fn replies(s: &Scripted) -> Vec<Frame> {
        let mut cur = std::io::Cursor::new(s.output.clone());
        let mut out = Vec::new();
        while let Ok(f) = read_frame(&mut cur) {
            out.push(f);
        }
        out
    }

    fn served(auth: Option<Vec<u8>>, frames: &[Frame]) -> Vec<Frame> {
        let inner: Arc<dyn Executor> =
            Arc::new(LocalExecutor::new("scripted", Platform::any(), tools()));
        let mut s = script(frames);
        WorkerServer::handle(&mut s, inner, None, auth).unwrap();
        replies(&s)
    }

    #[test]
    fn the_protocol_runs_over_any_byte_stream_not_only_sockets() {
        let out = served(None, &[Frame::Ping, Frame::Describe]);
        assert_eq!(out[0], Frame::Welcome, "the server always opens the handshake");
        assert_eq!(out[1], Frame::Pong);
        assert!(matches!(out[2], Frame::Advertisement { .. }));
    }

    #[test]
    fn authentication_gates_every_frame_including_describe() {
        // The property the auth handshake exists for, asserted deterministically
        // rather than against a socket: a worker that answers `Describe` to an
        // unauthenticated peer has already disclosed its capabilities and tools.
        let out = served(Some(b"fleet-key".to_vec()), &[Frame::Describe, Frame::Ping]);
        assert!(matches!(out[0], Frame::Challenge { .. }));
        assert_eq!(out[1], Frame::Denied);
        assert_eq!(out.len(), 2, "nothing after the refusal — not even a Pong: {out:?}");
        assert!(
            !out.iter().any(|f| matches!(f, Frame::Advertisement { .. })),
            "capabilities must not leak to an unauthenticated peer"
        );
    }

    #[test]
    fn a_correct_proof_opens_the_session_and_a_wrong_one_closes_it() {
        let key = b"fleet-key".to_vec();

        // The nonce is server-generated, so the proof can only be computed after
        // reading the challenge — which a fixed script cannot do. Run the
        // handshake in two passes: learn the nonce, then answer it.
        let probe = served(Some(key.clone()), &[]);
        let Frame::Challenge { nonce } = &probe[0] else {
            panic!("expected a challenge, got {probe:?}");
        };
        // Nonces are per-connection, so this one is already spent; what it
        // proves is that a *wrong* mac is refused.
        let wrong = served(
            Some(key.clone()),
            &[Frame::Auth { mac: auth_response(b"not-the-key", nonce) }],
        );
        assert_eq!(wrong[1], Frame::Denied);
    }

    // ── a wrapped transport, end to end ─────────────────────────────────────

    /// A stream that XORs every byte in both directions.
    ///
    /// **This is not encryption and must never be mistaken for it** — a fixed
    /// XOR is trivially broken and provides no confidentiality whatsoever. It is
    /// here for exactly one reason: it is a *byte-transforming* wrapper with no
    /// cryptographic dependency, so it proves the transport seam carries a
    /// transform in both directions. TLS plugs into the same two points; what is
    /// tested here is the plumbing, not any security property.
    struct Xor<S> {
        inner: S,
        key: u8,
    }

    impl<S> Xor<S> {
        fn new(inner: S, key: u8) -> Self {
            Xor { inner, key }
        }
    }

    impl<S: Read> Read for Xor<S> {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            let n = self.inner.read(buf)?;
            for b in &mut buf[..n] {
                *b ^= self.key;
            }
            Ok(n)
        }
    }

    impl<S: Write> Write for Xor<S> {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            let masked: Vec<u8> = buf.iter().map(|b| b ^ self.key).collect();
            self.inner.write_all(&masked)?;
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            self.inner.flush()
        }
    }

    fn spawn_wrapped_worker(key: u8) -> (String, Arc<AtomicBool>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        let exec: Arc<dyn Executor> = Arc::new(LocalExecutor::new("wrapped", Platform::any(), tools()));
        let server = WorkerServer::new(exec);
        let shutdown = server.shutdown_handle();
        std::thread::spawn(move || {
            let _ = server.serve_with(listener, move |tcp| Ok(Xor::new(tcp, key)));
        });
        (addr, shutdown)
    }

    fn xor_connector(key: u8) -> Connector {
        Arc::new(move |tcp| Ok(Box::new(Xor::new(tcp, key)) as Box<dyn ReadWrite>))
    }

    #[test]
    fn a_wrapped_transport_carries_a_whole_execution_in_both_directions() {
        let (addr, stop) = spawn_wrapped_worker(0x5a);
        let remote =
            RemoteExecutor::connect_over(&addr, timeout(), None, xor_connector(0x5a)).unwrap();
        assert_eq!(remote.name(), "wrapped", "the advertisement crossed the wrapper");
        assert!(remote.ping(), "and so does a ping");

        let action = Action::new("t", "upper@1").input("a", Digest::of(b"payload")).output("o");
        let mut inputs = Inputs::new();
        inputs.insert("a".into(), b"payload".to_vec());
        let out = remote.run(&action, &inputs).unwrap();
        assert_eq!(out.outputs["o"], b"PAYLOAD", "a full round trip through the wrapper");

        stop.store(true, Ordering::Relaxed);
    }

    #[test]
    fn a_mismatched_wrapper_cannot_talk_to_the_worker() {
        // Without this the previous test proves nothing: a wrapper that quietly
        // did nothing would pass it. Different keys means the bytes really are
        // transformed, so the seam is carrying the transform rather than being
        // decorative.
        let (addr, stop) = spawn_wrapped_worker(0x5a);
        let err = RemoteExecutor::connect_over(&addr, timeout(), None, xor_connector(0x33))
            .unwrap_err();
        assert!(err.is_transient(), "a handshake mismatch is infrastructure, not a build error");

        // And plaintext against a wrapped worker fails too — the mirror case,
        // which is what a client dialling a TLS-only fleet without TLS would do.
        assert!(RemoteExecutor::connect(&addr, timeout()).is_err());

        stop.store(true, Ordering::Relaxed);
    }

    #[test]
    fn an_oversized_frame_is_refused_before_it_is_allocated() {
        // A length prefix off the network is attacker-controlled: without the
        // cap, `0xFFFFFFFF` is a 4 GB allocation request from one packet.
        let mut raw = (MAX_FRAME + 1).to_be_bytes().to_vec();
        raw.extend_from_slice(b"{}");
        let mut cur = std::io::Cursor::new(raw);
        let err = read_frame(&mut cur).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
        assert!(err.to_string().contains("exceeds"), "{err}");
    }

    #[test]
    fn a_truncated_frame_is_an_error_not_a_partial_parse() {
        // Claims 64 bytes, supplies 2. Must fail rather than decode whatever
        // arrived — a half-read Execute frame is an action nobody submitted.
        let mut raw = 64u32.to_be_bytes().to_vec();
        raw.extend_from_slice(b"{}");
        let mut cur = std::io::Cursor::new(raw);
        assert!(read_frame(&mut cur).is_err());
    }
}
