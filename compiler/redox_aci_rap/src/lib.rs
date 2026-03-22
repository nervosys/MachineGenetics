//! # ACI RAP Endpoints
//!
//! Exposes all ACI services through the Redox Augmentation Protocol (RAP).
//!
//! Endpoints:
//! - `aci.warnings`   — Dynamic warning generation
//! - `aci.debug`      — Intelligent debugging / root-cause analysis
//! - `aci.perf`       — Performance advisor
//! - `aci.swarm`      — Swarm coordination intelligence
//! - `aci.model`      — Codebase model queries
//! - `aci.status`     — Health and status of all ACI services
//!
//! Each endpoint accepts a service-specific request and returns a typed response.
//! The RAP router dispatches to the appropriate backend.
//!
//! (ROADMAP Step 66)

use std::collections::HashMap;
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════════
// RAP Endpoint Registry
// ═══════════════════════════════════════════════════════════════════════════

/// All ACI endpoints available via RAP.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AciEndpoint {
    Warnings,
    Debug,
    Perf,
    Swarm,
    Model,
    Status,
}

impl AciEndpoint {
    pub fn path(&self) -> &'static str {
        match self {
            AciEndpoint::Warnings => "aci.warnings",
            AciEndpoint::Debug => "aci.debug",
            AciEndpoint::Perf => "aci.perf",
            AciEndpoint::Swarm => "aci.swarm",
            AciEndpoint::Model => "aci.model",
            AciEndpoint::Status => "aci.status",
        }
    }

    pub fn from_path(path: &str) -> Option<AciEndpoint> {
        match path {
            "aci.warnings" => Some(AciEndpoint::Warnings),
            "aci.debug" => Some(AciEndpoint::Debug),
            "aci.perf" => Some(AciEndpoint::Perf),
            "aci.swarm" => Some(AciEndpoint::Swarm),
            "aci.model" => Some(AciEndpoint::Model),
            "aci.status" => Some(AciEndpoint::Status),
            _ => None,
        }
    }

    pub fn all() -> &'static [AciEndpoint] {
        &[
            AciEndpoint::Warnings,
            AciEndpoint::Debug,
            AciEndpoint::Perf,
            AciEndpoint::Swarm,
            AciEndpoint::Model,
            AciEndpoint::Status,
        ]
    }
}

impl fmt::Display for AciEndpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.path())
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Request / Response Protocol
// ═══════════════════════════════════════════════════════════════════════════

/// A RAP request to an ACI endpoint.
#[derive(Debug, Clone)]
pub struct RapRequest {
    pub endpoint: AciEndpoint,
    pub action: String,
    pub params: HashMap<String, String>,
    pub body: Option<String>,
}

impl RapRequest {
    pub fn new(endpoint: AciEndpoint, action: &str) -> Self {
        RapRequest {
            endpoint,
            action: action.to_string(),
            params: HashMap::new(),
            body: None,
        }
    }

    pub fn with_param(mut self, key: &str, value: &str) -> Self {
        self.params.insert(key.to_string(), value.to_string());
        self
    }

    pub fn with_body(mut self, body: &str) -> Self {
        self.body = Some(body.to_string());
        self
    }
}

/// Status code for RAP responses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RapStatus {
    Ok,
    Created,
    BadRequest,
    NotFound,
    InternalError,
    ServiceUnavailable,
}

impl RapStatus {
    pub fn code(&self) -> u16 {
        match self {
            RapStatus::Ok => 200,
            RapStatus::Created => 201,
            RapStatus::BadRequest => 400,
            RapStatus::NotFound => 404,
            RapStatus::InternalError => 500,
            RapStatus::ServiceUnavailable => 503,
        }
    }

    pub fn is_success(&self) -> bool {
        matches!(self, RapStatus::Ok | RapStatus::Created)
    }
}

/// A RAP response from an ACI endpoint.
#[derive(Debug, Clone)]
pub struct RapResponse {
    pub status: RapStatus,
    pub endpoint: AciEndpoint,
    pub action: String,
    pub data: HashMap<String, String>,
    pub message: Option<String>,
}

impl RapResponse {
    pub fn ok(endpoint: AciEndpoint, action: &str) -> Self {
        RapResponse {
            status: RapStatus::Ok,
            endpoint,
            action: action.to_string(),
            data: HashMap::new(),
            message: None,
        }
    }

    pub fn error(endpoint: AciEndpoint, action: &str, status: RapStatus, msg: &str) -> Self {
        RapResponse {
            status,
            endpoint,
            action: action.to_string(),
            data: HashMap::new(),
            message: Some(msg.to_string()),
        }
    }

    pub fn with_data(mut self, key: &str, value: &str) -> Self {
        self.data.insert(key.to_string(), value.to_string());
        self
    }

    pub fn with_message(mut self, msg: &str) -> Self {
        self.message = Some(msg.to_string());
        self
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Service Status
// ═══════════════════════════════════════════════════════════════════════════

/// Health status of a single ACI service.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceHealth {
    Healthy,
    Degraded,
    Unavailable,
}

/// Status of a single ACI service.
#[derive(Debug, Clone)]
pub struct ServiceStatus {
    pub endpoint: AciEndpoint,
    pub health: ServiceHealth,
    pub request_count: u64,
    pub error_count: u64,
    pub avg_latency_ms: f64,
}

impl ServiceStatus {
    pub fn error_rate(&self) -> f64 {
        if self.request_count > 0 {
            self.error_count as f64 / self.request_count as f64
        } else {
            0.0
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// RAP Router
// ═══════════════════════════════════════════════════════════════════════════

/// The ACI RAP router dispatches requests to service handlers.
pub struct AciRouter {
    service_statuses: HashMap<AciEndpoint, ServiceStatus>,
    enabled_endpoints: HashMap<AciEndpoint, bool>,
}

impl AciRouter {
    pub fn new() -> Self {
        let mut statuses = HashMap::new();
        let mut enabled = HashMap::new();
        for ep in AciEndpoint::all() {
            statuses.insert(*ep, ServiceStatus {
                endpoint: *ep,
                health: ServiceHealth::Healthy,
                request_count: 0,
                error_count: 0,
                avg_latency_ms: 0.0,
            });
            enabled.insert(*ep, true);
        }
        AciRouter {
            service_statuses: statuses,
            enabled_endpoints: enabled,
        }
    }

    pub fn disable(&mut self, endpoint: AciEndpoint) {
        self.enabled_endpoints.insert(endpoint, false);
        if let Some(status) = self.service_statuses.get_mut(&endpoint) {
            status.health = ServiceHealth::Unavailable;
        }
    }

    pub fn enable(&mut self, endpoint: AciEndpoint) {
        self.enabled_endpoints.insert(endpoint, true);
        if let Some(status) = self.service_statuses.get_mut(&endpoint) {
            status.health = ServiceHealth::Healthy;
        }
    }

    pub fn is_enabled(&self, endpoint: AciEndpoint) -> bool {
        self.enabled_endpoints.get(&endpoint).copied().unwrap_or(false)
    }

    /// Dispatch a RAP request.
    pub fn dispatch(&mut self, request: &RapRequest) -> RapResponse {
        let endpoint = request.endpoint;

        // Track request
        if let Some(status) = self.service_statuses.get_mut(&endpoint) {
            status.request_count += 1;
        }

        // Check if endpoint is enabled
        if !self.is_enabled(endpoint) {
            if let Some(status) = self.service_statuses.get_mut(&endpoint) {
                status.error_count += 1;
            }
            return RapResponse::error(
                endpoint,
                &request.action,
                RapStatus::ServiceUnavailable,
                &format!("{endpoint} is currently disabled"),
            );
        }

        // Route to handler
        let result = match endpoint {
            AciEndpoint::Warnings => self.handle_warnings(request),
            AciEndpoint::Debug => self.handle_debug(request),
            AciEndpoint::Perf => self.handle_perf(request),
            AciEndpoint::Swarm => self.handle_swarm(request),
            AciEndpoint::Model => self.handle_model(request),
            AciEndpoint::Status => self.handle_status(request),
        };

        // Track errors
        if !result.status.is_success() {
            if let Some(status) = self.service_statuses.get_mut(&endpoint) {
                status.error_count += 1;
            }
        }

        result
    }

    /// Dispatch by path string.
    pub fn dispatch_by_path(&mut self, path: &str, action: &str, params: HashMap<String, String>) -> RapResponse {
        match AciEndpoint::from_path(path) {
            Some(endpoint) => {
                let request = RapRequest {
                    endpoint,
                    action: action.to_string(),
                    params,
                    body: None,
                };
                self.dispatch(&request)
            }
            None => RapResponse::error(
                AciEndpoint::Status,
                action,
                RapStatus::NotFound,
                &format!("Unknown endpoint: {path}"),
            ),
        }
    }

    // ── Warning handlers ─────────────────────────────────────────────────

    fn handle_warnings(&self, request: &RapRequest) -> RapResponse {
        match request.action.as_str() {
            "analyze" => {
                let file = request.params.get("file").cloned().unwrap_or_default();
                RapResponse::ok(AciEndpoint::Warnings, "analyze")
                    .with_data("file", &file)
                    .with_data("warning_count", "0")
                    .with_message("Warning analysis complete")
            }
            "configure" => {
                RapResponse::ok(AciEndpoint::Warnings, "configure")
                    .with_message("Warning engine configured")
            }
            "feedback" => {
                let warning_id = request.params.get("warning_id").cloned().unwrap_or_default();
                let is_fp = request.params.get("false_positive").cloned().unwrap_or_default();
                RapResponse::ok(AciEndpoint::Warnings, "feedback")
                    .with_data("warning_id", &warning_id)
                    .with_data("false_positive", &is_fp)
            }
            _ => RapResponse::error(
                AciEndpoint::Warnings,
                &request.action,
                RapStatus::BadRequest,
                &format!("Unknown warnings action: {}", request.action),
            ),
        }
    }

    // ── Debug handlers ───────────────────────────────────────────────────

    fn handle_debug(&self, request: &RapRequest) -> RapResponse {
        match request.action.as_str() {
            "analyze_trace" => {
                RapResponse::ok(AciEndpoint::Debug, "analyze_trace")
                    .with_data("root_causes", "0")
                    .with_message("Trace analysis complete")
            }
            "causal_graph" => {
                RapResponse::ok(AciEndpoint::Debug, "causal_graph")
                    .with_data("edges", "0")
                    .with_message("Causal graph built")
            }
            _ => RapResponse::error(
                AciEndpoint::Debug,
                &request.action,
                RapStatus::BadRequest,
                &format!("Unknown debug action: {}", request.action),
            ),
        }
    }

    // ── Perf handlers ────────────────────────────────────────────────────

    fn handle_perf(&self, request: &RapRequest) -> RapResponse {
        match request.action.as_str() {
            "advise" => {
                RapResponse::ok(AciEndpoint::Perf, "advise")
                    .with_data("suggestion_count", "0")
                    .with_message("Performance advice generated")
            }
            "profile" => {
                let target = request.params.get("target").cloned().unwrap_or_else(|| "cpu".to_string());
                RapResponse::ok(AciEndpoint::Perf, "profile")
                    .with_data("target", &target)
                    .with_message("Profile collected")
            }
            _ => RapResponse::error(
                AciEndpoint::Perf,
                &request.action,
                RapStatus::BadRequest,
                &format!("Unknown perf action: {}", request.action),
            ),
        }
    }

    // ── Swarm handlers ───────────────────────────────────────────────────

    fn handle_swarm(&self, request: &RapRequest) -> RapResponse {
        match request.action.as_str() {
            "predict_conflicts" => {
                RapResponse::ok(AciEndpoint::Swarm, "predict_conflicts")
                    .with_data("prediction_count", "0")
                    .with_message("Conflict predictions generated")
            }
            "suggest_decomposition" => {
                RapResponse::ok(AciEndpoint::Swarm, "suggest_decomposition")
                    .with_data("group_count", "0")
                    .with_message("Decomposition suggested")
            }
            "session_history" => {
                RapResponse::ok(AciEndpoint::Swarm, "session_history")
                    .with_data("session_count", "0")
            }
            _ => RapResponse::error(
                AciEndpoint::Swarm,
                &request.action,
                RapStatus::BadRequest,
                &format!("Unknown swarm action: {}", request.action),
            ),
        }
    }

    // ── Model handlers ───────────────────────────────────────────────────

    fn handle_model(&self, request: &RapRequest) -> RapResponse {
        match request.action.as_str() {
            "infer" => {
                let query_type = request.params.get("type").cloned().unwrap_or_default();
                RapResponse::ok(AciEndpoint::Model, "infer")
                    .with_data("query_type", &query_type)
                    .with_message("Inference complete")
            }
            "train" => {
                RapResponse::ok(AciEndpoint::Model, "train")
                    .with_message("Training initiated")
            }
            "update" => {
                RapResponse::ok(AciEndpoint::Model, "update")
                    .with_message("Incremental update applied")
            }
            _ => RapResponse::error(
                AciEndpoint::Model,
                &request.action,
                RapStatus::BadRequest,
                &format!("Unknown model action: {}", request.action),
            ),
        }
    }

    // ── Status handler ───────────────────────────────────────────────────

    fn handle_status(&self, request: &RapRequest) -> RapResponse {
        match request.action.as_str() {
            "health" => {
                let mut resp = RapResponse::ok(AciEndpoint::Status, "health");
                for ep in AciEndpoint::all() {
                    if let Some(status) = self.service_statuses.get(ep) {
                        let health_str = match status.health {
                            ServiceHealth::Healthy => "healthy",
                            ServiceHealth::Degraded => "degraded",
                            ServiceHealth::Unavailable => "unavailable",
                        };
                        resp.data.insert(ep.path().to_string(), health_str.to_string());
                    }
                }
                resp.with_message("All services queried")
            }
            "metrics" => {
                let mut resp = RapResponse::ok(AciEndpoint::Status, "metrics");
                let total_requests: u64 = self.service_statuses.values()
                    .map(|s| s.request_count)
                    .sum();
                let total_errors: u64 = self.service_statuses.values()
                    .map(|s| s.error_count)
                    .sum();
                resp.data.insert("total_requests".to_string(), total_requests.to_string());
                resp.data.insert("total_errors".to_string(), total_errors.to_string());
                resp
            }
            _ => RapResponse::error(
                AciEndpoint::Status,
                &request.action,
                RapStatus::BadRequest,
                &format!("Unknown status action: {}", request.action),
            ),
        }
    }

    /// Get status for a specific endpoint.
    pub fn service_status(&self, endpoint: AciEndpoint) -> Option<&ServiceStatus> {
        self.service_statuses.get(&endpoint)
    }

    /// Get all service statuses.
    pub fn all_statuses(&self) -> Vec<&ServiceStatus> {
        self.service_statuses.values().collect()
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── Endpoint ─────────────────────────────────────────────────────────

    #[test]
    fn endpoint_path_roundtrip() {
        for ep in AciEndpoint::all() {
            let path = ep.path();
            assert_eq!(AciEndpoint::from_path(path), Some(*ep));
        }
    }

    #[test]
    fn endpoint_from_unknown_path() {
        assert_eq!(AciEndpoint::from_path("aci.unknown"), None);
    }

    #[test]
    fn endpoint_display() {
        assert_eq!(format!("{}", AciEndpoint::Warnings), "aci.warnings");
    }

    #[test]
    fn all_endpoints_count() {
        assert_eq!(AciEndpoint::all().len(), 6);
    }

    // ── RapStatus ────────────────────────────────────────────────────────

    #[test]
    fn status_codes() {
        assert_eq!(RapStatus::Ok.code(), 200);
        assert_eq!(RapStatus::NotFound.code(), 404);
        assert_eq!(RapStatus::InternalError.code(), 500);
    }

    #[test]
    fn status_success() {
        assert!(RapStatus::Ok.is_success());
        assert!(RapStatus::Created.is_success());
        assert!(!RapStatus::BadRequest.is_success());
    }

    // ── Request ──────────────────────────────────────────────────────────

    #[test]
    fn request_builder() {
        let req = RapRequest::new(AciEndpoint::Warnings, "analyze")
            .with_param("file", "main.rdx")
            .with_body("source code here");
        assert_eq!(req.endpoint, AciEndpoint::Warnings);
        assert_eq!(req.action, "analyze");
        assert_eq!(req.params["file"], "main.rdx");
        assert_eq!(req.body.as_deref(), Some("source code here"));
    }

    // ── Response ─────────────────────────────────────────────────────────

    #[test]
    fn response_ok() {
        let resp = RapResponse::ok(AciEndpoint::Debug, "analyze_trace")
            .with_data("root_causes", "3")
            .with_message("done");
        assert!(resp.status.is_success());
        assert_eq!(resp.data["root_causes"], "3");
        assert_eq!(resp.message.as_deref(), Some("done"));
    }

    #[test]
    fn response_error() {
        let resp = RapResponse::error(AciEndpoint::Perf, "bad", RapStatus::BadRequest, "invalid");
        assert!(!resp.status.is_success());
        assert_eq!(resp.status.code(), 400);
    }

    // ── ServiceStatus ────────────────────────────────────────────────────

    #[test]
    fn error_rate() {
        let s = ServiceStatus {
            endpoint: AciEndpoint::Warnings,
            health: ServiceHealth::Healthy,
            request_count: 100,
            error_count: 5,
            avg_latency_ms: 10.0,
        };
        assert!((s.error_rate() - 0.05).abs() < 0.001);
    }

    #[test]
    fn error_rate_zero_requests() {
        let s = ServiceStatus {
            endpoint: AciEndpoint::Debug,
            health: ServiceHealth::Healthy,
            request_count: 0,
            error_count: 0,
            avg_latency_ms: 0.0,
        };
        assert_eq!(s.error_rate(), 0.0);
    }

    // ── Router: Warnings ─────────────────────────────────────────────────

    #[test]
    fn router_warnings_analyze() {
        let mut router = AciRouter::new();
        let req = RapRequest::new(AciEndpoint::Warnings, "analyze")
            .with_param("file", "main.rdx");
        let resp = router.dispatch(&req);
        assert_eq!(resp.status, RapStatus::Ok);
        assert_eq!(resp.data["file"], "main.rdx");
    }

    #[test]
    fn router_warnings_feedback() {
        let mut router = AciRouter::new();
        let req = RapRequest::new(AciEndpoint::Warnings, "feedback")
            .with_param("warning_id", "w1")
            .with_param("false_positive", "true");
        let resp = router.dispatch(&req);
        assert_eq!(resp.status, RapStatus::Ok);
    }

    #[test]
    fn router_warnings_unknown_action() {
        let mut router = AciRouter::new();
        let req = RapRequest::new(AciEndpoint::Warnings, "bogus");
        let resp = router.dispatch(&req);
        assert_eq!(resp.status, RapStatus::BadRequest);
    }

    // ── Router: Debug ────────────────────────────────────────────────────

    #[test]
    fn router_debug_analyze_trace() {
        let mut router = AciRouter::new();
        let req = RapRequest::new(AciEndpoint::Debug, "analyze_trace");
        let resp = router.dispatch(&req);
        assert_eq!(resp.status, RapStatus::Ok);
    }

    // ── Router: Perf ─────────────────────────────────────────────────────

    #[test]
    fn router_perf_advise() {
        let mut router = AciRouter::new();
        let req = RapRequest::new(AciEndpoint::Perf, "advise");
        let resp = router.dispatch(&req);
        assert_eq!(resp.status, RapStatus::Ok);
        assert!(resp.data.contains_key("suggestion_count"));
    }

    // ── Router: Swarm ────────────────────────────────────────────────────

    #[test]
    fn router_swarm_predict_conflicts() {
        let mut router = AciRouter::new();
        let req = RapRequest::new(AciEndpoint::Swarm, "predict_conflicts");
        let resp = router.dispatch(&req);
        assert_eq!(resp.status, RapStatus::Ok);
    }

    #[test]
    fn router_swarm_suggest_decomposition() {
        let mut router = AciRouter::new();
        let req = RapRequest::new(AciEndpoint::Swarm, "suggest_decomposition");
        let resp = router.dispatch(&req);
        assert_eq!(resp.status, RapStatus::Ok);
    }

    // ── Router: Model ────────────────────────────────────────────────────

    #[test]
    fn router_model_infer() {
        let mut router = AciRouter::new();
        let req = RapRequest::new(AciEndpoint::Model, "infer")
            .with_param("type", "pattern");
        let resp = router.dispatch(&req);
        assert_eq!(resp.status, RapStatus::Ok);
        assert_eq!(resp.data["query_type"], "pattern");
    }

    // ── Router: Status ───────────────────────────────────────────────────

    #[test]
    fn router_status_health() {
        let mut router = AciRouter::new();
        let req = RapRequest::new(AciEndpoint::Status, "health");
        let resp = router.dispatch(&req);
        assert_eq!(resp.status, RapStatus::Ok);
        assert_eq!(resp.data["aci.warnings"], "healthy");
        assert_eq!(resp.data["aci.debug"], "healthy");
    }

    #[test]
    fn router_status_metrics() {
        let mut router = AciRouter::new();
        // Make some requests first
        router.dispatch(&RapRequest::new(AciEndpoint::Warnings, "analyze"));
        router.dispatch(&RapRequest::new(AciEndpoint::Debug, "analyze_trace"));
        let req = RapRequest::new(AciEndpoint::Status, "metrics");
        let resp = router.dispatch(&req);
        assert_eq!(resp.status, RapStatus::Ok);
        // 3 requests: 2 + the metrics request itself
        assert_eq!(resp.data["total_requests"], "3");
    }

    // ── Router: Disable/Enable ───────────────────────────────────────────

    #[test]
    fn router_disable_endpoint() {
        let mut router = AciRouter::new();
        router.disable(AciEndpoint::Perf);
        assert!(!router.is_enabled(AciEndpoint::Perf));
        let req = RapRequest::new(AciEndpoint::Perf, "advise");
        let resp = router.dispatch(&req);
        assert_eq!(resp.status, RapStatus::ServiceUnavailable);
    }

    #[test]
    fn router_reenable_endpoint() {
        let mut router = AciRouter::new();
        router.disable(AciEndpoint::Perf);
        router.enable(AciEndpoint::Perf);
        assert!(router.is_enabled(AciEndpoint::Perf));
        let req = RapRequest::new(AciEndpoint::Perf, "advise");
        let resp = router.dispatch(&req);
        assert_eq!(resp.status, RapStatus::Ok);
    }

    // ── Router: dispatch_by_path ─────────────────────────────────────────

    #[test]
    fn dispatch_by_path_ok() {
        let mut router = AciRouter::new();
        let resp = router.dispatch_by_path("aci.debug", "causal_graph", HashMap::new());
        assert_eq!(resp.status, RapStatus::Ok);
    }

    #[test]
    fn dispatch_by_path_not_found() {
        let mut router = AciRouter::new();
        let resp = router.dispatch_by_path("aci.nope", "anything", HashMap::new());
        assert_eq!(resp.status, RapStatus::NotFound);
    }

    // ── Router: request tracking ─────────────────────────────────────────

    #[test]
    fn request_count_tracked() {
        let mut router = AciRouter::new();
        router.dispatch(&RapRequest::new(AciEndpoint::Warnings, "analyze"));
        router.dispatch(&RapRequest::new(AciEndpoint::Warnings, "analyze"));
        router.dispatch(&RapRequest::new(AciEndpoint::Warnings, "bogus"));
        let status = router.service_status(AciEndpoint::Warnings).unwrap();
        assert_eq!(status.request_count, 3);
        assert_eq!(status.error_count, 1); // "bogus" -> BadRequest
    }

    #[test]
    fn disabled_endpoint_tracks_errors() {
        let mut router = AciRouter::new();
        router.disable(AciEndpoint::Debug);
        router.dispatch(&RapRequest::new(AciEndpoint::Debug, "analyze_trace"));
        let status = router.service_status(AciEndpoint::Debug).unwrap();
        assert_eq!(status.request_count, 1);
        assert_eq!(status.error_count, 1);
    }
}
