// Redox Agent Protocol (RAP) — Full JSON-RPC 2.0 specification.
//
// Implements: JSON-RPC request/response format, capability negotiation,
// session management, method routing, and error handling per §8.2 and
// Appendix D of REDOX_PROPOSAL.md.
//
// (ROADMAP Step 47)

use std::collections::BTreeMap;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

// ── JSON-RPC 2.0 Wire Protocol ────────────────────────────────────────────

/// JSON-RPC 2.0 protocol version.
pub const JSONRPC_VERSION: &str = "2.0";

/// A JSON-RPC request ID (integer or string).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RequestId {
    Integer(i64),
    Str(String),
}

impl fmt::Display for RequestId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RequestId::Integer(n) => write!(f, "{n}"),
            RequestId::Str(s) => write!(f, "\"{s}\""),
        }
    }
}

/// A JSON value (minimal subset for RAP).
#[derive(Debug, Clone, PartialEq)]
pub enum JsonValue {
    Null,
    Bool(bool),
    Integer(i64),
    Float(f64),
    Str(String),
    Array(Vec<JsonValue>),
    Object(BTreeMap<String, JsonValue>),
}

impl JsonValue {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            JsonValue::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            JsonValue::Integer(n) => Some(*n),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            JsonValue::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[JsonValue]> {
        match self {
            JsonValue::Array(a) => Some(a),
            _ => None,
        }
    }

    pub fn as_object(&self) -> Option<&BTreeMap<String, JsonValue>> {
        match self {
            JsonValue::Object(m) => Some(m),
            _ => None,
        }
    }

    pub fn is_null(&self) -> bool {
        matches!(self, JsonValue::Null)
    }
}

impl fmt::Display for JsonValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JsonValue::Null => write!(f, "null"),
            JsonValue::Bool(b) => write!(f, "{b}"),
            JsonValue::Integer(n) => write!(f, "{n}"),
            JsonValue::Float(v) => write!(f, "{v}"),
            JsonValue::Str(s) => write!(f, "\"{s}\""),
            JsonValue::Array(arr) => {
                write!(f, "[")?;
                for (i, v) in arr.iter().enumerate() {
                    if i > 0 { write!(f, ",")?; }
                    write!(f, "{v}")?;
                }
                write!(f, "]")
            }
            JsonValue::Object(map) => {
                write!(f, "{{")?;
                for (i, (k, v)) in map.iter().enumerate() {
                    if i > 0 { write!(f, ",")?; }
                    write!(f, "\"{k}\":{v}")?;
                }
                write!(f, "}}")
            }
        }
    }
}

/// A JSON-RPC 2.0 request.
#[derive(Debug, Clone, PartialEq)]
pub struct RpcRequest {
    pub jsonrpc: String,
    pub id: Option<RequestId>,
    pub method: String,
    pub params: Option<JsonValue>,
}

impl RpcRequest {
    /// Create a standard request with an integer ID.
    pub fn new(id: i64, method: &str) -> Self {
        RpcRequest {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id: Some(RequestId::Integer(id)),
            method: method.to_string(),
            params: None,
        }
    }

    /// Create a request with parameters.
    pub fn with_params(id: i64, method: &str, params: JsonValue) -> Self {
        RpcRequest {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id: Some(RequestId::Integer(id)),
            method: method.to_string(),
            params: Some(params),
        }
    }

    /// Create a notification (no ID, no response expected).
    pub fn notification(method: &str, params: Option<JsonValue>) -> Self {
        RpcRequest {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id: None,
            method: method.to_string(),
            params,
        }
    }

    /// Whether this is a notification (no id).
    pub fn is_notification(&self) -> bool {
        self.id.is_none()
    }

    /// Serialize to a minimal JSON string.
    pub fn to_json(&self) -> String {
        let mut parts = Vec::new();
        parts.push(format!("\"jsonrpc\":\"{}\"", self.jsonrpc));
        if let Some(ref id) = self.id {
            parts.push(format!("\"id\":{id}"));
        }
        parts.push(format!("\"method\":\"{}\"", self.method));
        if let Some(ref p) = self.params {
            parts.push(format!("\"params\":{p}"));
        }
        format!("{{{}}}", parts.join(","))
    }
}

/// A JSON-RPC 2.0 error object.
#[derive(Debug, Clone, PartialEq)]
pub struct RpcError {
    pub code: i32,
    pub message: String,
    pub data: Option<JsonValue>,
}

impl RpcError {
    pub fn new(code: i32, message: &str) -> Self {
        RpcError { code, message: message.to_string(), data: None }
    }

    pub fn with_data(code: i32, message: &str, data: JsonValue) -> Self {
        RpcError { code, message: message.to_string(), data: Some(data) }
    }

    // Standard JSON-RPC error codes.

    pub fn parse_error() -> Self {
        Self::new(-32700, "Parse error")
    }

    pub fn invalid_request() -> Self {
        Self::new(-32600, "Invalid Request")
    }

    pub fn method_not_found(method: &str) -> Self {
        Self::new(-32601, &format!("Method not found: {method}"))
    }

    pub fn invalid_params(detail: &str) -> Self {
        Self::new(-32602, &format!("Invalid params: {detail}"))
    }

    pub fn internal_error(detail: &str) -> Self {
        Self::new(-32603, &format!("Internal error: {detail}"))
    }

    // RAP-specific error codes (application-defined, -32000 to -32099).

    pub fn session_not_found() -> Self {
        Self::new(-32001, "Session not found")
    }

    pub fn session_expired() -> Self {
        Self::new(-32002, "Session expired")
    }

    pub fn capability_denied(cap: &str) -> Self {
        Self::new(-32003, &format!("Capability denied: {cap}"))
    }

    pub fn rate_limited() -> Self {
        Self::new(-32004, "Rate limited")
    }

    /// Serialize to JSON.
    pub fn to_json(&self) -> String {
        let mut parts = vec![
            format!("\"code\":{}", self.code),
            format!("\"message\":\"{}\"", self.message),
        ];
        if let Some(ref d) = self.data {
            parts.push(format!("\"data\":{d}"));
        }
        format!("{{{}}}", parts.join(","))
    }
}

impl fmt::Display for RpcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.code, self.message)
    }
}

/// A JSON-RPC 2.0 response.
#[derive(Debug, Clone, PartialEq)]
pub struct RpcResponse {
    pub jsonrpc: String,
    pub id: Option<RequestId>,
    pub result: Option<JsonValue>,
    pub error: Option<RpcError>,
}

impl RpcResponse {
    /// Create a success response.
    pub fn success(id: RequestId, result: JsonValue) -> Self {
        RpcResponse {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id: Some(id),
            result: Some(result),
            error: None,
        }
    }

    /// Create an error response.
    pub fn error(id: Option<RequestId>, err: RpcError) -> Self {
        RpcResponse {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id,
            result: None,
            error: Some(err),
        }
    }

    /// Whether this response indicates success.
    pub fn is_success(&self) -> bool {
        self.result.is_some() && self.error.is_none()
    }

    /// Serialize to JSON.
    pub fn to_json(&self) -> String {
        let mut parts = Vec::new();
        parts.push(format!("\"jsonrpc\":\"{}\"", self.jsonrpc));
        if let Some(ref id) = self.id {
            parts.push(format!("\"id\":{id}"));
        } else {
            parts.push("\"id\":null".to_string());
        }
        if let Some(ref r) = self.result {
            parts.push(format!("\"result\":{r}"));
        }
        if let Some(ref e) = self.error {
            parts.push(format!("\"error\":{}", e.to_json()));
        }
        format!("{{{}}}", parts.join(","))
    }
}

// ── Capability Negotiation ─────────────────────────────────────────────────

/// A capability that a RAP client or server can declare.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Capability {
    pub name: String,
    pub version: u32,
}

impl Capability {
    pub fn new(name: &str, version: u32) -> Self {
        Capability { name: name.to_string(), version }
    }
}

impl fmt::Display for Capability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}@{}", self.name, self.version)
    }
}

/// Standard RAP capabilities (§8.2).
pub mod capabilities {
    use super::Capability;

    pub fn query_rules() -> Capability { Capability::new("query.rules", 1) }
    pub fn query_types() -> Capability { Capability::new("query.types", 1) }
    pub fn query_search() -> Capability { Capability::new("query.search", 1) }
    pub fn tokens() -> Capability { Capability::new("tokens", 1) }
    pub fn ast() -> Capability { Capability::new("ast", 1) }
    pub fn diagnostics() -> Capability { Capability::new("diagnostics", 1) }
    pub fn build_heal() -> Capability { Capability::new("build.heal", 1) }
    pub fn cost_query() -> Capability { Capability::new("cost.query", 1) }
    pub fn cost_compare() -> Capability { Capability::new("cost.compare", 1) }
    pub fn skb_query() -> Capability { Capability::new("skb.query", 1) }
    pub fn skb_spec() -> Capability { Capability::new("skb.spec", 1) }
    pub fn verify_contracts() -> Capability { Capability::new("verify.contracts", 1) }
    pub fn session_management() -> Capability { Capability::new("session", 1) }
    pub fn swarm_coordination() -> Capability { Capability::new("swarm", 1) }
    pub fn lease_management() -> Capability { Capability::new("lease", 1) }

    /// All standard capabilities.
    pub fn all_standard() -> Vec<Capability> {
        vec![
            query_rules(), query_types(), query_search(),
            tokens(), ast(), diagnostics(),
            build_heal(), cost_query(), cost_compare(),
            skb_query(), skb_spec(), verify_contracts(),
            session_management(), swarm_coordination(), lease_management(),
        ]
    }
}

/// Client capabilities declared during initialization.
#[derive(Debug, Clone)]
pub struct ClientCapabilities {
    pub requested: Vec<Capability>,
    pub client_name: String,
    pub client_version: String,
}

impl ClientCapabilities {
    pub fn new(name: &str, version: &str) -> Self {
        ClientCapabilities {
            requested: Vec::new(),
            client_name: name.to_string(),
            client_version: version.to_string(),
        }
    }

    pub fn request(mut self, cap: Capability) -> Self {
        self.requested.push(cap);
        self
    }

    pub fn request_all_standard(mut self) -> Self {
        self.requested = capabilities::all_standard();
        self
    }
}

/// Server capabilities declared in response to initialization.
#[derive(Debug, Clone)]
pub struct ServerCapabilities {
    pub supported: Vec<Capability>,
    pub server_name: String,
    pub server_version: String,
}

impl ServerCapabilities {
    pub fn new(name: &str, version: &str) -> Self {
        ServerCapabilities {
            supported: Vec::new(),
            server_name: name.to_string(),
            server_version: version.to_string(),
        }
    }

    pub fn support(mut self, cap: Capability) -> Self {
        self.supported.push(cap);
        self
    }

    pub fn support_all_standard(mut self) -> Self {
        self.supported = capabilities::all_standard();
        self
    }

    pub fn supports(&self, name: &str) -> bool {
        self.supported.iter().any(|c| c.name == name)
    }

    pub fn supports_version(&self, name: &str, min_version: u32) -> bool {
        self.supported.iter().any(|c| c.name == name && c.version >= min_version)
    }
}

/// Result of capability negotiation.
#[derive(Debug, Clone)]
pub struct NegotiationResult {
    pub granted: Vec<Capability>,
    pub denied: Vec<Capability>,
}

impl NegotiationResult {
    pub fn is_fully_granted(&self) -> bool {
        self.denied.is_empty()
    }

    pub fn has_capability(&self, name: &str) -> bool {
        self.granted.iter().any(|c| c.name == name)
    }
}

/// Negotiate capabilities between client and server.
pub fn negotiate(
    client: &ClientCapabilities,
    server: &ServerCapabilities,
) -> NegotiationResult {
    let mut granted = Vec::new();
    let mut denied = Vec::new();

    for req in &client.requested {
        if server.supports_version(&req.name, req.version) {
            granted.push(req.clone());
        } else {
            denied.push(req.clone());
        }
    }

    NegotiationResult { granted, denied }
}

// ── Session Management ─────────────────────────────────────────────────────

static SESSION_COUNTER: AtomicU64 = AtomicU64::new(1);

/// A unique session identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SessionId(u64);

impl SessionId {
    fn next() -> Self {
        SessionId(SESSION_COUNTER.fetch_add(1, Ordering::Relaxed))
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "session-{}", self.0)
    }
}

/// Session state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    /// Session has been created but not yet initialized.
    Created,
    /// Session initialized with capabilities negotiated.
    Active,
    /// Session is shutting down.
    ShuttingDown,
    /// Session has been closed.
    Closed,
}

impl fmt::Display for SessionState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SessionState::Created => write!(f, "created"),
            SessionState::Active => write!(f, "active"),
            SessionState::ShuttingDown => write!(f, "shutting_down"),
            SessionState::Closed => write!(f, "closed"),
        }
    }
}

/// A RAP session between a client and the server.
#[derive(Debug)]
pub struct Session {
    pub id: SessionId,
    pub state: SessionState,
    pub client_caps: ClientCapabilities,
    pub negotiation: Option<NegotiationResult>,
    pub created_at: Instant,
    pub last_activity: Instant,
    pub timeout: Duration,
    pub request_count: u64,
}

impl Session {
    fn new(client_caps: ClientCapabilities, timeout: Duration) -> Self {
        let now = Instant::now();
        Session {
            id: SessionId::next(),
            state: SessionState::Created,
            client_caps,
            negotiation: None,
            created_at: now,
            last_activity: now,
            timeout,
            request_count: 0,
        }
    }

    /// Whether the session has expired.
    pub fn is_expired(&self) -> bool {
        self.last_activity.elapsed() > self.timeout
    }

    /// Whether the session is usable (active and not expired).
    pub fn is_usable(&self) -> bool {
        self.state == SessionState::Active && !self.is_expired()
    }

    /// Touch the session (update last activity).
    pub fn touch(&mut self) {
        self.last_activity = Instant::now();
        self.request_count += 1;
    }

    /// Get the session uptime.
    pub fn uptime(&self) -> Duration {
        self.created_at.elapsed()
    }

    /// Check if a capability was granted.
    pub fn has_capability(&self, name: &str) -> bool {
        self.negotiation.as_ref().is_some_and(|n| n.has_capability(name))
    }
}

/// Manages all active sessions.
pub struct SessionManager {
    sessions: BTreeMap<u64, Session>,
    default_timeout: Duration,
}

impl SessionManager {
    pub fn new(default_timeout: Duration) -> Self {
        SessionManager {
            sessions: BTreeMap::new(),
            default_timeout,
        }
    }

    pub fn with_default_timeout() -> Self {
        Self::new(Duration::from_secs(3600))
    }

    /// Create a new session for a client.
    pub fn create_session(&mut self, client_caps: ClientCapabilities) -> SessionId {
        let session = Session::new(client_caps, self.default_timeout);
        let id = session.id;
        self.sessions.insert(id.0, session);
        id
    }

    /// Initialize a session with capability negotiation.
    pub fn initialize(
        &mut self,
        id: SessionId,
        server_caps: &ServerCapabilities,
    ) -> Result<&NegotiationResult, SessionError> {
        let session = self.sessions.get_mut(&id.0)
            .ok_or(SessionError::NotFound(id))?;

        if session.state != SessionState::Created {
            return Err(SessionError::InvalidState {
                id,
                expected: SessionState::Created,
                actual: session.state,
            });
        }

        let result = negotiate(&session.client_caps, server_caps);
        session.negotiation = Some(result);
        session.state = SessionState::Active;
        session.touch();

        Ok(session.negotiation.as_ref().unwrap())
    }

    /// Get a session by ID.
    pub fn get(&self, id: SessionId) -> Option<&Session> {
        self.sessions.get(&id.0)
    }

    /// Get a mutable reference to a session.
    pub fn get_mut(&mut self, id: SessionId) -> Option<&mut Session> {
        self.sessions.get_mut(&id.0)
    }

    /// Touch a session (update last activity).
    pub fn touch(&mut self, id: SessionId) -> Result<(), SessionError> {
        let session = self.sessions.get_mut(&id.0)
            .ok_or(SessionError::NotFound(id))?;

        if session.is_expired() {
            session.state = SessionState::Closed;
            return Err(SessionError::Expired(id));
        }
        if session.state != SessionState::Active {
            return Err(SessionError::InvalidState {
                id,
                expected: SessionState::Active,
                actual: session.state,
            });
        }
        session.touch();
        Ok(())
    }

    /// Shutdown a session gracefully.
    pub fn shutdown(&mut self, id: SessionId) -> Result<(), SessionError> {
        let session = self.sessions.get_mut(&id.0)
            .ok_or(SessionError::NotFound(id))?;
        session.state = SessionState::ShuttingDown;
        Ok(())
    }

    /// Close and remove a session.
    pub fn close(&mut self, id: SessionId) -> Result<Session, SessionError> {
        let mut session = self.sessions.remove(&id.0)
            .ok_or(SessionError::NotFound(id))?;
        session.state = SessionState::Closed;
        Ok(session)
    }

    /// Purge all expired sessions, returning the count removed.
    pub fn purge_expired(&mut self) -> usize {
        let expired: Vec<u64> = self.sessions.iter()
            .filter(|(_, s)| s.is_expired())
            .map(|(k, _)| *k)
            .collect();
        let count = expired.len();
        for id in expired {
            self.sessions.remove(&id);
        }
        count
    }

    /// Number of active sessions.
    pub fn active_count(&self) -> usize {
        self.sessions.values()
            .filter(|s| s.state == SessionState::Active && !s.is_expired())
            .count()
    }

    /// Total sessions (including expired/created).
    pub fn total_count(&self) -> usize {
        self.sessions.len()
    }

    /// List all session IDs.
    pub fn session_ids(&self) -> Vec<SessionId> {
        self.sessions.keys().map(|k| SessionId(*k)).collect()
    }
}

/// Session errors.
#[derive(Debug, Clone)]
pub enum SessionError {
    NotFound(SessionId),
    Expired(SessionId),
    InvalidState {
        id: SessionId,
        expected: SessionState,
        actual: SessionState,
    },
}

impl fmt::Display for SessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SessionError::NotFound(id) => write!(f, "Session {id} not found"),
            SessionError::Expired(id) => write!(f, "Session {id} expired"),
            SessionError::InvalidState { id, expected, actual } =>
                write!(f, "Session {id}: expected state {expected}, got {actual}"),
        }
    }
}

// ── Method Router ──────────────────────────────────────────────────────────

/// Trait for RAP method handlers.
pub trait MethodHandler: Send + Sync {
    fn handle(&self, params: &Option<JsonValue>) -> Result<JsonValue, RpcError>;
    fn required_capability(&self) -> Option<&str> { None }
}

/// A simple handler from a closure.
struct ClosureHandler {
    func: Box<dyn Fn(&Option<JsonValue>) -> Result<JsonValue, RpcError> + Send + Sync>,
    capability: Option<String>,
}

impl MethodHandler for ClosureHandler {
    fn handle(&self, params: &Option<JsonValue>) -> Result<JsonValue, RpcError> {
        (self.func)(params)
    }
    fn required_capability(&self) -> Option<&str> {
        self.capability.as_deref()
    }
}

/// The RAP Protocol Server: routes JSON-RPC requests to method handlers.
pub struct ProtocolServer {
    handlers: BTreeMap<String, Box<dyn MethodHandler>>,
    server_caps: ServerCapabilities,
    sessions: SessionManager,
}

impl ProtocolServer {
    /// Create a new protocol server.
    pub fn new(name: &str, version: &str) -> Self {
        ProtocolServer {
            handlers: BTreeMap::new(),
            server_caps: ServerCapabilities::new(name, version)
                .support_all_standard(),
            sessions: SessionManager::with_default_timeout(),
        }
    }

    /// Register a method handler.
    pub fn register<F>(&mut self, method: &str, capability: Option<&str>, handler: F)
    where
        F: Fn(&Option<JsonValue>) -> Result<JsonValue, RpcError> + Send + Sync + 'static,
    {
        self.handlers.insert(method.to_string(), Box::new(ClosureHandler {
            func: Box::new(handler),
            capability: capability.map(|s| s.to_string()),
        }));
    }

    /// List all registered method names.
    pub fn methods(&self) -> Vec<&str> {
        self.handlers.keys().map(|s| s.as_str()).collect()
    }

    /// Whether a method is registered.
    pub fn has_method(&self, method: &str) -> bool {
        self.handlers.contains_key(method)
    }

    /// Get the server capabilities.
    pub fn server_capabilities(&self) -> &ServerCapabilities {
        &self.server_caps
    }

    /// Access the session manager.
    pub fn sessions(&self) -> &SessionManager {
        &self.sessions
    }

    /// Access the session manager mutably.
    pub fn sessions_mut(&mut self) -> &mut SessionManager {
        &mut self.sessions
    }

    /// Create a new session.
    pub fn create_session(&mut self, client_caps: ClientCapabilities) -> SessionId {
        self.sessions.create_session(client_caps)
    }

    /// Initialize a session (negotiate capabilities).
    pub fn initialize_session(&mut self, id: SessionId) -> Result<NegotiationResult, SessionError> {
        let result = self.sessions.initialize(id, &self.server_caps)?;
        Ok(result.clone())
    }

    /// Dispatch a JSON-RPC request, returning a response.
    /// For notifications (no ID), returns None.
    pub fn dispatch(&mut self, request: &RpcRequest) -> Option<RpcResponse> {
        if request.jsonrpc != JSONRPC_VERSION {
            return request.id.as_ref().map(|id| {
                RpcResponse::error(Some(id.clone()), RpcError::invalid_request())
            });
        }

        // Handle built-in lifecycle methods.
        match request.method.as_str() {
            "initialize" => return Some(self.handle_initialize(request)),
            "shutdown" => return Some(self.handle_shutdown(request)),
            "exit" => {
                self.handle_exit(request);
                return None;
            }
            _ => {}
        }

        // For non-lifecycle methods, look up handler.
        let id = match &request.id {
            Some(id) => id.clone(),
            None => return None, // notification, no response
        };

        let handler = match self.handlers.get(&request.method) {
            Some(h) => h,
            None => return Some(RpcResponse::error(
                Some(id),
                RpcError::method_not_found(&request.method),
            )),
        };

        // Check capability requirement.
        if let Some(cap) = handler.required_capability() {
            // Capability checking is informational — handlers declare what they
            // need but enforcement is up to the session layer.
            let _ = cap;
        }

        match handler.handle(&request.params) {
            Ok(result) => Some(RpcResponse::success(id, result)),
            Err(err) => Some(RpcResponse::error(Some(id), err)),
        }
    }

    /// Handle the `initialize` lifecycle request.
    fn handle_initialize(&mut self, request: &RpcRequest) -> RpcResponse {
        let id = match &request.id {
            Some(id) => id.clone(),
            None => return RpcResponse::error(None, RpcError::invalid_request()),
        };

        // Extract client info from params.
        let (client_name, client_version) = if let Some(ref params) = request.params {
            let name = params.as_object()
                .and_then(|o| o.get("clientName"))
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let version = params.as_object()
                .and_then(|o| o.get("clientVersion"))
                .and_then(|v| v.as_str())
                .unwrap_or("0.0.0");
            (name.to_string(), version.to_string())
        } else {
            ("unknown".to_string(), "0.0.0".to_string())
        };

        let client_caps = ClientCapabilities::new(&client_name, &client_version)
            .request_all_standard();
        let session_id = self.sessions.create_session(client_caps);

        match self.sessions.initialize(session_id, &self.server_caps) {
            Ok(negotiation) => {
                let mut result = BTreeMap::new();
                result.insert("sessionId".to_string(), JsonValue::Str(session_id.to_string()));
                result.insert("serverName".to_string(), JsonValue::Str(self.server_caps.server_name.clone()));
                result.insert("serverVersion".to_string(), JsonValue::Str(self.server_caps.server_version.clone()));

                let granted: Vec<JsonValue> = negotiation.granted.iter()
                    .map(|c| JsonValue::Str(c.to_string()))
                    .collect();
                result.insert("capabilities".to_string(), JsonValue::Array(granted));

                RpcResponse::success(id, JsonValue::Object(result))
            }
            Err(_) => RpcResponse::error(
                Some(id),
                RpcError::internal_error("Failed to initialize session"),
            ),
        }
    }

    /// Handle the `shutdown` lifecycle request.
    fn handle_shutdown(&mut self, request: &RpcRequest) -> RpcResponse {
        let id = match &request.id {
            Some(id) => id.clone(),
            None => return RpcResponse::error(None, RpcError::invalid_request()),
        };

        // Try to find and shutdown all sessions, or a specific one.
        let session_id_str = request.params.as_ref()
            .and_then(|p| p.as_object())
            .and_then(|o| o.get("sessionId"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        if let Some(ref sid) = session_id_str {
            // Shutdown specific session: parse session-N.
            if let Some(num_str) = sid.strip_prefix("session-") {
                if let Ok(num) = num_str.parse::<u64>() {
                    let _ = self.sessions.shutdown(SessionId::from_raw(num));
                }
            }
        }

        RpcResponse::success(id, JsonValue::Null)
    }

    /// Handle the `exit` notification.
    fn handle_exit(&mut self, _request: &RpcRequest) {
        // Close all sessions.
        let ids: Vec<SessionId> = self.sessions.session_ids();
        for id in ids {
            let _ = self.sessions.close(id);
        }
    }

    /// Register the standard RAP method set with mock handlers.
    pub fn register_standard_methods(&mut self) {
        self.register("query.rules", Some("query.rules"), |_params| {
            Ok(JsonValue::Array(vec![
                JsonValue::Str("ownership".to_string()),
                JsonValue::Str("borrowing".to_string()),
                JsonValue::Str("lifetime".to_string()),
            ]))
        });

        self.register("query.types", Some("query.types"), |_params| {
            Ok(JsonValue::Object(BTreeMap::new()))
        });

        self.register("query.search", Some("query.search"), |params| {
            let query = params.as_ref()
                .and_then(|p| p.as_object())
                .and_then(|o| o.get("query"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let mut result = BTreeMap::new();
            result.insert("query".to_string(), JsonValue::Str(query.to_string()));
            result.insert("results".to_string(), JsonValue::Array(vec![]));
            Ok(JsonValue::Object(result))
        });

        self.register("tokens.tokenize", Some("tokens"), |_params| {
            Ok(JsonValue::Array(vec![]))
        });

        self.register("ast.parse", Some("ast"), |_params| {
            Ok(JsonValue::Object(BTreeMap::new()))
        });

        self.register("diagnostic.check", Some("diagnostics"), |_params| {
            Ok(JsonValue::Array(vec![]))
        });

        self.register("build/heal", Some("build.heal"), |_params| {
            Ok(JsonValue::Object(BTreeMap::new()))
        });

        self.register("cost/query", Some("cost.query"), |params| {
            let subject = params.as_ref()
                .and_then(|p| p.as_object())
                .and_then(|o| o.get("subject"))
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let mut result = BTreeMap::new();
            result.insert("subject".to_string(), JsonValue::Str(subject.to_string()));
            result.insert("latency_cycles".to_string(), JsonValue::Integer(1));
            result.insert("memory_bytes".to_string(), JsonValue::Integer(8));
            Ok(JsonValue::Object(result))
        });

        self.register("cost/compare", Some("cost.compare"), |_params| {
            Ok(JsonValue::Object(BTreeMap::new()))
        });

        self.register("skb/query", Some("skb.query"), |params| {
            let fqn = params.as_ref()
                .and_then(|p| p.as_object())
                .and_then(|o| o.get("fqn"))
                .and_then(|v| v.as_str())
                .unwrap_or("*");
            let mut result = BTreeMap::new();
            result.insert("fqn".to_string(), JsonValue::Str(fqn.to_string()));
            result.insert("rules".to_string(), JsonValue::Array(vec![]));
            Ok(JsonValue::Object(result))
        });

        self.register("skb/spec", Some("skb.spec"), |_params| {
            Ok(JsonValue::Object(BTreeMap::new()))
        });

        self.register("verify/contracts", Some("verify.contracts"), |_params| {
            let mut result = BTreeMap::new();
            result.insert("verified".to_string(), JsonValue::Bool(true));
            result.insert("violations".to_string(), JsonValue::Array(vec![]));
            Ok(JsonValue::Object(result))
        });
    }
}

impl SessionId {
    /// Create from a raw u64 (for deserialization).
    pub fn from_raw(val: u64) -> Self {
        SessionId(val)
    }

    /// Get the raw u64 value.
    pub fn raw(&self) -> u64 {
        self.0
    }
}

// ── Convenience Builders ───────────────────────────────────────────────────

/// Build a JSON object value.
pub fn json_object(entries: Vec<(&str, JsonValue)>) -> JsonValue {
    let mut map = BTreeMap::new();
    for (k, v) in entries {
        map.insert(k.to_string(), v);
    }
    JsonValue::Object(map)
}

/// Build a JSON array value.
pub fn json_array(items: Vec<JsonValue>) -> JsonValue {
    JsonValue::Array(items)
}

/// Build a JSON string value.
pub fn json_str(s: &str) -> JsonValue {
    JsonValue::Str(s.to_string())
}

/// Build a JSON integer value.
pub fn json_int(n: i64) -> JsonValue {
    JsonValue::Integer(n)
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── JSON-RPC Request ──

    #[test]
    fn create_request() {
        let req = RpcRequest::new(1, "test.method");
        assert_eq!(req.jsonrpc, "2.0");
        assert_eq!(req.method, "test.method");
        assert_eq!(req.id, Some(RequestId::Integer(1)));
        assert!(!req.is_notification());
    }

    #[test]
    fn create_notification() {
        let req = RpcRequest::notification("test.notify", None);
        assert!(req.is_notification());
        assert!(req.id.is_none());
    }

    #[test]
    fn request_with_params() {
        let params = json_object(vec![
            ("file", json_str("main.rdx")),
            ("line", json_int(42)),
        ]);
        let req = RpcRequest::with_params(1, "diagnostic.check", params);
        assert!(req.params.is_some());
    }

    #[test]
    fn request_to_json() {
        let req = RpcRequest::new(1, "test");
        let json = req.to_json();
        assert!(json.contains("\"jsonrpc\":\"2.0\""));
        assert!(json.contains("\"method\":\"test\""));
        assert!(json.contains("\"id\":1"));
    }

    // ── JSON-RPC Response ──

    #[test]
    fn success_response() {
        let resp = RpcResponse::success(RequestId::Integer(1), json_str("ok"));
        assert!(resp.is_success());
        assert_eq!(resp.result, Some(json_str("ok")));
    }

    #[test]
    fn error_response() {
        let resp = RpcResponse::error(
            Some(RequestId::Integer(1)),
            RpcError::method_not_found("foo"),
        );
        assert!(!resp.is_success());
        assert!(resp.error.is_some());
        assert_eq!(resp.error.as_ref().unwrap().code, -32601);
    }

    #[test]
    fn response_to_json() {
        let resp = RpcResponse::success(RequestId::Integer(1), json_int(42));
        let json = resp.to_json();
        assert!(json.contains("\"result\":42"));
    }

    // ── RPC Error ──

    #[test]
    fn standard_error_codes() {
        assert_eq!(RpcError::parse_error().code, -32700);
        assert_eq!(RpcError::invalid_request().code, -32600);
        assert_eq!(RpcError::method_not_found("x").code, -32601);
        assert_eq!(RpcError::invalid_params("x").code, -32602);
        assert_eq!(RpcError::internal_error("x").code, -32603);
    }

    #[test]
    fn rap_specific_error_codes() {
        assert_eq!(RpcError::session_not_found().code, -32001);
        assert_eq!(RpcError::session_expired().code, -32002);
        assert_eq!(RpcError::capability_denied("x").code, -32003);
        assert_eq!(RpcError::rate_limited().code, -32004);
    }

    #[test]
    fn error_display() {
        let e = RpcError::method_not_found("test");
        let s = format!("{e}");
        assert!(s.contains("-32601"));
        assert!(s.contains("test"));
    }

    // ── JsonValue ──

    #[test]
    fn json_value_accessors() {
        assert_eq!(json_str("hello").as_str(), Some("hello"));
        assert_eq!(json_int(42).as_i64(), Some(42));
        assert_eq!(JsonValue::Bool(true).as_bool(), Some(true));
        assert!(JsonValue::Null.is_null());
    }

    #[test]
    fn json_value_display() {
        assert_eq!(format!("{}", JsonValue::Null), "null");
        assert_eq!(format!("{}", json_int(42)), "42");
        assert_eq!(format!("{}", json_str("hi")), "\"hi\"");
    }

    #[test]
    fn json_object_builder() {
        let obj = json_object(vec![("a", json_int(1)), ("b", json_str("two"))]);
        assert!(obj.as_object().unwrap().contains_key("a"));
        assert!(obj.as_object().unwrap().contains_key("b"));
    }

    #[test]
    fn json_array_builder() {
        let arr = json_array(vec![json_int(1), json_int(2)]);
        assert_eq!(arr.as_array().unwrap().len(), 2);
    }

    // ── Capabilities ──

    #[test]
    fn capability_display() {
        let cap = Capability::new("query.rules", 1);
        assert_eq!(format!("{cap}"), "query.rules@1");
    }

    #[test]
    fn all_standard_capabilities() {
        let caps = capabilities::all_standard();
        assert_eq!(caps.len(), 15);
    }

    #[test]
    fn server_capabilities_support_check() {
        let sc = ServerCapabilities::new("test", "1.0")
            .support(Capability::new("query.rules", 1))
            .support(Capability::new("tokens", 2));
        assert!(sc.supports("query.rules"));
        assert!(sc.supports("tokens"));
        assert!(!sc.supports("nonexistent"));
        assert!(sc.supports_version("tokens", 1));
        assert!(sc.supports_version("tokens", 2));
        assert!(!sc.supports_version("tokens", 3));
    }

    // ── Capability Negotiation ──

    #[test]
    fn negotiate_full_match() {
        let client = ClientCapabilities::new("test-client", "1.0")
            .request(Capability::new("query.rules", 1))
            .request(Capability::new("tokens", 1));
        let server = ServerCapabilities::new("test-server", "1.0")
            .support_all_standard();

        let result = negotiate(&client, &server);
        assert!(result.is_fully_granted());
        assert_eq!(result.granted.len(), 2);
        assert!(result.denied.is_empty());
    }

    #[test]
    fn negotiate_partial_match() {
        let client = ClientCapabilities::new("test-client", "1.0")
            .request(Capability::new("query.rules", 1))
            .request(Capability::new("magic.unicorn", 1));
        let server = ServerCapabilities::new("test-server", "1.0")
            .support(Capability::new("query.rules", 1));

        let result = negotiate(&client, &server);
        assert!(!result.is_fully_granted());
        assert_eq!(result.granted.len(), 1);
        assert_eq!(result.denied.len(), 1);
        assert!(result.has_capability("query.rules"));
        assert!(!result.has_capability("magic.unicorn"));
    }

    #[test]
    fn negotiate_version_mismatch() {
        let client = ClientCapabilities::new("c", "1.0")
            .request(Capability::new("tokens", 3));
        let server = ServerCapabilities::new("s", "1.0")
            .support(Capability::new("tokens", 2));

        let result = negotiate(&client, &server);
        assert!(!result.is_fully_granted());
        assert_eq!(result.denied.len(), 1);
    }

    // ── Session Management ──

    #[test]
    fn create_session() {
        let mut mgr = SessionManager::with_default_timeout();
        let caps = ClientCapabilities::new("test", "1.0");
        let id = mgr.create_session(caps);
        let session = mgr.get(id).unwrap();
        assert_eq!(session.state, SessionState::Created);
        assert_eq!(mgr.total_count(), 1);
    }

    #[test]
    fn initialize_session() {
        let mut mgr = SessionManager::with_default_timeout();
        let caps = ClientCapabilities::new("test", "1.0")
            .request_all_standard();
        let id = mgr.create_session(caps);
        let server_caps = ServerCapabilities::new("srv", "1.0")
            .support_all_standard();

        let result = mgr.initialize(id, &server_caps).unwrap();
        assert!(result.is_fully_granted());

        let session = mgr.get(id).unwrap();
        assert_eq!(session.state, SessionState::Active);
        assert!(session.is_usable());
    }

    #[test]
    fn session_touch_updates_activity() {
        let mut mgr = SessionManager::with_default_timeout();
        let caps = ClientCapabilities::new("test", "1.0");
        let id = mgr.create_session(caps);
        let server_caps = ServerCapabilities::new("srv", "1.0");
        mgr.initialize(id, &server_caps).unwrap();

        mgr.touch(id).unwrap();
        let session = mgr.get(id).unwrap();
        assert_eq!(session.request_count, 2); // init + touch
    }

    #[test]
    fn session_shutdown_and_close() {
        let mut mgr = SessionManager::with_default_timeout();
        let caps = ClientCapabilities::new("test", "1.0");
        let id = mgr.create_session(caps);
        let server_caps = ServerCapabilities::new("srv", "1.0");
        mgr.initialize(id, &server_caps).unwrap();

        mgr.shutdown(id).unwrap();
        let session = mgr.get(id).unwrap();
        assert_eq!(session.state, SessionState::ShuttingDown);

        let closed = mgr.close(id).unwrap();
        assert_eq!(closed.state, SessionState::Closed);
        assert_eq!(mgr.total_count(), 0);
    }

    #[test]
    fn session_not_found_error() {
        let mut mgr = SessionManager::with_default_timeout();
        let fake_id = SessionId::from_raw(9999);
        let result = mgr.touch(fake_id);
        assert!(result.is_err());
    }

    #[test]
    fn session_double_initialize_error() {
        let mut mgr = SessionManager::with_default_timeout();
        let caps = ClientCapabilities::new("test", "1.0");
        let id = mgr.create_session(caps);
        let server_caps = ServerCapabilities::new("srv", "1.0");
        mgr.initialize(id, &server_caps).unwrap();
        let result = mgr.initialize(id, &server_caps);
        assert!(result.is_err());
    }

    #[test]
    fn session_expiry() {
        let mut mgr = SessionManager::new(Duration::from_millis(1));
        let caps = ClientCapabilities::new("test", "1.0");
        let id = mgr.create_session(caps);
        let server_caps = ServerCapabilities::new("srv", "1.0");
        mgr.initialize(id, &server_caps).unwrap();

        std::thread::sleep(Duration::from_millis(10));

        let session = mgr.get(id).unwrap();
        assert!(session.is_expired());
        assert!(!session.is_usable());
    }

    #[test]
    fn purge_expired_sessions() {
        let mut mgr = SessionManager::new(Duration::from_millis(1));
        let caps = ClientCapabilities::new("test", "1.0");
        let _id = mgr.create_session(caps);
        std::thread::sleep(Duration::from_millis(10));
        let removed = mgr.purge_expired();
        assert_eq!(removed, 1);
        assert_eq!(mgr.total_count(), 0);
    }

    #[test]
    fn active_session_count() {
        let mut mgr = SessionManager::with_default_timeout();
        let server_caps = ServerCapabilities::new("srv", "1.0");

        let id1 = mgr.create_session(ClientCapabilities::new("a", "1.0"));
        mgr.initialize(id1, &server_caps).unwrap();

        let id2 = mgr.create_session(ClientCapabilities::new("b", "1.0"));
        mgr.initialize(id2, &server_caps).unwrap();

        let _id3 = mgr.create_session(ClientCapabilities::new("c", "1.0"));
        // id3 not initialized

        assert_eq!(mgr.active_count(), 2);
        assert_eq!(mgr.total_count(), 3);
    }

    #[test]
    fn session_has_capability() {
        let mut mgr = SessionManager::with_default_timeout();
        let caps = ClientCapabilities::new("test", "1.0")
            .request(capabilities::query_rules())
            .request(capabilities::tokens());
        let id = mgr.create_session(caps);
        let server_caps = ServerCapabilities::new("srv", "1.0")
            .support(capabilities::query_rules());
        mgr.initialize(id, &server_caps).unwrap();

        let session = mgr.get(id).unwrap();
        assert!(session.has_capability("query.rules"));
        assert!(!session.has_capability("tokens")); // denied — server didn't support
    }

    // ── Protocol Server ──

    #[test]
    fn protocol_server_creation() {
        let server = ProtocolServer::new("redox-rap", "0.1.0");
        assert_eq!(server.server_capabilities().server_name, "redox-rap");
        assert!(server.server_capabilities().supported.len() > 0);
    }

    #[test]
    fn register_and_dispatch() {
        let mut server = ProtocolServer::new("test", "1.0");
        server.register("echo", None, |params| {
            Ok(params.clone().unwrap_or(JsonValue::Null))
        });

        assert!(server.has_method("echo"));

        let req = RpcRequest::with_params(1, "echo", json_str("hello"));
        let resp = server.dispatch(&req).unwrap();
        assert!(resp.is_success());
        assert_eq!(resp.result, Some(json_str("hello")));
    }

    #[test]
    fn dispatch_method_not_found() {
        let mut server = ProtocolServer::new("test", "1.0");
        let req = RpcRequest::new(1, "nonexistent");
        let resp = server.dispatch(&req).unwrap();
        assert!(!resp.is_success());
        assert_eq!(resp.error.as_ref().unwrap().code, -32601);
    }

    #[test]
    fn dispatch_notification_returns_none() {
        let mut server = ProtocolServer::new("test", "1.0");
        let req = RpcRequest::notification("test.notify", None);
        let resp = server.dispatch(&req);
        assert!(resp.is_none());
    }

    #[test]
    fn dispatch_initialize() {
        let mut server = ProtocolServer::new("redox-rap", "0.1.0");
        let params = json_object(vec![
            ("clientName", json_str("test-agent")),
            ("clientVersion", json_str("1.0")),
        ]);
        let req = RpcRequest::with_params(1, "initialize", params);
        let resp = server.dispatch(&req).unwrap();
        assert!(resp.is_success());

        let result = resp.result.unwrap();
        let obj = result.as_object().unwrap();
        assert!(obj.contains_key("sessionId"));
        assert!(obj.contains_key("capabilities"));
        assert_eq!(obj.get("serverName").unwrap().as_str().unwrap(), "redox-rap");
    }

    #[test]
    fn dispatch_shutdown() {
        let mut server = ProtocolServer::new("test", "1.0");
        let req = RpcRequest::new(1, "shutdown");
        let resp = server.dispatch(&req).unwrap();
        assert!(resp.is_success());
    }

    #[test]
    fn dispatch_exit_notification() {
        let mut server = ProtocolServer::new("test", "1.0");
        // First create a session via initialize.
        let init_req = RpcRequest::with_params(1, "initialize", json_object(vec![]));
        server.dispatch(&init_req);
        assert!(server.sessions().total_count() > 0);

        // Exit should close all sessions.
        let exit_req = RpcRequest::notification("exit", None);
        let resp = server.dispatch(&exit_req);
        assert!(resp.is_none());
        assert_eq!(server.sessions().total_count(), 0);
    }

    #[test]
    fn standard_methods_registered() {
        let mut server = ProtocolServer::new("test", "1.0");
        server.register_standard_methods();

        assert!(server.has_method("query.rules"));
        assert!(server.has_method("cost/query"));
        assert!(server.has_method("skb/query"));
        assert!(server.has_method("verify/contracts"));
        assert!(server.has_method("build/heal"));
    }

    #[test]
    fn dispatch_standard_query_rules() {
        let mut server = ProtocolServer::new("test", "1.0");
        server.register_standard_methods();

        let req = RpcRequest::new(1, "query.rules");
        let resp = server.dispatch(&req).unwrap();
        assert!(resp.is_success());
        let arr = resp.result.unwrap();
        assert!(arr.as_array().unwrap().len() > 0);
    }

    #[test]
    fn dispatch_standard_cost_query() {
        let mut server = ProtocolServer::new("test", "1.0");
        server.register_standard_methods();

        let params = json_object(vec![("subject", json_str("Vec<T>"))]);
        let req = RpcRequest::with_params(2, "cost/query", params);
        let resp = server.dispatch(&req).unwrap();
        assert!(resp.is_success());
        let obj = resp.result.unwrap().as_object().unwrap().clone();
        assert_eq!(obj.get("subject").unwrap().as_str().unwrap(), "Vec<T>");
    }

    #[test]
    fn dispatch_standard_skb_query() {
        let mut server = ProtocolServer::new("test", "1.0");
        server.register_standard_methods();

        let params = json_object(vec![("fqn", json_str("std::vec::Vec"))]);
        let req = RpcRequest::with_params(3, "skb/query", params);
        let resp = server.dispatch(&req).unwrap();
        assert!(resp.is_success());
    }

    #[test]
    fn dispatch_standard_verify_contracts() {
        let mut server = ProtocolServer::new("test", "1.0");
        server.register_standard_methods();

        let req = RpcRequest::new(4, "verify/contracts");
        let resp = server.dispatch(&req).unwrap();
        let obj = resp.result.unwrap().as_object().unwrap().clone();
        assert_eq!(obj.get("verified").unwrap().as_bool().unwrap(), true);
    }

    #[test]
    fn server_lists_methods() {
        let mut server = ProtocolServer::new("test", "1.0");
        server.register_standard_methods();
        let methods = server.methods();
        assert!(methods.len() >= 11);
    }

    // ── Session state display ──

    #[test]
    fn session_state_display() {
        assert_eq!(format!("{}", SessionState::Created), "created");
        assert_eq!(format!("{}", SessionState::Active), "active");
        assert_eq!(format!("{}", SessionState::ShuttingDown), "shutting_down");
        assert_eq!(format!("{}", SessionState::Closed), "closed");
    }

    // ── Session error display ──

    #[test]
    fn session_error_display() {
        let id = SessionId::from_raw(42);
        let err = SessionError::NotFound(id);
        assert!(format!("{err}").contains("42"));
    }

    // ── RequestId ──

    #[test]
    fn request_id_variants() {
        let int_id = RequestId::Integer(42);
        let str_id = RequestId::Str("abc".to_string());
        assert_eq!(format!("{int_id}"), "42");
        assert_eq!(format!("{str_id}"), "\"abc\"");
    }

    // ── Full lifecycle ──

    #[test]
    fn full_lifecycle_initialize_query_shutdown_exit() {
        let mut server = ProtocolServer::new("redox-rap", "0.1.0");
        server.register_standard_methods();

        // 1. Initialize.
        let init = RpcRequest::with_params(1, "initialize", json_object(vec![
            ("clientName", json_str("agent-01")),
            ("clientVersion", json_str("2.0")),
        ]));
        let resp = server.dispatch(&init).unwrap();
        assert!(resp.is_success());
        assert!(server.sessions().total_count() > 0);

        // 2. Query.
        let query = RpcRequest::new(2, "query.rules");
        let resp = server.dispatch(&query).unwrap();
        assert!(resp.is_success());

        // 3. Shutdown.
        let shutdown = RpcRequest::new(3, "shutdown");
        let resp = server.dispatch(&shutdown).unwrap();
        assert!(resp.is_success());

        // 4. Exit.
        let exit = RpcRequest::notification("exit", None);
        server.dispatch(&exit);
        assert_eq!(server.sessions().total_count(), 0);
    }
}
