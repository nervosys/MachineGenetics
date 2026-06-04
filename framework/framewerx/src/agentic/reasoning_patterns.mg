// framewerx::agentic::reasoning_patterns — LLM orchestration scaffolds.

// Chain-of-Thought wrapper: prompts model to expose intermediate steps.
S ChainOfThought {
    style: s,
    max_steps: usize,
}

I ChainOfThought {
    +f new() -> ChainOfThought { @ChainOfThought { style: "verbose", max_steps: 16 } }
    +f compact() -> ChainOfThought { @ChainOfThought { style: "compact", max_steps: 8 } }
}

// Tree-of-Thoughts: branching exploration over reasoning paths.
S TreeOfThoughts {
    branching_factor: usize,
    max_depth: usize,
    evaluator: s,
    search: s,
}

I TreeOfThoughts {
    +f new(branching: usize, depth: usize) -> TreeOfThoughts {
        @TreeOfThoughts {
            branching_factor: branching,
            max_depth: depth,
            evaluator: "self_eval",
            search: "BFS",
        }
    }
}

// Graph-of-Thoughts: arbitrary DAG over partial-result nodes.
S GraphOfThoughts { merge_strategy: s, max_nodes: usize }

// ReAct: interleaved Reason / Act steps.
S ReAct {
    available_tools: [s]~,
    max_iterations: usize,
}

// Reflexion: self-critique then retry with the critique as context.
S Reflexion {
    max_retries: usize,
    memory_buffer: usize,
}

// Self-consistency: sample N CoT paths, majority-vote on the answer.
S SelfConsistency { num_samples: usize, temperature: f32, aggregator: s }

I SelfConsistency {
    +f new(num_samples: usize) -> SelfConsistency {
        @SelfConsistency { num_samples: num_samples, temperature: 0.7, aggregator: "majority" }
    }
}

// Plan-and-Solve: explicit plan generation before execution.
S PlanAndSolve { planner_model: s, executor_model: s }

// Skeleton-of-Thought: parallel completion of independent points.
S SkeletonOfThought { num_points: usize }
