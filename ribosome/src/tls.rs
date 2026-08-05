//! Encrypted worker transport, behind the `tls` feature.
//!
//! ## What this module deliberately does not decide
//!
//! Encryption was the last open item on [`remote`](super::remote), and it stayed
//! open across several sessions for a reason that turned out to be only half
//! right. Half of it was real: **the trust posture is the operator's call.**
//! Pinned self-signed certificates, mutual TLS against an internal CA, and
//! public PKI are three different operational stories with three different
//! failure modes, and a build system that quietly picked one would be making a
//! security decision on its operator's behalf.
//!
//! The other half was wrong: that this made TLS *undeliverable* here. It does
//! not, because in `rustls` the posture already **is** a value —
//! [`rustls::ServerConfig`] and [`rustls::ClientConfig`] are precisely the
//! encoding of "who do I trust, and how do I prove who I am". So this module
//! takes those configs as parameters and supplies only the plumbing. The
//! operator brings the policy; the crate brings the wrapper.
//!
//! That is why there is no `with_self_signed()` convenience here. It would be
//! one line, it would be used, and it would be the decision this module exists
//! to avoid making.
//!
//! ## What it is
//!
//! Two adapters onto the seam [`remote`](super::remote) already exposes:
//!
//! | | |
//! |---|---|
//! | [`acceptor`] | a `rustls::ServerConfig` → the wrapper [`WorkerServer::serve_with`] wants |
//! | [`connector`] | a `rustls::ClientConfig` → the [`Connector`] `RemoteExecutor::connect_over` wants |
//!
//! Nothing else changes. The handshake, the frame codec, the executor, the
//! registry, and the scheduler are untouched, because the protocol was made
//! generic over `Read + Write` first. That refactor was the actual work; this
//! file is its payoff and is deliberately small.
//!
//! ## Ordering: TLS wraps the fleet-key handshake, it does not replace it
//!
//! The two authenticate different things and both are wanted. TLS says *this is
//! the machine whose certificate I trust* and encrypts the channel; the fleet
//! key ([`remote`](super::remote)) says *this peer knows the shared secret* and
//! gates every frame. Running the existing handshake inside the TLS session
//! means a worker that is reachable but not authorized still learns nothing, and
//! that an attacker who obtains the fleet key still cannot read traffic.
//!
//! [`WorkerServer::serve_with`]: super::remote::WorkerServer::serve_with
//! [`Connector`]: super::remote::Connector

use super::remote::{Connector, ReadWrite};
use rustls::{ClientConfig, ClientConnection, ServerConfig, ServerConnection, StreamOwned};
use std::io;
use std::net::TcpStream;
use std::sync::Arc;

/// Server side: turn a `rustls::ServerConfig` into the wrapper
/// [`WorkerServer::serve_with`](super::remote::WorkerServer::serve_with) takes.
///
/// The returned closure performs the TLS handshake on each accepted socket. A
/// peer that fails it — no certificate, an untrusted one, a version mismatch —
/// produces an `Err`, which `serve_with` treats as that connection's problem
/// alone: it is dropped and the accept loop continues. One peer presenting a bad
/// certificate must not take a worker offline.
pub fn acceptor(config: Arc<ServerConfig>) -> impl Fn(TcpStream) -> io::Result<Box<dyn ReadWrite>> + Send + Sync + 'static {
    move |tcp| {
        let conn = ServerConnection::new(config.clone())
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        Ok(Box::new(StreamOwned::new(conn, tcp)) as Box<dyn ReadWrite>)
    }
}

/// Client side: turn a `rustls::ClientConfig` into a [`Connector`].
///
/// `server_name` is the name checked against the worker's certificate. It is a
/// required parameter rather than something derived from the dial address,
/// because deriving it is how certificate verification quietly becomes
/// decorative — an IP literal has no name to check, and a caller that has to
/// pass the name has to have thought about what it should be.
pub fn connector(config: Arc<ClientConfig>, server_name: impl Into<String>) -> Connector {
    let name = server_name.into();
    Arc::new(move |tcp| {
        let server = rustls::pki_types::ServerName::try_from(name.clone())
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
        let conn = ClientConnection::new(config.clone(), server)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        Ok(Box::new(StreamOwned::new(conn, tcp)) as Box<dyn ReadWrite>)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exec::{Executor, Inputs, LocalExecutor, ToolOutput, ToolRegistry};
    use crate::remote::{RemoteExecutor, WorkerServer};
    use crate::{Action, Digest, Platform};
    use rustls::pki_types::{CertificateDer, PrivateKeyDer};
    use std::net::TcpListener;
    use std::sync::atomic::Ordering;
    use std::time::Duration;

    fn timeout() -> Duration {
        Duration::from_secs(10)
    }

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
        r
    }

    /// A throwaway self-signed certificate for `localhost`.
    ///
    /// Generated per test run rather than checked in: a committed certificate is
    /// a test with an expiry date, and it invites being copied into a
    /// deployment.
    fn self_signed() -> (CertificateDer<'static>, PrivateKeyDer<'static>) {
        let cert = rcgen::generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
        let der = CertificateDer::from(cert.cert.der().to_vec());
        let key = PrivateKeyDer::try_from(cert.signing_key.serialize_der()).unwrap();
        (der, key)
    }

    fn server_config(
        cert: CertificateDer<'static>,
        key: PrivateKeyDer<'static>,
    ) -> Arc<ServerConfig> {
        Arc::new(
            ServerConfig::builder()
                .with_no_client_auth()
                .with_single_cert(vec![cert], key)
                .unwrap(),
        )
    }

    /// A client that trusts exactly one certificate — the "pinned self-signed"
    /// posture, chosen *here in the test* rather than by the module.
    fn client_config(cert: CertificateDer<'static>) -> Arc<ClientConfig> {
        let mut roots = rustls::RootCertStore::empty();
        roots.add(cert).unwrap();
        Arc::new(ClientConfig::builder().with_root_certificates(roots).with_no_client_auth())
    }

    fn spawn(config: Arc<ServerConfig>, auth: Option<Vec<u8>>) -> (String, Arc<std::sync::atomic::AtomicBool>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        let exec: Arc<dyn Executor> = Arc::new(LocalExecutor::new("tls-worker", Platform::any(), tools()));
        let mut server = WorkerServer::new(exec);
        if let Some(k) = auth {
            server = server.with_auth(k);
        }
        let stop = server.shutdown_handle();
        std::thread::spawn(move || {
            let _ = server.serve_with(listener, acceptor(config));
        });
        (addr, stop)
    }

    #[test]
    fn an_action_executes_over_a_real_tls_session() {
        let (cert, key) = self_signed();
        let (addr, stop) = spawn(server_config(cert.clone(), key), None);

        let remote = RemoteExecutor::connect_over(
            &addr,
            timeout(),
            None,
            connector(client_config(cert), "localhost"),
        )
        .unwrap();
        assert_eq!(remote.name(), "tls-worker", "the advertisement crossed the TLS session");

        let action = Action::new("t", "upper@1").input("a", Digest::of(b"payload")).output("o");
        let mut inputs = Inputs::new();
        inputs.insert("a".into(), b"payload".to_vec());
        assert_eq!(remote.run(&action, &inputs).unwrap().outputs["o"], b"PAYLOAD");

        stop.store(true, Ordering::Relaxed);
    }

    #[test]
    fn a_client_that_does_not_trust_the_certificate_is_refused() {
        // Without this the test above proves only that bytes moved. A client
        // trusting a *different* self-signed cert must fail to connect, or
        // verification is decorative.
        let (cert, key) = self_signed();
        let (other, _) = self_signed();
        let (addr, stop) = spawn(server_config(cert, key), None);

        let err = RemoteExecutor::connect_over(
            &addr,
            timeout(),
            None,
            connector(client_config(other), "localhost"),
        )
        .unwrap_err();
        assert!(err.is_transient(), "a rejected certificate is infrastructure, not a build error");

        stop.store(true, Ordering::Relaxed);
    }

    #[test]
    fn a_plaintext_client_cannot_talk_to_a_tls_worker() {
        // The mirror case: dialling a TLS-only fleet without TLS must fail
        // rather than silently downgrade.
        let (cert, key) = self_signed();
        let (addr, stop) = spawn(server_config(cert, key), None);
        assert!(RemoteExecutor::connect(&addr, timeout()).is_err());
        stop.store(true, Ordering::Relaxed);
    }

    #[test]
    fn the_fleet_key_still_gates_every_frame_inside_the_tls_session() {
        // TLS and the fleet key authenticate different things, and wrapping one
        // in the other must not disable the other. An encrypted channel to a
        // worker you are not authorized to use is still unauthorized.
        let (cert, key) = self_signed();
        let (addr, stop) = spawn(server_config(cert.clone(), key), Some(b"fleet-key".to_vec()));

        // Right certificate, no fleet key: refused.
        assert!(RemoteExecutor::connect_over(
            &addr,
            timeout(),
            None,
            connector(client_config(cert.clone()), "localhost"),
        )
        .is_err());

        // Right certificate and the right key: admitted.
        let ok = RemoteExecutor::connect_over(
            &addr,
            timeout(),
            Some(b"fleet-key".to_vec()),
            connector(client_config(cert), "localhost"),
        )
        .unwrap();
        assert_eq!(ok.name(), "tls-worker");

        stop.store(true, Ordering::Relaxed);
    }
}
