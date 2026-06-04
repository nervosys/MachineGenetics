//! Deployment Configuration
//!
//! Infrastructure-as-code primitives for deploying RMI agent clusters:
//!
//! - **DeploymentSpec**: Declarative deployment configuration
//! - **ContainerSpec**: Container image and resource definitions
//! - **ScalingPolicy**: Horizontal and vertical auto-scaling
//! - **NetworkPolicy**: Inter-agent network access control

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

// ============================================================================
// Deployment Specification
// ============================================================================

/// Top-level deployment specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentSpec {
    /// Deployment name
    pub name: String,
    /// Namespace/project
    pub namespace: String,
    /// Version
    pub version: String,
    /// Cluster target
    pub target: DeploymentTarget,
    /// Agent replicas
    pub replicas: Vec<ReplicaSpec>,
    /// Network policies
    pub network: NetworkPolicy,
    /// Resource quotas
    pub quotas: ResourceQuota,
    /// Environment variables
    pub env: HashMap<String, String>,
    /// Labels
    pub labels: HashMap<String, String>,
    /// Annotations
    pub annotations: HashMap<String, String>,
}

impl DeploymentSpec {
    /// Create a minimal deployment spec.
    pub fn new(name: &str, namespace: &str) -> Self {
        Self {
            name: name.to_string(),
            namespace: namespace.to_string(),
            version: "0.1.0".to_string(),
            target: DeploymentTarget::default(),
            replicas: Vec::new(),
            network: NetworkPolicy::default(),
            quotas: ResourceQuota::default(),
            env: HashMap::new(),
            labels: HashMap::new(),
            annotations: HashMap::new(),
        }
    }

    /// Add a replica set.
    pub fn add_replica(&mut self, spec: ReplicaSpec) {
        self.replicas.push(spec);
    }

    /// Set an environment variable.
    pub fn set_env(&mut self, key: &str, value: &str) {
        self.env.insert(key.to_string(), value.to_string());
    }

    /// Total replica count across all sets.
    pub fn total_replicas(&self) -> u32 {
        self.replicas.iter().map(|r| r.count).sum()
    }

    /// Validate the spec.
    pub fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();

        if self.name.is_empty() {
            errors.push("Deployment name is required".to_string());
        }
        if self.namespace.is_empty() {
            errors.push("Namespace is required".to_string());
        }
        if self.replicas.is_empty() {
            errors.push("At least one replica spec is required".to_string());
        }

        for (i, replica) in self.replicas.iter().enumerate() {
            if replica.count == 0 {
                errors.push(format!("Replica set {} has 0 replicas", i));
            }
            errors.extend(
                replica
                    .container
                    .validate()
                    .into_iter()
                    .map(|e| format!("Replica set {}: {}", i, e)),
            );
        }

        errors
    }
}

/// Deployment target environment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentTarget {
    /// Provider
    pub provider: CloudProvider,
    /// Region
    pub region: String,
    /// Availability zones
    pub zones: Vec<String>,
    /// Cluster type
    pub cluster_type: ClusterType,
}

impl Default for DeploymentTarget {
    fn default() -> Self {
        Self {
            provider: CloudProvider::Local,
            region: "local".to_string(),
            zones: vec!["default".to_string()],
            cluster_type: ClusterType::SingleNode,
        }
    }
}

/// Cloud provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CloudProvider {
    /// Local deployment
    Local,
    /// Amazon Web Services
    Aws,
    /// Microsoft Azure
    Azure,
    /// Google Cloud Platform
    Gcp,
    /// On-premise infrastructure
    OnPremise,
}

/// Cluster type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ClusterType {
    /// Single node deployment
    SingleNode,
    /// Kubernetes cluster
    Kubernetes,
    /// Docker Swarm cluster
    DockerSwarm,
    /// HashiCorp Nomad cluster
    Nomad,
    /// Bare metal deployment
    Bare,
}

// ============================================================================
// Replica Specification
// ============================================================================

/// Defines a group of identical agent replicas.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplicaSpec {
    /// Group name
    pub name: String,
    /// Number of replicas
    pub count: u32,
    /// Container specification
    pub container: ContainerSpec,
    /// Scaling policy
    pub scaling: Option<ScalingPolicy>,
    /// Health check
    pub health_check: HealthCheck,
    /// Restart policy
    pub restart_policy: RestartPolicy,
}

impl ReplicaSpec {
    /// Create a new replica spec.
    pub fn new(name: &str, count: u32, container: ContainerSpec) -> Self {
        Self {
            name: name.to_string(),
            count,
            container,
            scaling: None,
            health_check: HealthCheck::default(),
            restart_policy: RestartPolicy::default(),
        }
    }
}

/// Container image and resource specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerSpec {
    /// Container image (e.g., "rmi-agent:latest")
    pub image: String,
    /// Image pull policy
    pub pull_policy: PullPolicy,
    /// Command override
    pub command: Option<Vec<String>>,
    /// Arguments
    pub args: Option<Vec<String>>,
    /// CPU request (millicores)
    pub cpu_request: u32,
    /// CPU limit (millicores)
    pub cpu_limit: u32,
    /// Memory request (MB)
    pub memory_request_mb: u32,
    /// Memory limit (MB)
    pub memory_limit_mb: u32,
    /// GPU count
    pub gpus: u32,
    /// GPU type (e.g., "nvidia-a100", "nvidia-t4")
    pub gpu_type: Option<String>,
    /// Exposed ports
    pub ports: Vec<PortSpec>,
    /// Volume mounts
    pub volumes: Vec<VolumeSpec>,
    /// Extra environment variables
    pub env: HashMap<String, String>,
}

impl ContainerSpec {
    /// Create a minimal container spec.
    pub fn new(image: &str) -> Self {
        Self {
            image: image.to_string(),
            pull_policy: PullPolicy::IfNotPresent,
            command: None,
            args: None,
            cpu_request: 250,
            cpu_limit: 1000,
            memory_request_mb: 256,
            memory_limit_mb: 512,
            gpus: 0,
            gpu_type: None,
            ports: Vec::new(),
            volumes: Vec::new(),
            env: HashMap::new(),
        }
    }

    /// Create a GPU-enabled container spec.
    pub fn with_gpu(image: &str, gpus: u32, gpu_type: &str) -> Self {
        let mut spec = Self::new(image);
        spec.gpus = gpus;
        spec.gpu_type = Some(gpu_type.to_string());
        spec.cpu_limit = 4000;
        spec.memory_limit_mb = 16384;
        spec
    }

    /// Add a port.
    pub fn add_port(&mut self, port: PortSpec) {
        self.ports.push(port);
    }

    /// Add a volume.
    pub fn add_volume(&mut self, volume: VolumeSpec) {
        self.volumes.push(volume);
    }

    /// Validate the container spec.
    pub fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();

        if self.image.is_empty() {
            errors.push("Container image is required".to_string());
        }
        if self.cpu_limit < self.cpu_request {
            errors.push("CPU limit must be >= CPU request".to_string());
        }
        if self.memory_limit_mb < self.memory_request_mb {
            errors.push("Memory limit must be >= memory request".to_string());
        }

        errors
    }
}

/// Image pull policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PullPolicy {
    /// Always pull
    Always,
    /// Pull if not present
    IfNotPresent,
    /// Never pull
    Never,
}

/// Port specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortSpec {
    /// Port name
    pub name: String,
    /// Container port
    pub container_port: u16,
    /// Host port (optional)
    pub host_port: Option<u16>,
    /// Protocol
    pub protocol: PortProtocol,
}

/// Port protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PortProtocol {
    /// TCP protocol
    Tcp,
    /// UDP protocol
    Udp,
}

/// Volume specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeSpec {
    /// Volume name
    pub name: String,
    /// Mount path in container
    pub mount_path: String,
    /// Volume source
    pub source: VolumeSource,
    /// Read-only flag
    pub read_only: bool,
}

/// Volume source type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VolumeSource {
    /// Ephemeral in-memory
    EmptyDir,
    /// Host path
    HostPath(String),
    /// Persistent volume claim
    PersistentClaim(String),
    /// Config map
    ConfigMap(String),
    /// Secret
    Secret(String),
}

// ============================================================================
// Scaling Policy
// ============================================================================

/// Auto-scaling policy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScalingPolicy {
    /// Minimum replicas
    pub min_replicas: u32,
    /// Maximum replicas
    pub max_replicas: u32,
    /// Target CPU utilization (0.0-1.0)
    pub target_cpu: f64,
    /// Target memory utilization (0.0-1.0)
    pub target_memory: f64,
    /// Custom metrics for scaling
    pub custom_metrics: Vec<ScalingMetric>,
    /// Scale-up cooldown
    pub scale_up_cooldown_secs: u32,
    /// Scale-down cooldown
    pub scale_down_cooldown_secs: u32,
    /// Scale-down stabilization window
    pub stabilization_window_secs: u32,
}

impl Default for ScalingPolicy {
    fn default() -> Self {
        Self {
            min_replicas: 1,
            max_replicas: 10,
            target_cpu: 0.7,
            target_memory: 0.8,
            custom_metrics: Vec::new(),
            scale_up_cooldown_secs: 60,
            scale_down_cooldown_secs: 300,
            stabilization_window_secs: 300,
        }
    }
}

/// Custom scaling metric.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScalingMetric {
    /// Metric name
    pub name: String,
    /// Target value
    pub target_value: f64,
    /// Metric type
    pub metric_type: ScalingMetricType,
}

/// Scaling metric type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ScalingMetricType {
    /// Average across pods
    AverageValue,
    /// Total value
    TotalValue,
    /// Average utilization percentage
    AverageUtilization,
}

// ============================================================================
// Health Check
// ============================================================================

/// Health check configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheck {
    /// Check type
    pub check_type: HealthCheckType,
    /// Initial delay before first check
    pub initial_delay_secs: u32,
    /// Interval between checks
    pub period_secs: u32,
    /// Timeout per check
    pub timeout_secs: u32,
    /// Number of consecutive failures before unhealthy
    pub failure_threshold: u32,
    /// Number of consecutive successes before healthy
    pub success_threshold: u32,
}

impl Default for HealthCheck {
    fn default() -> Self {
        Self {
            check_type: HealthCheckType::Tcp(8080),
            initial_delay_secs: 5,
            period_secs: 10,
            timeout_secs: 3,
            failure_threshold: 3,
            success_threshold: 1,
        }
    }
}

/// Health check type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HealthCheckType {
    /// TCP port check
    Tcp(u16),
    /// HTTP GET check
    Http {
        /// Port to probe.
        port: u16,
        /// Request path (e.g. `/healthz`).
        path: String,
    },
    /// Command execution
    Command(Vec<String>),
}

/// Restart policy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestartPolicy {
    /// Policy type
    pub policy: RestartPolicyType,
    /// Maximum restart count
    pub max_restarts: u32,
    /// Backoff delay
    pub backoff_secs: u32,
}

impl Default for RestartPolicy {
    fn default() -> Self {
        Self {
            policy: RestartPolicyType::OnFailure,
            max_restarts: 5,
            backoff_secs: 10,
        }
    }
}

/// Restart policy type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RestartPolicyType {
    /// Always restart
    Always,
    /// Restart on failure
    OnFailure,
    /// Never restart
    Never,
}

// ============================================================================
// Network Policy
// ============================================================================

/// Network access control policy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkPolicy {
    /// Allow inter-agent communication
    pub allow_inter_agent: bool,
    /// Allow external ingress
    pub allow_ingress: bool,
    /// Allow external egress
    pub allow_egress: bool,
    /// Allowed external endpoints
    pub allowed_endpoints: Vec<String>,
    /// TLS required for inter-agent
    pub require_tls: bool,
    /// mTLS enabled
    pub mtls: bool,
}

impl Default for NetworkPolicy {
    fn default() -> Self {
        Self {
            allow_inter_agent: true,
            allow_ingress: false,
            allow_egress: false,
            allowed_endpoints: Vec::new(),
            require_tls: true,
            mtls: false,
        }
    }
}

/// Resource quota for a namespace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceQuota {
    /// Maximum total CPU (millicores)
    pub max_cpu: u32,
    /// Maximum total memory (MB)
    pub max_memory_mb: u32,
    /// Maximum total storage (MB)
    pub max_storage_mb: u32,
    /// Maximum pod/container count
    pub max_pods: u32,
    /// Maximum GPU count
    pub max_gpus: u32,
}

impl Default for ResourceQuota {
    fn default() -> Self {
        Self {
            max_cpu: 16000,          // 16 cores
            max_memory_mb: 65536,    // 64 GB
            max_storage_mb: 1048576, // 1 TB
            max_pods: 100,
            max_gpus: 8,
        }
    }
}

// ============================================================================
// Deployment Rendering
// ============================================================================

impl DeploymentSpec {
    /// Render as a Kubernetes-style YAML string (simplified).
    pub fn render_yaml(&self) -> String {
        let mut yaml = String::new();

        yaml.push_str(&format!("# RMI Deployment: {}\n", self.name));
        yaml.push_str("apiVersion: rmi/v1\n");
        yaml.push_str("kind: Deployment\n");
        yaml.push_str("metadata:\n");
        yaml.push_str(&format!("  name: {}\n", self.name));
        yaml.push_str(&format!("  namespace: {}\n", self.namespace));
        yaml.push_str(&format!("  version: {}\n", self.version));

        if !self.labels.is_empty() {
            yaml.push_str("  labels:\n");
            for (k, v) in &self.labels {
                yaml.push_str(&format!("    {}: {}\n", k, v));
            }
        }

        yaml.push_str("spec:\n");
        yaml.push_str("  target:\n");
        yaml.push_str(&format!("    provider: {:?}\n", self.target.provider));
        yaml.push_str(&format!("    region: {}\n", self.target.region));

        for replica in &self.replicas {
            yaml.push_str("  replicas:\n");
            yaml.push_str(&format!("    - name: {}\n", replica.name));
            yaml.push_str(&format!("      count: {}\n", replica.count));
            yaml.push_str("      container:\n");
            yaml.push_str(&format!("        image: {}\n", replica.container.image));
            yaml.push_str(&format!(
                "        cpu: {}m/{}\n",
                replica.container.cpu_request, replica.container.cpu_limit
            ));
            yaml.push_str(&format!(
                "        memory: {}Mi/{}Mi\n",
                replica.container.memory_request_mb, replica.container.memory_limit_mb
            ));

            if replica.container.gpus > 0 {
                yaml.push_str(&format!("        gpus: {}\n", replica.container.gpus));
                if let Some(ref gpu_type) = replica.container.gpu_type {
                    yaml.push_str(&format!("        gpuType: {}\n", gpu_type));
                }
            }
        }

        if !self.env.is_empty() {
            yaml.push_str("  env:\n");
            for (k, v) in &self.env {
                yaml.push_str(&format!("    {}: {}\n", k, v));
            }
        }

        yaml
    }

    /// Render as Docker Compose YAML (simplified).
    pub fn render_compose(&self) -> String {
        let mut yaml = String::new();

        yaml.push_str("# RMI Docker Compose\n");
        yaml.push_str("version: '3.8'\n");
        yaml.push_str("services:\n");

        for replica in &self.replicas {
            yaml.push_str(&format!("  {}:\n", replica.name));
            yaml.push_str(&format!("    image: {}\n", replica.container.image));
            yaml.push_str("    deploy:\n");
            yaml.push_str(&format!("      replicas: {}\n", replica.count));
            yaml.push_str("      resources:\n");
            yaml.push_str("        limits:\n");
            yaml.push_str(&format!(
                "          cpus: '{:.1}'\n",
                replica.container.cpu_limit as f64 / 1000.0
            ));
            yaml.push_str(&format!(
                "          memory: {}M\n",
                replica.container.memory_limit_mb
            ));

            if !replica.container.ports.is_empty() {
                yaml.push_str("    ports:\n");
                for port in &replica.container.ports {
                    let host = port.host_port.unwrap_or(port.container_port);
                    yaml.push_str(&format!("      - \"{}:{}\"\n", host, port.container_port));
                }
            }

            let all_env: HashMap<_, _> = self
                .env
                .iter()
                .chain(replica.container.env.iter())
                .collect();
            if !all_env.is_empty() {
                yaml.push_str("    environment:\n");
                for (k, v) in &all_env {
                    yaml.push_str(&format!("      - {}={}\n", k, v));
                }
            }
        }

        yaml
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deployment_spec_creation() {
        let spec = DeploymentSpec::new("rmi-agents", "production");
        assert_eq!(spec.name, "rmi-agents");
        assert_eq!(spec.namespace, "production");
        assert_eq!(spec.total_replicas(), 0);
    }

    #[test]
    fn test_deployment_with_replicas() {
        let mut spec = DeploymentSpec::new("rmi", "default");

        let container = ContainerSpec::new("rmi-agent:v1.0");
        spec.add_replica(ReplicaSpec::new("workers", 3, container));

        assert_eq!(spec.total_replicas(), 3);
    }

    #[test]
    fn test_container_spec_validation() {
        let mut c = ContainerSpec::new("my-image");
        assert!(c.validate().is_empty());

        c.image = String::new();
        assert!(!c.validate().is_empty());

        c.image = "valid".to_string();
        c.cpu_limit = 100;
        c.cpu_request = 500;
        assert!(!c.validate().is_empty());
    }

    #[test]
    fn test_container_with_gpu() {
        let c = ContainerSpec::with_gpu("rmi-gpu:latest", 2, "nvidia-a100");
        assert_eq!(c.gpus, 2);
        assert_eq!(c.gpu_type, Some("nvidia-a100".to_string()));
    }

    #[test]
    fn test_deployment_validation() {
        let spec = DeploymentSpec::new("rmi", "default");
        let errors = spec.validate();
        assert!(!errors.is_empty()); // No replicas

        let mut spec2 = DeploymentSpec::new("rmi", "default");
        spec2.add_replica(ReplicaSpec::new("workers", 2, ContainerSpec::new("img:v1")));
        assert!(spec2.validate().is_empty());
    }

    #[test]
    fn test_scaling_policy() {
        let policy = ScalingPolicy {
            min_replicas: 2,
            max_replicas: 20,
            target_cpu: 0.6,
            ..Default::default()
        };
        assert_eq!(policy.min_replicas, 2);
        assert_eq!(policy.max_replicas, 20);
    }

    #[test]
    fn test_render_yaml() {
        let mut spec = DeploymentSpec::new("rmi-cluster", "prod");
        spec.add_replica(ReplicaSpec::new("agents", 5, ContainerSpec::new("rmi:v2")));
        spec.set_env("LOG_LEVEL", "info");

        let yaml = spec.render_yaml();
        assert!(yaml.contains("rmi-cluster"));
        assert!(yaml.contains("rmi:v2"));
        assert!(yaml.contains("LOG_LEVEL"));
    }

    #[test]
    fn test_render_compose() {
        let mut spec = DeploymentSpec::new("rmi", "default");
        let mut container = ContainerSpec::new("rmi:latest");
        container.add_port(PortSpec {
            name: "grpc".to_string(),
            container_port: 50051,
            host_port: Some(50051),
            protocol: PortProtocol::Tcp,
        });
        spec.add_replica(ReplicaSpec::new("agents", 3, container));

        let compose = spec.render_compose();
        assert!(compose.contains("agents"));
        assert!(compose.contains("50051"));
    }

    #[test]
    fn test_network_policy() {
        let policy = NetworkPolicy {
            allow_inter_agent: true,
            allow_ingress: true,
            require_tls: true,
            mtls: true,
            ..Default::default()
        };
        assert!(policy.mtls);
    }

    #[test]
    fn test_health_check() {
        let hc = HealthCheck {
            check_type: HealthCheckType::Http {
                port: 8080,
                path: "/health".to_string(),
            },
            ..Default::default()
        };
        assert_eq!(hc.failure_threshold, 3);
    }

    #[test]
    fn test_volume_spec() {
        let vol = VolumeSpec {
            name: "model-data".to_string(),
            mount_path: "/data/models".to_string(),
            source: VolumeSource::PersistentClaim("model-pvc".to_string()),
            read_only: true,
        };
        assert!(vol.read_only);
    }
}
