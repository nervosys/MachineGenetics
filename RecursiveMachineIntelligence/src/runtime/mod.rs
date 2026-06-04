//! Production Runtime
//!
//! Memory management, observability, and deployment infrastructure:
//!
//! - **Memory Pool**: Arena-based allocation with slab management
//! - **Observability**: Metrics, distributed tracing, structured logging
//! - **Deployment**: Container specs, scaling policies, infrastructure-as-code

pub mod deployment;
pub mod memory_pool;
pub mod observability;

pub use deployment::{
    CloudProvider, ClusterType, ContainerSpec, DeploymentSpec, DeploymentTarget, HealthCheck,
    NetworkPolicy, PortSpec, ReplicaSpec, ResourceQuota, ScalingPolicy,
};
pub use memory_pool::{MemoryPool, PoolConfig, PoolStats, SizeClass, TensorBuffer};
pub use observability::{
    EventLog, HealthSnapshot, HistogramData, LogEvent, MetricKey, MetricValue, MetricsCollector,
    MetricsSnapshot, Severity, Span, SpanCollector, SpanStatus, SystemHealth, TraceContext,
};
