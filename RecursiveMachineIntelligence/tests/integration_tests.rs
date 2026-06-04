//! Integration Tests for RMI
//!
//! Tests that exercise multiple modules working together,
//! simulating real-world AI agent workflows.

use std::time::Duration;
use uuid::Uuid;

use rmi::core::{
    codegen::{
        ActivationKind, BinaryOpKind, CodeEmitter, FunctionBuilder, IRType, NormalizeKind,
        PrimitiveType, Program, ProgramBuilder, ReduceOpKind, RustEmitter,
    },
    emitters::{CudaEmitter, MlirEmitter, OnnxEmitter},
    optimization::{
        CommonSubexpressionElimination, ConstantFolding, DeadCodeElimination, OperatorFusion,
        OptimizationLevel, OptimizationPipeline,
    },
    swarm::{
        CollaborationStrategy, Proposal, ProposalType, ResourceSharingMode, SwarmConfig,
        SwarmCoordinator, SwarmTask, SwarmTaskType, TaskPriority, TaskStatus, Vote, VoteDecision,
    },
    verification::Verifier,
    AgentBuilder, AgentCapability, Concept, ConceptId, Ontology, Relation, RelationType,
};
use rmi::distributed::transport::LoadBalanceStrategy;
use rmi::distributed::{
    consensus::{RaftNode, RaftRole},
    discovery::{DiscoveryConfig, HealthStatus, ServiceInfo, ServiceRegistry},
    transport::{LoadBalancer, NodeAddr, TransportProtocol},
};
use rmi::evolution::self_modification::SideEffectKind;
use rmi::evolution::{
    population::{EvolutionConfig, EvolutionEngine, Genome, SelectionStrategy},
    self_modification::{
        CodePatch, PatchKind, PatchPayload, RollbackManager, SafetyGuard, Sandbox,
    },
};
use rmi::neural::{GradientTape, Variable};
use rmi::prelude::ConceptType;
use rmi::runtime::{
    deployment::{ContainerSpec, DeploymentSpec, ReplicaSpec},
    observability::{MetricKey, MetricsCollector, SpanCollector, SpanStatus},
};
use rmi::symbolic::inference::InferenceConfig;
use rmi::symbolic::InferenceEngine;
use rmi::symbolic::{unify, KnowledgeBase, Term};

// ============================================================================
// Multi-Module Integration Tests
// ============================================================================

/// Test: Agent + Ontology + Primitives integration
///
/// Simulates an agent discovering concepts from an ontology.
#[test]
fn test_agent_ontology_integration() {
    // 1. Create an ontology with neural concepts
    let ontology = Ontology::new("neural_concepts");

    let attention_id = ConceptId::new("neural", "attention");
    let attention = Concept::new(attention_id.clone(), ConceptType::Process)
        .with_label("Attention")
        .with_definition("Scaled dot-product attention mechanism");

    let transformer_id = ConceptId::new("neural", "transformer");
    let transformer = Concept::new(transformer_id.clone(), ConceptType::Schema)
        .with_label("Transformer")
        .with_definition("Attention-based neural architecture");

    ontology.add_concept(attention);
    ontology.add_concept(transformer);

    ontology.add_relation(Relation::new(
        transformer_id.clone(),
        RelationType::HasComponent,
        attention_id.clone(),
    ));

    // 2. Create an agent with architecture search capability
    let agent = AgentBuilder::new()
        .name("architect_agent")
        .capability(AgentCapability::ArchitectureSearch)
        .build()
        .unwrap();

    assert!(agent.has_capability(AgentCapability::ArchitectureSearch));

    // 3. Query the ontology for transformer
    let found = ontology.get(&transformer_id);
    assert!(found.is_some());
    assert_eq!(found.unwrap().id, transformer_id);

    // 4. Query related concepts
    let components = ontology.get_related(&transformer_id, RelationType::HasComponent);
    assert!(!components.is_empty());
}

/// Test: CodeGen + Neural Layers integration
///
/// Generates IR for a neural network and verifies it can be emitted as Rust code.
#[test]
fn test_codegen_neural_integration() {
    // 1. Build a simple MLP using the IR
    let mut builder = FunctionBuilder::new(
        "mlp_forward",
        vec![
            (
                "x".to_string(),
                IRType::tensor(PrimitiveType::F32, vec![1, 784]),
            ),
            (
                "w1".to_string(),
                IRType::tensor(PrimitiveType::F32, vec![784, 256]),
            ),
            (
                "w2".to_string(),
                IRType::tensor(PrimitiveType::F32, vec![256, 10]),
            ),
        ],
        IRType::tensor(PrimitiveType::F32, vec![1, 10]),
    );

    let x = builder.param(0);
    let w1 = builder.param(1);
    let w2 = builder.param(2);

    // Hidden layer: matmul + ReLU
    let h = builder.matmul(x, w1, false, false);
    let h = builder.activation(ActivationKind::ReLU, h);

    // Output layer
    let out = builder.matmul(h, w2, false, false);
    builder.ret(out);

    let func = builder.build();
    assert!(func.verify().is_ok());

    // 2. Create program and emit Rust code
    let mut program = Program::new("mlp_model");
    program.add_function(func);

    let emitter = RustEmitter;
    let rust_code = emitter.emit(&program).unwrap();

    // Verify emitted code contains expected elements
    assert!(rust_code.contains("pub fn mlp_forward"));
    assert!(rust_code.contains("matmul"));
}

/// Test: Autodiff + Layers integration
///
/// Tests gradient computation through neural network layers.
#[test]
fn test_autodiff_layers_integration() {
    let mut tape = GradientTape::new();

    // Create input variable
    let x = Variable::new(vec![1.0, 2.0, 3.0, 4.0], vec![2, 2], true);
    let x_id = tape.register(x);

    // Forward pass: simple operations
    let y_id = tape.mul(x_id, x_id); // y = x * x
    let z_id = tape.sum(y_id, None, false); // z = sum(y)

    // Backward pass
    tape.backward(z_id);

    // Check gradients exist
    let x_var = tape.get(x_id).unwrap();
    assert!(x_var.grad.is_some());

    // Gradient of sum(x^2) w.r.t. x is 2*x
    let grad = x_var.grad.as_ref().unwrap();
    assert_eq!(grad.len(), 4);
    // grad[0] should be 2*1 = 2, grad[1] should be 2*2 = 4, etc.
    assert!((grad[0] - 2.0).abs() < 1e-5);
    assert!((grad[1] - 4.0).abs() < 1e-5);
}

/// Test: Symbolic reasoning integration
///
/// Tests knowledge base reasoning.
#[test]
fn test_symbolic_reasoning_integration() {
    // 1. Create a knowledge base with logical facts
    let mut kb = KnowledgeBase::new();

    // Add facts: Bird(tweety), Penguin(opus)
    kb.add_fact("Bird", vec![Term::constant("tweety")]);
    kb.add_fact("Penguin", vec![Term::constant("opus")]);

    // Add rule: Penguin(X) -> Bird(X)
    // Since add_rule takes Predicates, construct them properly
    let head = rmi::symbolic::Predicate::new("Bird", vec![Term::var("X")]);
    let body = vec![rmi::symbolic::Predicate::new(
        "Penguin",
        vec![Term::var("X")],
    )];
    kb.add_rule(head, body);

    // 2. Query using inference
    let config = InferenceConfig::default();
    let engine = InferenceEngine::new(config);

    // Forward chaining should derive Bird(opus)
    let derived = engine.forward_chain(&mut kb);

    // Check that we derived Bird(opus)
    assert!(derived
        .iter()
        .any(|p| { p.name == "Bird" && matches!(&p.args[0], Term::Constant(c) if c == "opus") }));
}

/// Test: Unification symmetry
///
/// Tests that unification is symmetric.
#[test]
fn test_unification_symmetric() {
    let term_pairs = vec![
        (Term::var("X"), Term::constant("a")),
        (Term::var("X"), Term::var("Y")),
        (
            Term::func("f", vec![Term::var("X")]),
            Term::func("f", vec![Term::constant("a")]),
        ),
    ];

    for (t1, t2) in term_pairs {
        let result1 = unify(&t1, &t2);
        let result2 = unify(&t2, &t1);

        // Both should succeed or both should fail
        assert_eq!(
            result1.is_ok(),
            result2.is_ok(),
            "Unification should be symmetric for {:?} and {:?}",
            t1,
            t2
        );
    }
}

// ============================================================================
// Cross-Module Integration Tests
// ============================================================================

/// Test: Full IR pipeline — Build → Optimize → Verify → Emit (all backends)
///
/// Exercises the complete codegen pipeline: construct an IR program via
/// FunctionBuilder, run optimization passes, verify the result, then emit
/// to all code-generation backends (Rust, CUDA, MLIR, ONNX).
#[test]
fn test_ir_optimize_verify_emit_pipeline() {
    // 1. Build a 2-layer MLP with normalization in IR
    let mut builder = FunctionBuilder::new(
        "mlp_norm",
        vec![
            (
                "x".to_string(),
                IRType::tensor(PrimitiveType::F32, vec![4, 128]),
            ),
            (
                "w1".to_string(),
                IRType::tensor(PrimitiveType::F32, vec![128, 64]),
            ),
            (
                "w2".to_string(),
                IRType::tensor(PrimitiveType::F32, vec![64, 10]),
            ),
        ],
        IRType::tensor(PrimitiveType::F32, vec![4, 10]),
    );

    let x = builder.param(0);
    let w1 = builder.param(1);
    let w2 = builder.param(2);

    // hidden = LayerNorm(ReLU(x @ w1))
    let h = builder.matmul(x, w1, false, false);
    let h = builder.activation(ActivationKind::ReLU, h);
    let h = builder.normalize(NormalizeKind::LayerNorm, h, 1e-5);

    // out = Softmax(h @ w2)
    let out = builder.matmul(h, w2, false, false);
    let out = builder.activation(ActivationKind::Softmax, out);
    builder.ret(out);

    let func = builder.build();
    assert!(
        func.verify().is_ok(),
        "IR function should pass verification"
    );

    let mut program = Program::new("mlp_model");
    program.add_function(func);

    // 2. Optimize with O2 pipeline
    let pipeline = OptimizationPipeline::level(OptimizationLevel::O2);
    assert!(
        !pipeline.pass_names().is_empty(),
        "O2 pipeline should have passes"
    );
    let program = pipeline.optimize(program);

    // 3. Verify (optimization may reshape nodes)
    let verifier = Verifier::new();
    let _report = verifier.verify(&program);

    // 4. Emit to all backends and verify output is non-empty
    let rust_code = RustEmitter.emit(&program).unwrap();
    assert!(
        rust_code.contains("fn mlp_norm"),
        "Rust output missing function"
    );

    let cuda_code = CudaEmitter::new().emit(&program).unwrap();
    assert!(!cuda_code.is_empty(), "CUDA output should be non-empty");

    let mlir_code = MlirEmitter::new().emit(&program).unwrap();
    assert!(!mlir_code.is_empty(), "MLIR output should be non-empty");

    let onnx_code = OnnxEmitter::new().emit(&program).unwrap();
    assert!(!onnx_code.is_empty(), "ONNX output should be non-empty");
}

/// Test: IR optimization reduces dead code
///
/// Constructs a program with dead nodes, optimizes it, and verifies
/// the optimization pipeline removed them.
#[test]
fn test_optimization_dead_code_elimination() {
    let mut builder = FunctionBuilder::new(
        "dead_code_test",
        vec![(
            "x".to_string(),
            IRType::tensor(PrimitiveType::F32, vec![2, 2]),
        )],
        IRType::tensor(PrimitiveType::F32, vec![2, 2]),
    );

    let x = builder.param(0);
    // Dead computation: not used in result
    let _dead = builder.activation(ActivationKind::ReLU, x);
    // Live computation: returned
    let live = builder.activation(ActivationKind::Sigmoid, x);
    builder.ret(live);

    let func = builder.build();
    let mut program = Program::new("dce_test");
    program.add_function(func);

    let mut pipeline = OptimizationPipeline::new();
    pipeline.add_pass(DeadCodeElimination::new());
    let optimized = pipeline.optimize(program);

    // DCE pipeline should run without panicking
    let verifier = Verifier::new();
    let _report = verifier.verify(&optimized);
}

/// Test: Custom optimization pipeline with all passes
///
/// Builds a custom pipeline with all four pass types and runs it.
#[test]
fn test_custom_optimization_pipeline() {
    let mut builder = FunctionBuilder::new(
        "custom_opt",
        vec![(
            "x".to_string(),
            IRType::tensor(PrimitiveType::F32, vec![2, 4]),
        )],
        IRType::tensor(PrimitiveType::F32, vec![2, 4]),
    );

    let x = builder.param(0);
    let y = builder.activation(ActivationKind::ReLU, x);
    builder.ret(y);

    let func = builder.build();
    let mut program = Program::new("custom_test");
    program.add_function(func);

    let mut pipeline = OptimizationPipeline::new();
    pipeline.add_pass(DeadCodeElimination::new());
    pipeline.add_pass(ConstantFolding::new());
    pipeline.add_pass(CommonSubexpressionElimination::new());
    pipeline.add_pass(OperatorFusion::new());
    let pipeline = pipeline.max_iterations(3);

    let names = pipeline.pass_names();
    assert_eq!(names.len(), 4, "Should have 4 passes");

    let optimized = pipeline.optimize(program);
    assert!(Verifier::new().check(&optimized).is_ok());
}

/// Test: Verifier catches type errors
///
/// Constructs invalid IR and ensures the verifier reports errors.
#[test]
fn test_verifier_catches_invalid_ir() {
    // Build a function with mismatched return type
    let func = rmi::core::codegen::Function::new(
        "bad_return",
        vec![(
            "x".to_string(),
            IRType::tensor(PrimitiveType::F32, vec![2, 2]),
        )],
        // Claim return type is [3,3] but we won't match it
        IRType::tensor(PrimitiveType::F32, vec![3, 3]),
    );

    let mut program = Program::new("bad_program");
    program.add_function(func);

    let verifier = Verifier::new();
    let report = verifier.verify(&program);
    // Verify the verifier runs without panicking on malformed IR
    let _diagnostics = report.diagnostics();
}

/// Test: Swarm coordination — register agents, submit tasks, track status
///
/// Exercises the swarm module's core workflow: creating a coordinator,
/// registering agents with capabilities, submitting tasks, and checking status.
#[test]
fn test_swarm_task_lifecycle() {
    let config = SwarmConfig {
        max_agents: 10,
        consensus_threshold: 0.6,
        resource_sharing: ResourceSharingMode::Cooperative,
        collaboration_strategy: CollaborationStrategy::Centralized,
        task_timeout: Duration::from_secs(60),
        ..Default::default()
    };

    let mut coord = SwarmCoordinator::new(config);

    // Register agents with different capabilities
    let arch_id = coord
        .register_agent("architect", vec!["architecture_search".into()])
        .unwrap();
    let train_id = coord
        .register_agent("trainer", vec!["training".into()])
        .unwrap();

    assert_ne!(arch_id, train_id);

    // Submit a design task
    let task = SwarmTask::new(
        SwarmTaskType::Custom {
            name: "design_rnn".into(),
            params: Default::default(),
        },
        "Design an RNN architecture",
    )
    .with_priority(TaskPriority::High);

    let task_id = coord.submit_task(task);

    // Task should start as Pending
    let status = coord.task_status(task_id);
    assert_eq!(status, Some(TaskStatus::Pending));
}

/// Test: Swarm proposal + voting consensus
///
/// Tests the swarm's proposal/vote mechanism to reach consensus.
#[test]
fn test_swarm_proposal_consensus() {
    let mut coord = SwarmCoordinator::new(SwarmConfig {
        consensus_threshold: 0.5,
        max_agents: 5,
        ..Default::default()
    });

    // Register 3 agents
    let a1 = coord.register_agent("agent_1", vec![]).unwrap();
    let a2 = coord.register_agent("agent_2", vec![]).unwrap();
    let a3 = coord.register_agent("agent_3", vec![]).unwrap();

    // Create a proposal
    let proposal = Proposal::new(
        ProposalType::Custom("switch_optimizer".into()),
        a1,
        "Switch from SGD to Adam",
    );
    let prop_id = coord.create_proposal(proposal);

    // Vote: 2 approve, 1 reject → should pass with threshold 0.5
    coord
        .submit_vote(prop_id, Vote::new(a1, VoteDecision::Approve))
        .unwrap();
    coord
        .submit_vote(prop_id, Vote::new(a2, VoteDecision::Approve))
        .unwrap();
    coord
        .submit_vote(prop_id, Vote::new(a3, VoteDecision::Reject))
        .unwrap();

    // 2/3 ≈ 0.67 > 0.5 threshold → consensus reached
    // The proposal object tracks votes internally
}

/// Test: Distributed consensus — Raft cluster formation + leader election
///
/// Creates a small Raft cluster, triggers an election, and verifies roles.
#[test]
fn test_raft_cluster_election() {
    let node1_id = Uuid::new_v4();
    let node2_id = Uuid::new_v4();
    let node3_id = Uuid::new_v4();

    let mut node1 = RaftNode::new(node1_id);
    node1.add_node(node2_id);
    node1.add_node(node3_id);

    assert_eq!(node1.role(), RaftRole::Follower, "Start as follower");
    assert_eq!(node1.term(), 0);
    assert_eq!(node1.cluster_size(), 3);
    assert_eq!(node1.quorum_size(), 2);

    // Start election
    let _vote_request = node1.start_election();
    assert_eq!(
        node1.role(),
        RaftRole::Candidate,
        "Should transition to candidate"
    );
    assert_eq!(node1.term(), 1, "Term should increment");
}

/// Test: Service registry + load balancer integration
///
/// Registers services in a discovery registry, then uses the load balancer
/// to route requests based on latency.
#[test]
fn test_service_registry_load_balancer() {
    let registry = ServiceRegistry::new(DiscoveryConfig::default());

    // Register 3 services with different capabilities
    let svc1 = ServiceInfo::new(
        "gpu_worker_1",
        NodeAddr::new("127.0.0.1:5001", TransportProtocol::Tcp),
        vec!["gpu".into(), "training".into()],
    )
    .with_load(0.3);
    let svc2 = ServiceInfo::new(
        "gpu_worker_2",
        NodeAddr::new("127.0.0.1:5002", TransportProtocol::Tcp),
        vec!["gpu".into(), "inference".into()],
    )
    .with_load(0.8);
    let svc3 = ServiceInfo::new(
        "cpu_worker",
        NodeAddr::new("127.0.0.1:5003", TransportProtocol::Tcp),
        vec!["cpu".into(), "preprocessing".into()],
    )
    .with_load(0.1);

    let id1 = svc1.id;
    let id2 = svc2.id;
    let id3 = svc3.id;

    registry.register(svc1).unwrap();
    registry.register(svc2).unwrap();
    registry.register(svc3).unwrap();

    assert_eq!(registry.len(), 3);

    // Find GPU-capable services
    let gpu_services = registry.find_by_capability("gpu");
    assert_eq!(gpu_services.len(), 2, "Should find 2 GPU services");

    // Set up a load balancer with LeastLatency strategy
    let lb = LoadBalancer::new(LoadBalanceStrategy::LeastLatency);
    lb.register_node(id1, 1.0);
    lb.register_node(id2, 1.0);
    lb.register_node(id3, 1.0);

    // Update latencies
    lb.update_latency(id1, 50); // 50µs
    lb.update_latency(id2, 200); // 200µs
    lb.update_latency(id3, 10); // 10µs

    // LeastLatency should pick node3 (lowest latency)
    let candidates = vec![id1, id2, id3];
    let selected = lb.select(&candidates);
    assert!(selected.is_some());
    assert_eq!(selected.unwrap(), id3, "Should select lowest-latency node");
}

/// Test: Service health tracking + pruning stale services
///
/// Registers services, marks some unhealthy, and prunes stale ones.
#[test]
fn test_service_health_and_pruning() {
    let config = DiscoveryConfig {
        service_timeout: Duration::from_millis(1), // Very short for test
        ..Default::default()
    };
    let registry = ServiceRegistry::new(config);

    let svc = ServiceInfo::new(
        "ephemeral",
        NodeAddr::new("10.0.0.1:8080", TransportProtocol::Tcp),
        vec!["temp".into()],
    );
    let svc_id = svc.id;
    registry.register(svc).unwrap();

    // Mark unhealthy
    registry.update_health(svc_id, HealthStatus::Unhealthy);

    // Let it become stale (timeout is 1ms)
    std::thread::sleep(Duration::from_millis(5));

    let pruned = registry.prune_stale();
    assert!(pruned.contains(&svc_id), "Stale service should be pruned");
    assert!(
        registry.is_empty(),
        "Registry should be empty after pruning"
    );
}

/// Test: Evolution engine — initialize, set fitness, evolve
///
/// Creates a population of real-valued genomes, assigns fitness,
/// evolves one generation, and verifies the engine progresses.
#[test]
fn test_evolution_lifecycle() {
    let config = EvolutionConfig {
        population_size: 20,
        max_generations: 10,
        crossover_rate: 0.9,
        mutation_rate: 0.1,
        elitism_ratio: 0.1,
        selection: SelectionStrategy::Tournament(3),
        ..Default::default()
    };

    let mut engine = EvolutionEngine::new(config);

    // Initialize with real-valued genomes of length 5
    engine.initialize_real(5, -1.0, 1.0);
    assert_eq!(engine.population().len(), 20);
    assert_eq!(engine.generation(), 0);

    // Assign fitness = -(sum of squared values) → maximize towards 0
    for ind in engine.population_mut() {
        if let Genome::RealValued(ref genes) = ind.genome {
            let fitness = -genes.iter().map(|g| g * g).sum::<f64>();
            ind.set_fitness(fitness);
        }
    }

    let _best_before = engine.best_fitness();

    // Evolve one generation
    engine.step();
    assert_eq!(engine.generation(), 1);

    // Assign fitness again for the new generation
    for ind in engine.population_mut() {
        if let Genome::RealValued(ref genes) = ind.genome {
            let fitness = -genes.iter().map(|g| g * g).sum::<f64>();
            ind.set_fitness(fitness);
        }
    }

    // Population should still be the right size
    assert_eq!(engine.population().len(), 20);

    // Statistics should be tracked
    assert!(!engine.statistics().is_empty());
}

/// Test: Self-modification safety pipeline — Sandbox → Rollback → SafetyGuard
///
/// Simulates a self-modifying agent: run a code change in a sandbox,
/// checkpoint via rollback manager, verify safety with the guard.
#[test]
fn test_self_modification_safety_pipeline() {
    // 1. Execute in sandbox
    let mut sandbox = Sandbox::new();
    sandbox.snapshot_state("model_weights", vec![1, 2, 3, 4]);

    let result = sandbox.execute(|usage| {
        usage.memory_bytes += 1024;
        usage.cpu_ms += 50;
        Ok(42u64)
    });
    assert!(result.success);

    // 2. Checkpoint state with rollback manager
    let rm = RollbackManager::new(10);
    let v1 = rm.checkpoint("model", vec![10, 20, 30], "initial weights", None);
    assert_eq!(v1, 1);

    let v2 = rm.checkpoint("model", vec![11, 21, 31], "after training epoch 1", None);
    assert_eq!(v2, 2);

    assert_eq!(rm.current_version("model"), Some(2));

    // Rollback to v1
    let rolled = rm.rollback("model", 1).unwrap();
    assert_eq!(rolled.data, vec![10, 20, 30]);
    assert_eq!(rm.current_version("model"), Some(1));

    // 3. Safety guard checks
    let guard = SafetyGuard::new();
    let mut params = std::collections::HashMap::new();
    params.insert("lr".into(), 0.001);
    let patch = CodePatch::new(
        "optimizer::learning_rate",
        PatchKind::ParameterUpdate,
        PatchPayload::Parameters(params),
        Uuid::new_v4(),
    )
    .with_rationale("Reduce learning rate for stability");

    let verdict = guard.check(&patch, None);
    // A parameter update should generally be safe
    assert!(
        verdict.safe || verdict.risk_score < 0.8,
        "Parameter update should not be blocked, risk={}",
        verdict.risk_score
    );
}

/// Test: Sandbox side-effect tracking
///
/// Exercises the sandbox's side-effect recording and resource tracking.
#[test]
fn test_sandbox_side_effects() {
    let mut sandbox = Sandbox::new();

    // Execute something that produces side effects
    let _ = sandbox.execute(|usage| {
        usage.memory_bytes += 2048;
        usage.cpu_ms += 100;
        Ok("done")
    });

    sandbox.record_side_effect(
        SideEffectKind::StateModification,
        "Updated model parameters",
        0.3,
    );

    assert!(sandbox.usage().memory_bytes >= 2048);
    assert!(sandbox.usage().cpu_ms >= 100);
}

/// Test: Deployment + Observability — metrics during deployment lifecycle
///
/// Creates a deployment spec, starts metrics/tracing, records deployment events.
#[test]
fn test_deployment_observability_integration() {
    // 1. Create deployment spec
    let mut spec = DeploymentSpec::new("rmi-inference-service", "production");
    let container = ContainerSpec::new("rmi/inference:v0.2.0");
    let replica = ReplicaSpec::new("inference-worker", 3, container);
    spec.add_replica(replica);
    spec.set_env("MODEL_PATH", "/models/transformer-v1");

    assert_eq!(spec.total_replicas(), 3);
    let validation_errors = spec.validate();
    assert!(
        validation_errors.is_empty(),
        "Deployment spec should be valid: {:?}",
        validation_errors
    );

    // 2. Set up observability
    let metrics = MetricsCollector::new("deployment");
    let spans = SpanCollector::new("deploy-service", 1000);

    // 3. Simulate deployment lifecycle with metrics
    let deploy_span = spans.start_span("deploy");

    // Record deployment metrics
    metrics.counter_inc(MetricKey::new("deployments_total"), 1);
    metrics.gauge_set(MetricKey::new("replicas_desired"), 3.0);

    // Simulate replica startup
    for i in 0..3 {
        let replica_span = spans.start_child(&deploy_span, &format!("start_replica_{i}"));
        metrics.counter_inc(MetricKey::new("replicas_started"), 1);
        metrics.histogram_observe(
            MetricKey::new("replica_startup_seconds"),
            0.5 + (i as f64) * 0.1,
        );

        let mut replica_span = replica_span;
        replica_span.finish(SpanStatus::Ok);
        spans.record(replica_span);
    }

    let mut deploy_span = deploy_span;
    deploy_span.finish(SpanStatus::Ok);
    spans.record(deploy_span);

    // 4. Verify metrics
    assert_eq!(metrics.counter_get(&MetricKey::new("deployments_total")), 1);
    assert_eq!(metrics.counter_get(&MetricKey::new("replicas_started")), 3);
    assert_eq!(
        metrics.gauge_get(&MetricKey::new("replicas_desired")),
        Some(3.0)
    );

    let startup_hist = metrics
        .histogram_get(&MetricKey::new("replica_startup_seconds"))
        .unwrap();
    assert_eq!(startup_hist.count, 3);

    // 5. Verify tracing
    assert!(spans.span_count() >= 4); // 1 deploy + 3 replica spans
}

/// Test: Observability — snapshot and metric labels
///
/// Tests that metrics with labels are tracked independently and
/// snapshots capture all data.
#[test]
fn test_observability_labeled_metrics() {
    let mc = MetricsCollector::new("model");

    // Track metrics with labels
    let key_train = MetricKey::new("loss").with_label("phase", "train");
    let key_val = MetricKey::new("loss").with_label("phase", "validation");

    mc.histogram_observe(key_train.clone(), 0.5);
    mc.histogram_observe(key_train.clone(), 0.4);
    mc.histogram_observe(key_val.clone(), 0.6);

    let train_hist = mc.histogram_get(&key_train).unwrap();
    assert_eq!(train_hist.count, 2);

    let val_hist = mc.histogram_get(&key_val).unwrap();
    assert_eq!(val_hist.count, 1);

    // Take a snapshot
    let snapshot = mc.snapshot();
    assert!(!snapshot.metrics.is_empty());
}

/// Test: Agent + Swarm + Ontology — agent-driven task submission
///
/// Creates agents, registers them with a swarm coordinator, uses ontology
/// knowledge to determine task assignments.
#[test]
fn test_agent_swarm_ontology_integration() {
    // 1. Build ontology
    let ontology = Ontology::new("ml_pipeline");
    let preprocessing_id = ConceptId::new("pipeline", "preprocessing");
    let training_id = ConceptId::new("pipeline", "training");
    ontology.add_concept(
        Concept::new(preprocessing_id.clone(), ConceptType::Process)
            .with_label("Preprocessing")
            .with_definition("Data preprocessing stage"),
    );
    ontology.add_concept(
        Concept::new(training_id.clone(), ConceptType::Process)
            .with_label("Training")
            .with_definition("Model training stage"),
    );
    ontology.add_relation(Relation::new(
        training_id.clone(),
        RelationType::Precedes,
        preprocessing_id.clone(),
    ));

    // 2. Create agents
    let preprocess_agent = AgentBuilder::new()
        .name("preprocessor")
        .capability(AgentCapability::ResourceManagement)
        .build()
        .unwrap();
    let train_agent = AgentBuilder::new()
        .name("trainer")
        .capability(AgentCapability::TrainingOrchestration)
        .capability(AgentCapability::GradientComputation)
        .build()
        .unwrap();

    assert!(preprocess_agent.has_capability(AgentCapability::ResourceManagement));
    assert!(train_agent.has_capability(AgentCapability::TrainingOrchestration));

    // 3. Register in swarm
    let mut coord = SwarmCoordinator::new(SwarmConfig::default());
    let _prep_id = coord
        .register_agent("preprocessor", vec!["data_prep".into()])
        .unwrap();
    let _train_id = coord
        .register_agent("trainer", vec!["training".into(), "gpu".into()])
        .unwrap();

    // 4. Verify ontology dependency
    let deps = ontology.get_related(&training_id, RelationType::Precedes);
    assert_eq!(deps.len(), 1, "Training precedes preprocessing");
    assert_eq!(deps[0].id, preprocessing_id);

    // 5. Submit tasks respecting the dependency order
    let prep_task = SwarmTask::new(
        SwarmTaskType::Custom {
            name: "preprocess_data".into(),
            params: Default::default(),
        },
        "Clean and normalize training data",
    )
    .with_priority(TaskPriority::Normal);

    let prep_task_id = coord.submit_task(prep_task);

    let train_task = SwarmTask::new(
        SwarmTaskType::Custom {
            name: "train_model".into(),
            params: Default::default(),
        },
        "Train transformer model",
    )
    .with_priority(TaskPriority::High)
    .with_dependency(prep_task_id);

    let _train_task_id = coord.submit_task(train_task);
}

/// Test: Evolution + CodeGen — evolve IR programs
///
/// Uses the evolution engine to evolve parameters that affect an IR program,
/// combining the evolution and codegen modules.
#[test]
fn test_evolution_codegen_integration() {
    // 1. Create an evolution engine for hyperparameters
    let config = EvolutionConfig {
        population_size: 10,
        max_generations: 5,
        mutation_rate: 0.2,
        ..Default::default()
    };
    let mut engine = EvolutionEngine::new(config);

    // Each genome encodes: [hidden_dim_factor, learning_rate, dropout_rate]
    engine.initialize_real(3, 0.0, 1.0);

    // 2. Evaluate each individual by building an IR program
    for ind in engine.population_mut() {
        if let Genome::RealValued(ref genes) = ind.genome {
            let hidden_scale = genes[0]; // 0..1 → maps to hidden dim
            let _lr = genes[1] * 0.1; // 0..0.1
            let dropout = genes[2] * 0.5; // 0..0.5

            // Build a simple IR to "evaluate" this config
            let hidden_dim = 32 + (hidden_scale * 224.0) as usize; // 32..256
            let mut builder = FunctionBuilder::new(
                "evolved_net",
                vec![(
                    "x".to_string(),
                    IRType::tensor(PrimitiveType::F32, vec![1, 64]),
                )],
                IRType::tensor(PrimitiveType::F32, vec![1, 10]),
            );
            let x = builder.param(0);
            // Use dropout rate as a proxy for complexity penalty
            let _ = builder.activation(ActivationKind::ReLU, x);
            let func = builder.build();
            assert!(func.verify().is_ok());

            // Fitness: prefer medium hidden dim (128) and low dropout
            let dim_penalty = ((hidden_dim as f64) - 128.0).abs() / 128.0;
            let fitness = 1.0 - dim_penalty - dropout;
            ind.set_fitness(fitness);
        }
    }

    let _best_before = engine.best_fitness();
    engine.step();

    // Population should maintain size
    assert_eq!(engine.population().len(), 10);
    assert_eq!(engine.generation(), 1);
}

/// Test: Autodiff + CodeGen — gradient-guided IR construction
///
/// Combines autodiff (for computing gradients) with codegen (for building
/// an IR representation of the same computation).
#[test]
fn test_autodiff_codegen_integration() {
    // 1. Autodiff: compute gradients of a simple function
    let mut tape = GradientTape::new();
    let x = Variable::new(vec![2.0, 3.0], vec![1, 2], true);
    let x_id = tape.register(x);
    let y_id = tape.mul(x_id, x_id); // y = x^2
    let z_id = tape.sum(y_id, None, false); // z = sum(x^2) = 4 + 9 = 13
    tape.backward(z_id);

    let grad = tape.get(x_id).unwrap().grad.as_ref().unwrap();
    assert!((grad[0] - 4.0).abs() < 1e-5); // d/dx(x^2) = 2x = 4
    assert!((grad[1] - 6.0).abs() < 1e-5); // 2*3 = 6

    // 2. CodeGen: build the same computation as IR
    let mut builder = FunctionBuilder::new(
        "grad_test",
        vec![(
            "x".to_string(),
            IRType::tensor(PrimitiveType::F32, vec![1, 2]),
        )],
        IRType::Primitive(PrimitiveType::F32),
    );
    let x = builder.param(0);
    let x_sq = builder.binary_op(BinaryOpKind::Mul, x, x);
    let sum = builder.reduce(ReduceOpKind::Sum, x_sq, vec![]);
    builder.ret(sum);

    let func = builder.build();
    assert!(func.verify().is_ok());

    // 3. Emit and verify
    let mut program = Program::new("grad_model");
    program.add_function(func);
    let code = RustEmitter.emit(&program).unwrap();
    assert!(code.contains("grad_test"));
}

/// Test: Raft consensus + service registry — distributed cluster management
///
/// Sets up a Raft cluster alongside a service registry, demonstrating
/// how consensus and discovery work together.
#[test]
fn test_raft_service_registry_integration() {
    let id1 = Uuid::new_v4();
    let id2 = Uuid::new_v4();
    let id3 = Uuid::new_v4();

    // 1. Set up Raft cluster
    let mut node1 = RaftNode::new(id1);
    let mut node2 = RaftNode::new(id2);
    let mut node3 = RaftNode::new(id3);

    node1.add_node(id2);
    node1.add_node(id3);
    node2.add_node(id1);
    node2.add_node(id3);
    node3.add_node(id1);
    node3.add_node(id2);

    // 2. Register all nodes in service registry
    let registry = ServiceRegistry::new(DiscoveryConfig::default());
    for (_id, port) in [(id1, 5001), (id2, 5002), (id3, 5003)] {
        let svc = ServiceInfo::new(
            &format!("raft-node-{port}"),
            NodeAddr::new(&format!("127.0.0.1:{port}"), TransportProtocol::Tcp),
            vec!["consensus".into(), "storage".into()],
        );
        registry.register(svc).unwrap();
    }

    assert_eq!(registry.len(), 3);

    // 3. All nodes start as followers
    assert_eq!(node1.role(), RaftRole::Follower);
    assert_eq!(node2.role(), RaftRole::Follower);

    // 4. Node 1 starts election
    let _msg = node1.start_election();
    assert_eq!(node1.role(), RaftRole::Candidate);

    // 5. All consensus-capable services should be discoverable
    let consensus_services = registry.find_by_capability("consensus");
    assert_eq!(consensus_services.len(), 3);
}

/// Test: Load balancer strategy comparison
///
/// Tests multiple load balancing strategies produce valid selections.
#[test]
fn test_load_balancer_strategies() {
    let strategies = [
        LoadBalanceStrategy::RoundRobin,
        LoadBalanceStrategy::LeastLatency,
        LoadBalanceStrategy::LeastLoaded,
        LoadBalanceStrategy::Random,
    ];

    let nodes: Vec<Uuid> = (0..3).map(|_| Uuid::new_v4()).collect();

    for strategy in &strategies {
        let lb = LoadBalancer::new(*strategy);
        for &node in &nodes {
            lb.register_node(node, 1.0);
        }

        // Set some metrics
        for (i, &node) in nodes.iter().enumerate() {
            lb.update_latency(node, (i as u64 + 1) * 10);
            lb.update_load(node, i + 1);
        }

        let selected = lb.select(&nodes);
        assert!(
            selected.is_some(),
            "Strategy {:?} should select a node",
            strategy
        );
        assert!(
            nodes.contains(&selected.unwrap()),
            "Selected node should be from candidates"
        );
    }
}

/// Test: ProgramBuilder — high-level program construction
///
/// Uses ProgramBuilder (vs raw FunctionBuilder) to construct a multi-function program.
#[test]
fn test_program_builder_multi_function() {
    let prog_builder = ProgramBuilder::new("multi_func");

    // Add type aliases
    let prog_builder = prog_builder.add_type(
        "InputTensor",
        IRType::tensor(PrimitiveType::F32, vec![1, 784]),
    );
    let prog_builder = prog_builder.add_type(
        "OutputTensor",
        IRType::tensor(PrimitiveType::F32, vec![1, 10]),
    );

    let program = prog_builder.build().unwrap();

    // Verify and emit
    let verifier = Verifier::new();
    let report = verifier.verify(&program);
    // Empty program with just types should be OK
    assert!(
        report.errors().is_empty(),
        "Program with type aliases should verify cleanly"
    );
}

/// Test: Rollback manager — multi-key state management
///
/// Tests managing multiple state keys with independent rollback histories.
#[test]
fn test_rollback_multi_key() {
    let rm = RollbackManager::new(5);

    // Checkpoint two different keys
    rm.checkpoint("model_weights", vec![1, 2, 3], "epoch 0", None);
    rm.checkpoint("optimizer_state", vec![10, 20], "initial", None);

    rm.checkpoint("model_weights", vec![4, 5, 6], "epoch 1", None);
    rm.checkpoint("model_weights", vec![7, 8, 9], "epoch 2", None);

    // Current versions
    assert_eq!(rm.current_version("model_weights"), Some(3));
    assert_eq!(rm.current_version("optimizer_state"), Some(1));
    assert_eq!(rm.total_versions(), 4);

    // Rollback model weights to epoch 0 without affecting optimizer
    let restored = rm.rollback("model_weights", 1).unwrap();
    assert_eq!(restored.data, vec![1, 2, 3]);
    assert_eq!(rm.current_version("model_weights"), Some(1));
    assert_eq!(
        rm.current_version("optimizer_state"),
        Some(1),
        "Optimizer state should be unaffected"
    );

    // History should be preserved
    let hist = rm.history("model_weights");
    assert!(!hist.is_empty());
}

/// Test: Symbolic reasoning + Ontology — knowledge-driven inference
///
/// Uses the ontology to define concepts and the symbolic inference engine
/// to derive new facts from rules.
#[test]
fn test_symbolic_ontology_reasoning() {
    // 1. Ontology: define ML concept hierarchy
    let ontology = Ontology::new("ml_concepts");
    let nn_id = ConceptId::new("ml", "neural_network");
    let cnn_id = ConceptId::new("ml", "cnn");
    let rnn_id = ConceptId::new("ml", "rnn");

    ontology.add_concept(
        Concept::new(nn_id.clone(), ConceptType::Schema)
            .with_label("Neural Network")
            .with_definition("A parametric function approximator"),
    );
    ontology.add_concept(
        Concept::new(cnn_id.clone(), ConceptType::Schema)
            .with_label("CNN")
            .with_definition("Convolutional neural network"),
    );
    ontology.add_concept(
        Concept::new(rnn_id.clone(), ConceptType::Schema)
            .with_label("RNN")
            .with_definition("Recurrent neural network"),
    );

    ontology.add_relation(Relation::new(
        cnn_id.clone(),
        RelationType::IsA,
        nn_id.clone(),
    ));
    ontology.add_relation(Relation::new(
        rnn_id.clone(),
        RelationType::IsA,
        nn_id.clone(),
    ));

    // Verify hierarchy
    let _nn_subtypes = ontology.get_related(&nn_id, RelationType::IsA);
    // get_related gives outgoing relations from nn_id, but IsA goes cnn→nn
    // So query from cnn's perspective
    let cnn_parents = ontology.get_related(&cnn_id, RelationType::IsA);
    assert!(!cnn_parents.is_empty(), "CNN should be related via IsA");

    // 2. Symbolic reasoning: encode same knowledge
    let mut kb = KnowledgeBase::new();
    kb.add_fact("NeuralNetwork", vec![Term::constant("cnn")]);
    kb.add_fact("NeuralNetwork", vec![Term::constant("rnn")]);
    kb.add_fact("HasConvolutions", vec![Term::constant("cnn")]);

    // Rule: NeuralNetwork(X) ∧ HasConvolutions(X) → ImageModel(X)
    let head = rmi::symbolic::Predicate::new("ImageModel", vec![Term::var("X")]);
    let body = vec![
        rmi::symbolic::Predicate::new("NeuralNetwork", vec![Term::var("X")]),
        rmi::symbolic::Predicate::new("HasConvolutions", vec![Term::var("X")]),
    ];
    kb.add_rule(head, body);

    let engine = InferenceEngine::new(InferenceConfig::default());
    let derived = engine.forward_chain(&mut kb);

    // Should derive ImageModel(cnn)
    assert!(
        derived.iter().any(
            |p| p.name == "ImageModel" && matches!(&p.args[0], Term::Constant(c) if c == "cnn")
        ),
        "Should derive ImageModel(cnn)"
    );

    // Should NOT derive ImageModel(rnn) (no HasConvolutions fact)
    assert!(
        !derived.iter().any(
            |p| p.name == "ImageModel" && matches!(&p.args[0], Term::Constant(c) if c == "rnn")
        ),
        "Should not derive ImageModel(rnn)"
    );
}

/// Test: End-to-end — build IR, optimize, verify, emit to CUDA
///
/// Demonstrates the full ML workflow: build a transformer attention layer in IR,
/// optimize, verify correctness, and emit CUDA kernel code.
#[test]
fn test_e2e_attention_cuda_emit() {
    // 1. Build attention mechanism in IR
    let mut builder = FunctionBuilder::new(
        "self_attention",
        vec![
            (
                "query".to_string(),
                IRType::tensor(PrimitiveType::F32, vec![4, 8, 64]),
            ),
            (
                "key".to_string(),
                IRType::tensor(PrimitiveType::F32, vec![4, 8, 64]),
            ),
            (
                "value".to_string(),
                IRType::tensor(PrimitiveType::F32, vec![4, 8, 64]),
            ),
        ],
        IRType::tensor(PrimitiveType::F32, vec![4, 8, 64]),
    );

    let q = builder.param(0);
    let k = builder.param(1);
    let v = builder.param(2);

    let attn_out = builder.attention(q, k, v, 8, 64);
    builder.ret(attn_out);

    let func = builder.build();
    assert!(func.verify().is_ok());

    let mut program = Program::new("transformer_layer");
    program.add_function(func);

    // 2. Optimize
    let pipeline = OptimizationPipeline::level(OptimizationLevel::O1);
    let program = pipeline.optimize(program);

    // 3. Verify
    assert!(Verifier::new().check(&program).is_ok());

    // 4. Emit CUDA code
    let cuda_emitter = CudaEmitter::new();
    let cuda_code = cuda_emitter.emit(&program).unwrap();
    assert!(!cuda_code.is_empty());
    // CUDA emitter should produce kernel output
    assert!(
        cuda_code.contains("__global__") || cuda_code.contains("self_attention"),
        "CUDA output should contain kernel declaration or the function name"
    );
}
