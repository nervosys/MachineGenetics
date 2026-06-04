//! Multi-Agent Swarm Collaboration Example
//!
//! Demonstrates how multiple agents collaborate autonomously on AI models.

use rmi::prelude::*;
use std::time::Duration;
use uuid::Uuid;

fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    println!("=== RMI Multi-Agent Swarm Collaboration Demo ===\n");

    // Create Swarm Coordinator with custom configuration
    println!("1. Creating Swarm Coordinator...\n");
    let config = SwarmConfig {
        max_agents: 50,
        consensus_threshold: 0.66,
        resource_sharing: ResourceSharingMode::Cooperative,
        collaboration_strategy: CollaborationStrategy::Emergent,
        task_timeout: Duration::from_secs(300),
        auto_load_balance: true,
        heartbeat_interval: Duration::from_secs(10),
    };
    let mut coordinator = SwarmCoordinator::new(config);
    coordinator.start();
    println!("   Swarm coordinator initialized and running");

    // Register Specialized Agents
    println!("\n2. Registering Specialized Agents...\n");
    
    // Register architect agents
    let mut architect_ids = Vec::new();
    for i in 0..3 {
        let id = coordinator.register_agent(
            &format!("architect-{}", i),
            vec!["architecture_design".to_string(), "optimization".to_string()],
        )?;
        architect_ids.push(id);
        println!("   Registered: architect-{} ({:?})", i, id);
    }

    // Register trainer agents
    let mut trainer_ids = Vec::new();
    for i in 0..5 {
        let id = coordinator.register_agent(
            &format!("trainer-{}", i),
            vec!["training".to_string(), "evaluation".to_string()],
        )?;
        trainer_ids.push(id);
        println!("   Registered: trainer-{} ({:?})", i, id);
    }

    // Register evaluator agents
    let mut evaluator_ids = Vec::new();
    for i in 0..2 {
        let id = coordinator.register_agent(
            &format!("evaluator-{}", i),
            vec!["evaluation".to_string(), "merging".to_string()],
        )?;
        evaluator_ids.push(id);
        println!("   Registered: evaluator-{} ({:?})", i, id);
    }

    println!("\n   Total agents: {}", coordinator.agent_count());

    // Create Model Development Pipeline
    println!("\n3. Creating Model Development Pipeline...\n");
    let constraints = ModelConstraints {
        max_latency_ms: Some(100.0),
        max_memory_bytes: Some(512 * 1024 * 1024),
        target_accuracy: Some(0.95),
        ..Default::default()
    };

    let pipeline = ModelDevelopmentPipeline::new("image_classification")
        .with_constraints(constraints.clone());
    println!("   Pipeline: {} problem", pipeline.problem_type);
    
    println!("   Generated {} workflow stages", pipeline.workflow.stages.len());
    for stage in &pipeline.workflow.stages {
        println!("     - {} ({:?})", stage.name, stage.stage_type);
    }

    // Submit Tasks
    println!("\n4. Submitting Tasks...\n");
    let arch_task = SwarmTask::new(
        SwarmTaskType::DesignArchitecture {
            input_shape: vec![224, 224, 3],
            output_shape: vec![1000],
            constraints: constraints.clone(),
        },
        "Design CNN architecture for image classification",
    )
    .with_priority(TaskPriority::High);
    
    let task_id = coordinator.submit_task(arch_task);
    println!("   Submitted architecture task: {:?}", task_id);

    let train_task = SwarmTask::new(
        SwarmTaskType::TrainModel {
            architecture_id: Uuid::new_v4(),
            config: TrainingConfig {
                epochs: 100,
                batch_size: 32,
                learning_rate: 0.001,
                optimizer: "adam".to_string(),
                ..Default::default()
            },
        },
        "Train the designed model",
    )
    .with_priority(TaskPriority::Normal)
    .with_dependency(task_id);

    let train_id = coordinator.submit_task(train_task);
    println!("   Submitted training task: {:?} (depends on {:?})", train_id, task_id);

    // Run scheduler to assign tasks
    coordinator.schedule();
    println!("\n   Scheduler ran - {} pending, {} active tasks", 
        coordinator.pending_tasks(), coordinator.active_tasks());

    // Consensus Protocol Demo
    println!("\n5. Demonstrating Consensus Protocol...\n");
    let proposal = Proposal::new(
        ProposalType::Architecture(Uuid::new_v4()),
        architect_ids[0],
        "Propose ResNet-50 with SE blocks for image classification",
    );
    let proposal_id = coordinator.create_proposal(proposal);
    println!("   Created proposal: {:?}", proposal_id);

    // Agents vote on the proposal
    for (i, &agent_id) in architect_ids.iter().skip(1).chain(trainer_ids.iter().take(2)).enumerate() {
        let vote = Vote::new(agent_id, VoteDecision::Approve)
            .with_confidence(0.9 - (i as f64 * 0.05))
            .with_justification("Good architecture choice for the task");
        coordinator.submit_vote(proposal_id, vote)?;
        println!("   Agent {:?} voted Approve", agent_id);
    }

    if let Some(prop) = coordinator.proposal_status(proposal_id) {
        println!("\n   Proposal status: {:?}", prop.status);
        println!("   Approval ratio: {:.0}%", prop.approval_ratio() * 100.0);
    }

    // Autonomous Model Builder
    println!("\n6. Using Autonomous Model Builder...\n");
    let (auto_pipeline, auto_config) = AutonomousModelBuilder::new("nlp")
        .with_constraints(ModelConstraints {
            max_latency_ms: Some(50.0),
            target_accuracy: Some(0.90),
            ..Default::default()
        })
        .max_iterations(100)
        .collaboration_strategy(CollaborationStrategy::Competitive)
        .target_metric("f1_score", 0.88)
        .build();
    println!("   Pipeline: {} problem", auto_pipeline.problem_type);
    println!("   Strategy: {:?}", auto_config.collaboration_strategy);

    // Custom Workflow
    println!("\n7. Creating Custom Workflow...\n");
    let custom_workflow = CollaborativeWorkflow::new("Custom Development")
        .add_stage(
            WorkflowStage::new("exploration", StageType::Parallel)
                .with_completion(CompletionCriteria::MinTasks(3))
                .with_timeout(Duration::from_secs(1800)),
        )
        .add_stage(
            WorkflowStage::new("consensus", StageType::Consensus)
                .with_completion(CompletionCriteria::ConsensusReached(0.66)),
        )
        .add_stage(
            WorkflowStage::new("training", StageType::MapReduce)
                .with_completion(CompletionCriteria::QualityThreshold {
                    metric: "accuracy".to_string(),
                    threshold: 0.95,
                }),
        );
    
    for (i, stage) in custom_workflow.stages.iter().enumerate() {
        println!("   Stage {}: {} ({:?})", i + 1, stage.name, stage.stage_type);
    }
    
    let workflow_id = coordinator.start_workflow(custom_workflow);
    println!("\n   Started workflow: {:?}", workflow_id);

    if let Some(wf) = coordinator.workflow_status(workflow_id) {
        println!("   Workflow status: {:?}", wf.status);
    }

    // Final Status
    println!("\n8. Final Status...\n");
    println!("   Total agents: {}", coordinator.agent_count());
    println!("   Pending tasks: {}", coordinator.pending_tasks());
    println!("   Active tasks: {}", coordinator.active_tasks());
    println!("   Coordinator running: {}", coordinator.is_running());

    println!("\n=== Demo Complete ===");
    println!("\nRMI Swarm enables:");
    println!("  - Autonomous multi-agent AI model development");
    println!("  - Consensus-based architecture selection");
    println!("  - Parallel, competitive, and emergent collaboration");
    println!("  - Distributed training coordination");
    println!("  - Flexible workflow orchestration");

    Ok(())
}
