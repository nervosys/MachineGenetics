//! # std::net — Networking
//!
//! TCP, UDP, HTTP client/server, and DNS resolution.
//! All networking functions declare the `net` effect.

use std::io::{Read, Write, IoError, IoErrorKind};

// ---------------------------------------------------------------------------
// TCP
// ---------------------------------------------------------------------------

/// A TCP stream between a local and a remote socket.
pub struct TcpStream {
    _fd: u64,
}

impl TcpStream {
    /// Connect to a remote address.
    pub fn connect(addr: &String) -> Result<TcpStream, IoError> / net;

    /// Returns the remote address.
    pub fn peer_addr(&self) -> Result<SocketAddr, IoError>;

    /// Returns the local address.
    pub fn local_addr(&self) -> Result<SocketAddr, IoError>;

    /// Shuts down the read, write, or both halves of the connection.
    pub fn shutdown(&self, how: Shutdown) -> Result<(), IoError> / net;

    /// Set the read timeout.
    pub fn set_read_timeout(&self, dur: Option<std::time::Duration>) -> Result<(), IoError>;

    /// Set the write timeout.
    pub fn set_write_timeout(&self, dur: Option<std::time::Duration>) -> Result<(), IoError>;
}

impl Read for TcpStream {
    pub fn read(&mut self, buf: &mut [u8]) -> Result<usize, IoError> / net;
    pub fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), IoError> / net;
}

impl Write for TcpStream {
    pub fn write(&mut self, buf: &[u8]) -> Result<usize, IoError> / net;
    pub fn flush(&mut self) -> Result<(), IoError> / net;
}

/// A TCP socket listener.
pub struct TcpListener {
    _fd: u64,
}

impl TcpListener {
    /// Bind to the given address.
    pub fn bind(addr: &String) -> Result<TcpListener, IoError> / net;

    /// Accept a new incoming connection.
    pub fn accept(&self) -> Result<(TcpStream, SocketAddr), IoError> / net;

    /// Returns an iterator over incoming connections.
    pub fn incoming(&self) -> Incoming / net;

    /// Returns the local address.
    pub fn local_addr(&self) -> Result<SocketAddr, IoError>;
}

/// Iterator over incoming TCP connections.
pub struct Incoming {
    listener: &TcpListener,
}

impl Iterator for Incoming {
    type Item = Result<TcpStream, IoError>;
    pub fn next(&mut self) -> Option<Result<TcpStream, IoError>> / net;
}

// ---------------------------------------------------------------------------
// UDP
// ---------------------------------------------------------------------------

/// A UDP socket.
pub struct UdpSocket {
    _fd: u64,
}

impl UdpSocket {
    /// Bind to the given address.
    pub fn bind(addr: &String) -> Result<UdpSocket, IoError> / net;

    /// Send data to the given address.
    pub fn send_to(&self, buf: &[u8], addr: &String) -> Result<usize, IoError> / net;

    /// Receive data, returning byte count and source address.
    pub fn recv_from(&self, buf: &mut [u8]) -> Result<(usize, SocketAddr), IoError> / net;

    /// Connect to a remote address for subsequent `send`/`recv`.
    pub fn connect(&self, addr: &String) -> Result<(), IoError> / net;

    pub fn send(&self, buf: &[u8]) -> Result<usize, IoError> / net;
    pub fn recv(&self, buf: &mut [u8]) -> Result<usize, IoError> / net;
}

// ---------------------------------------------------------------------------
// Socket address
// ---------------------------------------------------------------------------

/// A socket address (IP + port).
pub enum SocketAddr {
    V4(SocketAddrV4),
    V6(SocketAddrV6),
}

pub struct SocketAddrV4 { ip: [u8; 4], port: u16 }
pub struct SocketAddrV6 { ip: [u8; 16], port: u16 }

pub enum Shutdown {
    Read,
    Write,
    Both,
}

// ---------------------------------------------------------------------------
// HTTP — first-class in MechGen
// ---------------------------------------------------------------------------

/// HTTP request method.
pub enum Method {
    Get,
    Post,
    Put,
    Delete,
    Patch,
    Head,
    Options,
}

/// An HTTP request.
pub struct Request {
    method: Method,
    url: String,
    headers: HashMap<String, String>,
    body: Option<Vec<u8>>,
}

impl Request {
    pub fn get(url: &String) -> Request {
        Request {
            method: Method::Get,
            url: url.to_owned(),
            headers: HashMap::new(),
            body: None,
        }
    }

    pub fn post(url: &String) -> Request {
        Request {
            method: Method::Post,
            url: url.to_owned(),
            headers: HashMap::new(),
            body: None,
        }
    }

    pub fn header(&mut self, key: &String, value: &String) -> &mut Request {
        self.headers.insert(key.to_owned(), value.to_owned());
        self
    }

    pub fn body(&mut self, data: Vec<u8>) -> &mut Request {
        self.body = Some(data);
        self
    }

    pub fn json<T: std::json::Serialize>(&mut self, value: &T) -> &mut Request {
        self.body = Some(std::json::stringify(value).into_bytes());
        self.header("Content-Type", "application/json")
    }

    /// Send the request and return a response.
    pub fn send(&self) -> Result<Response, HttpError> / net;
}

/// An HTTP response.
pub struct Response {
    status: u16,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

impl Response {
    /// HTTP status code.
    pub fn status(&self) -> u16 { self.status }

    /// Response body as a string.
    pub fn text(&self) -> Result<String, IoError> {
        String::from_utf8(&self.body)
    }

    /// Deserialize JSON body.
    pub fn json<T: std::json::Deserialize>(&self) -> Result<T, HttpError> {
        std::json::parse(&self.text()?)
    }

    /// Whether the status code indicates success (2xx).
    pub fn is_success(&self) -> bool { self.status >= 200 && self.status < 300 }
}

/// HTTP error type.
pub struct HttpError {
    kind: HttpErrorKind,
    message: String,
}

pub enum HttpErrorKind {
    ConnectionFailed,
    Timeout,
    InvalidUrl,
    TlsError,
    ParseError,
    Other,
}

// ---------------------------------------------------------------------------
// DNS
// ---------------------------------------------------------------------------

/// Resolve a hostname to IP addresses.
pub fn resolve(host: &String) -> Result<Vec<SocketAddr>, IoError> / net;

/// Reverse-resolve an IP address to a hostname.
pub fn reverse_resolve(addr: &SocketAddr) -> Result<String, IoError> / net;
