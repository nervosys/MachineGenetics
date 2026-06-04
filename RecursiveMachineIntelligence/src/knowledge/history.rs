//! AI History Knowledgebase
//!
//! A machine-readable ontology of progress in AI from the seminal
//! McCulloch-Pitts paper (1943) through modern large language models.
//! Designed for AI agents to reason about the evolution of AI.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Era of AI development
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AIEra {
    /// 1943-1955: Foundational work
    Foundations,
    /// 1956-1969: Birth of AI as a field
    BirthOfAI,
    /// 1970-1979: Knowledge-based systems
    KnowledgeSystems,
    /// 1980-1987: Expert systems boom
    ExpertSystems,
    /// 1988-1993: AI Winter
    AIWinter,
    /// 1994-2005: Statistical ML rises
    StatisticalML,
    /// 2006-2011: Deep learning begins
    DeepLearningDawn,
    /// 2012-2016: Deep learning revolution
    DeepLearningRevolution,
    /// 2017-2019: Transformer era begins
    TransformerEra,
    /// 2020-present: Large language models
    LLMEra,
}

impl AIEra {
    /// Get date range for this era
    pub fn date_range(&self) -> (i32, i32) {
        match self {
            AIEra::Foundations => (1943, 1955),
            AIEra::BirthOfAI => (1956, 1969),
            AIEra::KnowledgeSystems => (1970, 1979),
            AIEra::ExpertSystems => (1980, 1987),
            AIEra::AIWinter => (1988, 1993),
            AIEra::StatisticalML => (1994, 2005),
            AIEra::DeepLearningDawn => (2006, 2011),
            AIEra::DeepLearningRevolution => (2012, 2016),
            AIEra::TransformerEra => (2017, 2019),
            AIEra::LLMEra => (2020, 2025),
        }
    }
}

/// Category of AI contribution
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ContributionCategory {
    /// Theoretical foundations
    Theory,
    /// Neural network architectures
    Architecture,
    /// Learning algorithms
    Algorithm,
    /// Hardware and compute
    Hardware,
    /// Datasets and benchmarks
    Benchmark,
    /// Applications
    Application,
    /// Symbolic AI methods
    Symbolic,
    /// Optimization techniques
    Optimization,
    /// Language and NLP
    Language,
    /// Vision and perception
    Vision,
    /// Reinforcement learning
    Reinforcement,
    /// Multi-agent systems
    MultiAgent,
}

/// A seminal paper or contribution in AI history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIContribution {
    /// Unique identifier
    pub id: Uuid,

    /// Short title
    pub title: String,

    /// Full citation
    pub citation: String,

    /// Authors
    pub authors: Vec<String>,

    /// Publication year
    pub year: i32,

    /// Publication venue
    pub venue: String,

    /// Era of AI
    pub era: AIEra,

    /// Categories
    pub categories: Vec<ContributionCategory>,

    /// Key concepts introduced
    pub concepts: Vec<String>,

    /// Machine-readable summary
    pub summary: String,

    /// Impact description
    pub impact: String,

    /// Related contributions (by ID)
    pub builds_on: Vec<Uuid>,

    /// Contributions this enabled (by ID)
    pub enabled: Vec<Uuid>,

    /// Key equations or formulas (in LaTeX)
    pub equations: Vec<String>,

    /// Tags for search
    pub tags: Vec<String>,
}

impl AIContribution {
    /// Create a new contribution
    pub fn new(
        title: impl Into<String>,
        authors: Vec<&str>,
        year: i32,
        venue: impl Into<String>,
    ) -> Self {
        let title = title.into();
        let authors: Vec<String> = authors.into_iter().map(|s| s.to_string()).collect();
        let citation = format!(
            "{} et al. ({}) {}",
            authors.first().unwrap_or(&"Unknown".to_string()),
            year,
            title
        );

        let era = Self::determine_era(year);

        Self {
            id: Uuid::new_v4(),
            citation,
            title,
            authors,
            year,
            venue: venue.into(),
            era,
            categories: Vec::new(),
            concepts: Vec::new(),
            summary: String::new(),
            impact: String::new(),
            builds_on: Vec::new(),
            enabled: Vec::new(),
            equations: Vec::new(),
            tags: Vec::new(),
        }
    }

    fn determine_era(year: i32) -> AIEra {
        match year {
            1943..=1955 => AIEra::Foundations,
            1956..=1969 => AIEra::BirthOfAI,
            1970..=1979 => AIEra::KnowledgeSystems,
            1980..=1987 => AIEra::ExpertSystems,
            1988..=1993 => AIEra::AIWinter,
            1994..=2005 => AIEra::StatisticalML,
            2006..=2011 => AIEra::DeepLearningDawn,
            2012..=2016 => AIEra::DeepLearningRevolution,
            2017..=2019 => AIEra::TransformerEra,
            _ => AIEra::LLMEra,
        }
    }

    /// Add categories
    pub fn with_categories(mut self, cats: Vec<ContributionCategory>) -> Self {
        self.categories = cats;
        self
    }

    /// Add concepts
    pub fn with_concepts(mut self, concepts: Vec<&str>) -> Self {
        self.concepts = concepts.into_iter().map(|s| s.to_string()).collect();
        self
    }

    /// Add summary
    pub fn with_summary(mut self, summary: impl Into<String>) -> Self {
        self.summary = summary.into();
        self
    }

    /// Add impact
    pub fn with_impact(mut self, impact: impl Into<String>) -> Self {
        self.impact = impact.into();
        self
    }

    /// Add equations
    pub fn with_equations(mut self, equations: Vec<&str>) -> Self {
        self.equations = equations.into_iter().map(|s| s.to_string()).collect();
        self
    }

    /// Add tags
    pub fn with_tags(mut self, tags: Vec<&str>) -> Self {
        self.tags = tags.into_iter().map(|s| s.to_string()).collect();
        self
    }
}

/// The AI History Knowledgebase
pub struct AIHistoryKB {
    /// All contributions indexed by ID
    contributions: HashMap<Uuid, AIContribution>,

    /// Index by year
    by_year: HashMap<i32, Vec<Uuid>>,

    /// Index by era
    by_era: HashMap<AIEra, Vec<Uuid>>,

    /// Index by category
    by_category: HashMap<ContributionCategory, Vec<Uuid>>,

    /// Index by concept
    by_concept: HashMap<String, Vec<Uuid>>,

    /// Index by author
    by_author: HashMap<String, Vec<Uuid>>,
}

impl AIHistoryKB {
    /// Create a new empty knowledgebase
    pub fn new() -> Self {
        Self {
            contributions: HashMap::new(),
            by_year: HashMap::new(),
            by_era: HashMap::new(),
            by_category: HashMap::new(),
            by_concept: HashMap::new(),
            by_author: HashMap::new(),
        }
    }

    /// Create a knowledgebase populated with the history of AI
    pub fn with_history() -> Self {
        let mut kb = Self::new();
        kb.populate_history();
        kb
    }

    /// Add a contribution
    pub fn add(&mut self, contrib: AIContribution) -> Uuid {
        let id = contrib.id;

        // Index by year
        self.by_year.entry(contrib.year).or_default().push(id);

        // Index by era
        self.by_era.entry(contrib.era).or_default().push(id);

        // Index by category
        for cat in &contrib.categories {
            self.by_category.entry(*cat).or_default().push(id);
        }

        // Index by concept
        for concept in &contrib.concepts {
            self.by_concept
                .entry(concept.to_lowercase())
                .or_default()
                .push(id);
        }

        // Index by author
        for author in &contrib.authors {
            self.by_author
                .entry(author.to_lowercase())
                .or_default()
                .push(id);
        }

        self.contributions.insert(id, contrib);
        id
    }

    /// Get contribution by ID
    pub fn get(&self, id: &Uuid) -> Option<&AIContribution> {
        self.contributions.get(id)
    }

    /// Get contributions by year
    pub fn by_year(&self, year: i32) -> Vec<&AIContribution> {
        self.by_year
            .get(&year)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.contributions.get(id))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get contributions by era
    pub fn by_era(&self, era: AIEra) -> Vec<&AIContribution> {
        self.by_era
            .get(&era)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.contributions.get(id))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get contributions by category
    pub fn by_category(&self, cat: ContributionCategory) -> Vec<&AIContribution> {
        self.by_category
            .get(&cat)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.contributions.get(id))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Search by concept
    pub fn search_concept(&self, concept: &str) -> Vec<&AIContribution> {
        self.by_concept
            .get(&concept.to_lowercase())
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.contributions.get(id))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Search by author
    pub fn search_author(&self, author: &str) -> Vec<&AIContribution> {
        self.by_author
            .get(&author.to_lowercase())
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.contributions.get(id))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get all contributions in chronological order
    pub fn chronological(&self) -> Vec<&AIContribution> {
        let mut contribs: Vec<_> = self.contributions.values().collect();
        contribs.sort_by_key(|c| c.year);
        contribs
    }

    /// Get the lineage of a contribution (what it builds on, recursively)
    pub fn lineage(&self, id: &Uuid) -> Vec<&AIContribution> {
        let mut result = Vec::new();
        let mut visited = std::collections::HashSet::new();
        self.collect_lineage(id, &mut result, &mut visited);
        result.reverse();
        result
    }

    fn collect_lineage<'a>(
        &'a self,
        id: &Uuid,
        result: &mut Vec<&'a AIContribution>,
        visited: &mut std::collections::HashSet<Uuid>,
    ) {
        if visited.contains(id) {
            return;
        }
        visited.insert(*id);

        if let Some(contrib) = self.contributions.get(id) {
            for parent_id in &contrib.builds_on {
                self.collect_lineage(parent_id, result, visited);
            }
            result.push(contrib);
        }
    }

    /// Populate with AI history
    fn populate_history(&mut self) {
        // === FOUNDATIONS (1943-1955) ===

        let mcculloch_pitts = AIContribution::new(
            "A Logical Calculus of the Ideas Immanent in Nervous Activity",
            vec!["Warren McCulloch", "Walter Pitts"],
            1943,
            "Bulletin of Mathematical Biophysics",
        )
        .with_categories(vec![ContributionCategory::Theory, ContributionCategory::Architecture])
        .with_concepts(vec!["artificial neuron", "threshold logic", "neural computation", "McCulloch-Pitts neuron"])
        .with_summary("First mathematical model of artificial neurons. Showed that networks of simple threshold units can compute any logical function.")
        .with_impact("Founded computational neuroscience and neural network research. Inspired all subsequent neural network architectures.")
        .with_equations(vec![
            r"y = \theta\left(\sum_i w_i x_i - T\right)",
            r"\theta(x) = \begin{cases} 1 & x \geq 0 \\ 0 & x < 0 \end{cases}",
        ])
        .with_tags(vec!["neural-networks", "foundations", "computation-theory"]);
        let mcp_id = self.add(mcculloch_pitts);

        let hebb = AIContribution::new(
            "The Organization of Behavior",
            vec!["Donald Hebb"],
            1949,
            "Wiley",
        )
        .with_categories(vec![ContributionCategory::Theory, ContributionCategory::Algorithm])
        .with_concepts(vec!["Hebbian learning", "cell assemblies", "synaptic plasticity", "associative learning"])
        .with_summary("Proposed that neurons that fire together wire together. First learning rule for neural networks.")
        .with_impact("Foundation of unsupervised learning. Influenced development of all subsequent learning algorithms.")
        .with_equations(vec![
            r"\Delta w_{ij} = \eta x_i x_j",
        ])
        .with_tags(vec!["learning", "plasticity", "unsupervised"]);
        let hebb_id = self.add(hebb);

        let turing_computing = AIContribution::new(
            "Computing Machinery and Intelligence",
            vec!["Alan Turing"],
            1950,
            "Mind",
        )
        .with_categories(vec![ContributionCategory::Theory])
        .with_concepts(vec!["Turing test", "machine intelligence", "imitation game", "thinking machines"])
        .with_summary("Proposed the imitation game (Turing test) as criterion for machine intelligence. Asked 'Can machines think?'")
        .with_impact("Defined the philosophical framework for AI. The Turing test remains influential in AI evaluation.")
        .with_tags(vec!["philosophy", "intelligence", "evaluation"]);
        let _turing_id = self.add(turing_computing);

        // === BIRTH OF AI (1956-1969) ===

        let dartmouth = AIContribution::new(
            "A Proposal for the Dartmouth Summer Research Project on Artificial Intelligence",
            vec!["John McCarthy", "Marvin Minsky", "Nathaniel Rochester", "Claude Shannon"],
            1956,
            "Dartmouth Conference",
        )
        .with_categories(vec![ContributionCategory::Theory])
        .with_concepts(vec!["artificial intelligence", "symbolic AI", "knowledge representation"])
        .with_summary("The founding document of AI as a field. Coined the term 'artificial intelligence'.")
        .with_impact("Established AI as an independent field of research. Defined the research agenda for decades.")
        .with_tags(vec!["history", "symbolic-ai", "foundations"]);
        let _dartmouth_id = self.add(dartmouth);

        let perceptron = AIContribution::new(
            "The Perceptron: A Probabilistic Model for Information Storage and Organization in the Brain",
            vec!["Frank Rosenblatt"],
            1958,
            "Psychological Review",
        )
        .with_categories(vec![ContributionCategory::Architecture, ContributionCategory::Algorithm])
        .with_concepts(vec!["perceptron", "perceptron learning rule", "linear classifier", "convergence theorem"])
        .with_summary("First trainable neural network. Proved convergence theorem for linearly separable data.")
        .with_impact("Demonstrated that machines could learn from data. Sparked first wave of neural network enthusiasm.")
        .with_equations(vec![
            r"y = \text{sign}\left(\sum_i w_i x_i + b\right)",
            r"w_i \leftarrow w_i + \eta (y - \hat{y}) x_i",
        ])
        .with_tags(vec!["neural-networks", "learning", "classification"]);
        let mut perceptron_contrib = perceptron;
        perceptron_contrib.builds_on = vec![mcp_id, hebb_id];
        let perceptron_id = self.add(perceptron_contrib);

        let lisp = AIContribution::new(
            "Recursive Functions of Symbolic Expressions and Their Computation by Machine",
            vec!["John McCarthy"],
            1960,
            "Communications of the ACM",
        )
        .with_categories(vec![ContributionCategory::Symbolic, ContributionCategory::Application])
        .with_concepts(vec!["LISP", "symbolic computation", "garbage collection", "recursion", "list processing"])
        .with_summary("Introduced LISP, the first programming language designed for AI. Based on lambda calculus.")
        .with_impact("Dominant AI programming language for decades. Influenced all functional programming languages.")
        .with_tags(vec!["programming-language", "symbolic-ai", "computation"]);
        self.add(lisp);

        let widrow_hoff = AIContribution::new(
            "Adaptive Switching Circuits",
            vec!["Bernard Widrow", "Marcian Hoff"],
            1960,
            "IRE WESCON Convention Record",
        )
        .with_categories(vec![ContributionCategory::Algorithm, ContributionCategory::Optimization])
        .with_concepts(vec!["ADALINE", "LMS algorithm", "delta rule", "gradient descent"])
        .with_summary("Introduced ADALINE and the Least Mean Squares (LMS) algorithm for training linear units.")
        .with_impact("First practical gradient-based learning algorithm. Foundation for backpropagation.")
        .with_equations(vec![
            r"w \leftarrow w + \eta (y - w^T x) x",
            r"J = \frac{1}{2}(y - w^T x)^2",
        ])
        .with_tags(vec!["optimization", "learning", "adaptive-systems"]);
        let widrow_id = self.add(widrow_hoff);

        // === KNOWLEDGE SYSTEMS (1970-1979) ===

        let minsky_papert = AIContribution::new(
            "Perceptrons: An Introduction to Computational Geometry",
            vec!["Marvin Minsky", "Seymour Papert"],
            1969,
            "MIT Press",
        )
        .with_categories(vec![ContributionCategory::Theory])
        .with_concepts(vec![
            "XOR problem",
            "linear separability",
            "computational limitations",
        ])
        .with_summary(
            "Proved limitations of single-layer perceptrons, including inability to learn XOR.",
        )
        .with_impact(
            "Contributed to first AI winter. Delayed neural network research for over a decade.",
        )
        .with_tags(vec!["limitations", "perceptrons", "theory"]);
        self.add(minsky_papert);

        let werbos_backprop = AIContribution::new(
            "Beyond Regression: New Tools for Prediction and Analysis in the Behavioral Sciences",
            vec!["Paul Werbos"],
            1974,
            "PhD Thesis, Harvard",
        )
        .with_categories(vec![ContributionCategory::Algorithm, ContributionCategory::Optimization])
        .with_concepts(vec!["backpropagation", "automatic differentiation", "chain rule", "credit assignment"])
        .with_summary("First description of backpropagation for training multi-layer neural networks.")
        .with_impact("Essential algorithm for deep learning. Overlooked until rediscovered in 1986.")
        .with_equations(vec![
            r"\frac{\partial L}{\partial w_{ij}} = \frac{\partial L}{\partial a_j} \frac{\partial a_j}{\partial w_{ij}}",
            r"\delta_j = \frac{\partial L}{\partial a_j} = \sum_k \delta_k w_{jk} \sigma'(z_j)",
        ])
        .with_tags(vec!["backpropagation", "gradient", "learning"]);
        let werbos_id = self.add(werbos_backprop);

        // === EXPERT SYSTEMS ERA (1980-1987) ===

        let hopfield = AIContribution::new(
            "Neural Networks and Physical Systems with Emergent Collective Computational Abilities",
            vec!["John Hopfield"],
            1982,
            "PNAS",
        )
        .with_categories(vec![
            ContributionCategory::Architecture,
            ContributionCategory::Theory,
        ])
        .with_concepts(vec![
            "Hopfield network",
            "associative memory",
            "energy function",
            "attractor dynamics",
        ])
        .with_summary("Introduced recurrent networks with energy functions for associative memory.")
        .with_impact(
            "Revived neural network research. Connected neural networks to statistical physics.",
        )
        .with_equations(vec![
            r"E = -\frac{1}{2}\sum_{i,j} w_{ij} s_i s_j - \sum_i b_i s_i",
            r"s_i \leftarrow \text{sign}\left(\sum_j w_{ij} s_j + b_i\right)",
        ])
        .with_tags(vec!["recurrent", "memory", "physics"]);
        let hopfield_id = self.add(hopfield);

        let boltzmann = AIContribution::new(
            "A Learning Algorithm for Boltzmann Machines",
            vec!["David Ackley", "Geoffrey Hinton", "Terrence Sejnowski"],
            1985,
            "Cognitive Science",
        )
        .with_categories(vec![ContributionCategory::Architecture, ContributionCategory::Algorithm])
        .with_concepts(vec!["Boltzmann machine", "stochastic neurons", "simulated annealing", "hidden units"])
        .with_summary("First neural network with hidden units and a principled learning algorithm.")
        .with_impact("Foundation for deep belief networks and restricted Boltzmann machines.")
        .with_equations(vec![
            r"P(v, h) = \frac{1}{Z} e^{-E(v,h)}",
            r"\Delta w_{ij} = \epsilon (\langle s_i s_j \rangle_{data} - \langle s_i s_j \rangle_{model})",
        ])
        .with_tags(vec!["probabilistic", "generative", "unsupervised"]);
        let boltzmann_id = self.add(boltzmann);

        let backprop_nature = AIContribution::new(
            "Learning Representations by Back-propagating Errors",
            vec!["David Rumelhart", "Geoffrey Hinton", "Ronald Williams"],
            1986,
            "Nature",
        )
        .with_categories(vec![ContributionCategory::Algorithm, ContributionCategory::Theory])
        .with_concepts(vec!["backpropagation", "multi-layer perceptron", "representation learning", "hidden layers"])
        .with_summary("Popularized backpropagation for training multi-layer networks. Showed hidden units learn useful representations.")
        .with_impact("Sparked renewed interest in neural networks. Foundation of modern deep learning.")
        .with_equations(vec![
            r"\frac{\partial E}{\partial w_{ji}} = \delta_j o_i",
            r"\delta_j = o_j(1-o_j)\sum_k \delta_k w_{kj}",
        ])
        .with_tags(vec!["backpropagation", "deep-learning", "representations"]);
        let mut bp_contrib = backprop_nature;
        bp_contrib.builds_on = vec![werbos_id, widrow_id, perceptron_id];
        let bp_id = self.add(bp_contrib);

        // === STATISTICAL ML (1994-2005) ===

        let svm = AIContribution::new(
            "Support-Vector Networks",
            vec!["Corinna Cortes", "Vladimir Vapnik"],
            1995,
            "Machine Learning",
        )
        .with_categories(vec![ContributionCategory::Algorithm, ContributionCategory::Theory])
        .with_concepts(vec!["support vector machine", "kernel trick", "maximum margin", "VC dimension"])
        .with_summary("Introduced Support Vector Machines with kernel methods for nonlinear classification.")
        .with_impact("Dominated machine learning for a decade. Provided theoretical foundations via VC theory.")
        .with_equations(vec![
            r"\min_{w,b} \frac{1}{2}||w||^2 \text{ s.t. } y_i(w \cdot x_i + b) \geq 1",
            r"K(x, x') = \phi(x) \cdot \phi(x')",
        ])
        .with_tags(vec!["classification", "kernel", "optimization"]);
        self.add(svm);

        let lstm = AIContribution::new(
            "Long Short-Term Memory",
            vec!["Sepp Hochreiter", "Jürgen Schmidhuber"],
            1997,
            "Neural Computation",
        )
        .with_categories(vec![ContributionCategory::Architecture])
        .with_concepts(vec!["LSTM", "gating", "memory cell", "vanishing gradient", "sequence modeling"])
        .with_summary("Introduced gated memory cells to address vanishing gradient problem in RNNs.")
        .with_impact("Enabled learning long-range dependencies. Standard architecture for sequence tasks until transformers.")
        .with_equations(vec![
            r"f_t = \sigma(W_f \cdot [h_{t-1}, x_t] + b_f)",
            r"i_t = \sigma(W_i \cdot [h_{t-1}, x_t] + b_i)",
            r"C_t = f_t \odot C_{t-1} + i_t \odot \tanh(W_C \cdot [h_{t-1}, x_t] + b_C)",
        ])
        .with_tags(vec!["recurrent", "sequence", "memory"]);
        let mut lstm_contrib = lstm;
        lstm_contrib.builds_on = vec![bp_id, hopfield_id];
        let lstm_id = self.add(lstm_contrib);

        // === DEEP LEARNING DAWN (2006-2011) ===

        let dbn = AIContribution::new(
            "A Fast Learning Algorithm for Deep Belief Nets",
            vec!["Geoffrey Hinton", "Simon Osindero", "Yee-Whye Teh"],
            2006,
            "Neural Computation",
        )
        .with_categories(vec![ContributionCategory::Architecture, ContributionCategory::Algorithm])
        .with_concepts(vec!["deep belief network", "pretraining", "RBM", "greedy layer-wise training"])
        .with_summary("Showed how to train deep networks using layer-wise pretraining with RBMs.")
        .with_impact("Launched the deep learning revolution. Solved the problem of training very deep networks.")
        .with_tags(vec!["deep-learning", "pretraining", "generative"]);
        let mut dbn_contrib = dbn;
        dbn_contrib.builds_on = vec![bp_id, boltzmann_id];
        let dbn_id = self.add(dbn_contrib);

        let imagenet = AIContribution::new(
            "ImageNet: A Large-Scale Hierarchical Image Database",
            vec!["Jia Deng", "Wei Dong", "Richard Socher", "Li-Jia Li", "Kai Li", "Li Fei-Fei"],
            2009,
            "CVPR",
        )
        .with_categories(vec![ContributionCategory::Benchmark, ContributionCategory::Vision])
        .with_concepts(vec!["ImageNet", "large-scale dataset", "image classification", "object recognition"])
        .with_summary("Created massive image dataset with 14M+ images in 20K+ categories using WordNet hierarchy.")
        .with_impact("Enabled training of large vision models. ImageNet challenge drove deep learning progress.")
        .with_tags(vec!["dataset", "vision", "benchmark"]);
        let imagenet_id = self.add(imagenet);

        // === DEEP LEARNING REVOLUTION (2012-2016) ===

        let alexnet = AIContribution::new(
            "ImageNet Classification with Deep Convolutional Neural Networks",
            vec!["Alex Krizhevsky", "Ilya Sutskever", "Geoffrey Hinton"],
            2012,
            "NeurIPS",
        )
        .with_categories(vec![ContributionCategory::Architecture, ContributionCategory::Vision])
        .with_concepts(vec!["AlexNet", "CNN", "ReLU", "dropout", "GPU training", "deep learning"])
        .with_summary("Won ImageNet 2012 by large margin using deep CNN trained on GPUs. Used ReLU and dropout.")
        .with_impact("Proved deep learning at scale. Started modern deep learning era. GPUs became essential.")
        .with_tags(vec!["cnn", "vision", "gpu", "breakthrough"]);
        let mut alexnet_contrib = alexnet;
        alexnet_contrib.builds_on = vec![dbn_id, bp_id, imagenet_id];
        let alexnet_id = self.add(alexnet_contrib);

        let dropout = AIContribution::new(
            "Dropout: A Simple Way to Prevent Neural Networks from Overfitting",
            vec!["Nitish Srivastava", "Geoffrey Hinton", "Alex Krizhevsky", "Ilya Sutskever", "Ruslan Salakhutdinov"],
            2014,
            "JMLR",
        )
        .with_categories(vec![ContributionCategory::Algorithm, ContributionCategory::Optimization])
        .with_concepts(vec!["dropout", "regularization", "ensemble", "stochastic training"])
        .with_summary("Random neuron dropping during training prevents overfitting and improves generalization.")
        .with_impact("Standard regularization technique. Simple but highly effective.")
        .with_equations(vec![
            r"r_j \sim \text{Bernoulli}(p)",
            r"\tilde{y} = r \odot y",
        ])
        .with_tags(vec!["regularization", "training", "generalization"]);
        self.add(dropout);

        let adam = AIContribution::new(
            "Adam: A Method for Stochastic Optimization",
            vec!["Diederik Kingma", "Jimmy Ba"],
            2015,
            "ICLR",
        )
        .with_categories(vec![ContributionCategory::Optimization])
        .with_concepts(vec!["Adam", "adaptive learning rate", "momentum", "RMSprop"])
        .with_summary("Combined momentum with adaptive learning rates. Default optimizer for deep learning.")
        .with_impact("Most widely used optimizer. Enabled training of diverse architectures with minimal tuning.")
        .with_equations(vec![
            r"m_t = \beta_1 m_{t-1} + (1-\beta_1) g_t",
            r"v_t = \beta_2 v_{t-1} + (1-\beta_2) g_t^2",
            r"\theta_t = \theta_{t-1} - \alpha \frac{\hat{m}_t}{\sqrt{\hat{v}_t} + \epsilon}",
        ])
        .with_tags(vec!["optimizer", "adaptive", "gradient-descent"]);
        self.add(adam);

        let batchnorm = AIContribution::new(
            "Batch Normalization: Accelerating Deep Network Training",
            vec!["Sergey Ioffe", "Christian Szegedy"],
            2015,
            "ICML",
        )
        .with_categories(vec![
            ContributionCategory::Algorithm,
            ContributionCategory::Architecture,
        ])
        .with_concepts(vec![
            "batch normalization",
            "internal covariate shift",
            "normalization",
            "deep training",
        ])
        .with_summary("Normalizes layer inputs during training, enabling much deeper networks.")
        .with_impact(
            "Standard component in modern architectures. Dramatically accelerated training.",
        )
        .with_equations(vec![
            r"\hat{x} = \frac{x - \mu_B}{\sqrt{\sigma_B^2 + \epsilon}}",
            r"y = \gamma \hat{x} + \beta",
        ])
        .with_tags(vec!["normalization", "training", "deep-networks"]);
        self.add(batchnorm);

        let resnet = AIContribution::new(
            "Deep Residual Learning for Image Recognition",
            vec!["Kaiming He", "Xiangyu Zhang", "Shaoqing Ren", "Jian Sun"],
            2016,
            "CVPR",
        )
        .with_categories(vec![ContributionCategory::Architecture, ContributionCategory::Vision])
        .with_concepts(vec!["ResNet", "residual connection", "skip connection", "identity mapping"])
        .with_summary("Introduced skip connections enabling training of 100+ layer networks.")
        .with_impact("Solved degradation problem in very deep networks. Residual connections now ubiquitous.")
        .with_equations(vec![
            r"y = F(x, \{W_i\}) + x",
            r"\frac{\partial L}{\partial x} = \frac{\partial L}{\partial y}(1 + \frac{\partial F}{\partial x})",
        ])
        .with_tags(vec!["residual", "deep", "vision"]);
        let mut resnet_contrib = resnet;
        resnet_contrib.builds_on = vec![alexnet_id];
        let resnet_id = self.add(resnet_contrib);

        // === TRANSFORMER ERA (2017-2019) ===

        let transformer = AIContribution::new(
            "Attention Is All You Need",
            vec![
                "Ashish Vaswani",
                "Noam Shazeer",
                "Niki Parmar",
                "Jakob Uszkoreit",
                "Llion Jones",
                "Aidan Gomez",
                "Lukasz Kaiser",
                "Illia Polosukhin",
            ],
            2017,
            "NeurIPS",
        )
        .with_categories(vec![
            ContributionCategory::Architecture,
            ContributionCategory::Language,
        ])
        .with_concepts(vec![
            "transformer",
            "self-attention",
            "multi-head attention",
            "positional encoding",
        ])
        .with_summary(
            "Introduced attention-only architecture replacing RNNs for sequence modeling.",
        )
        .with_impact("Foundation of all modern LLMs. Transformed NLP and increasingly all of ML.")
        .with_equations(vec![
            r"\text{Attention}(Q,K,V) = \text{softmax}\left(\frac{QK^T}{\sqrt{d_k}}\right)V",
            r"\text{MultiHead}(Q,K,V) = \text{Concat}(\text{head}_1,...,\text{head}_h)W^O",
        ])
        .with_tags(vec!["transformer", "attention", "nlp", "breakthrough"]);
        let mut transformer_contrib = transformer;
        transformer_contrib.builds_on = vec![lstm_id, resnet_id];
        let transformer_id = self.add(transformer_contrib);

        let bert = AIContribution::new(
            "BERT: Pre-training of Deep Bidirectional Transformers for Language Understanding",
            vec![
                "Jacob Devlin",
                "Ming-Wei Chang",
                "Kenton Lee",
                "Kristina Toutanova",
            ],
            2019,
            "NAACL",
        )
        .with_categories(vec![
            ContributionCategory::Architecture,
            ContributionCategory::Language,
        ])
        .with_concepts(vec![
            "BERT",
            "masked language modeling",
            "pretraining",
            "bidirectional",
            "fine-tuning",
        ])
        .with_summary("Introduced bidirectional pretraining with masked language modeling.")
        .with_impact("Set new benchmarks across NLP tasks. Established pretrain-finetune paradigm.")
        .with_tags(vec!["pretraining", "nlp", "transfer-learning"]);
        let mut bert_contrib = bert;
        bert_contrib.builds_on = vec![transformer_id];
        let bert_id = self.add(bert_contrib);

        let gpt2 = AIContribution::new(
            "Language Models are Unsupervised Multitask Learners",
            vec!["Alec Radford", "Jeffrey Wu", "Rewon Child", "David Luan", "Dario Amodei", "Ilya Sutskever"],
            2019,
            "OpenAI Blog",
        )
        .with_categories(vec![ContributionCategory::Architecture, ContributionCategory::Language])
        .with_concepts(vec!["GPT-2", "autoregressive", "language model", "zero-shot", "emergent capabilities"])
        .with_summary("Scaled up autoregressive language models. Demonstrated surprising zero-shot capabilities.")
        .with_impact("Showed scaling leads to emergent abilities. Precursor to GPT-3 and ChatGPT.")
        .with_tags(vec!["language-model", "generative", "scaling"]);
        let mut gpt2_contrib = gpt2;
        gpt2_contrib.builds_on = vec![transformer_id];
        let gpt2_id = self.add(gpt2_contrib);

        // === LLM ERA (2020-present) ===

        let gpt3 = AIContribution::new(
            "Language Models are Few-Shot Learners",
            vec![
                "Tom Brown",
                "Benjamin Mann",
                "Nick Ryder",
                "Melanie Subbiah",
                "et al.",
            ],
            2020,
            "NeurIPS",
        )
        .with_categories(vec![
            ContributionCategory::Architecture,
            ContributionCategory::Language,
        ])
        .with_concepts(vec![
            "GPT-3",
            "few-shot learning",
            "in-context learning",
            "scaling laws",
            "175B parameters",
        ])
        .with_summary(
            "175B parameter model demonstrating few-shot learning via in-context examples.",
        )
        .with_impact(
            "Established scaling laws. Showed emergence of reasoning-like capabilities at scale.",
        )
        .with_tags(vec!["llm", "few-shot", "scaling", "emergence"]);
        let mut gpt3_contrib = gpt3;
        gpt3_contrib.builds_on = vec![gpt2_id, bert_id];
        let gpt3_id = self.add(gpt3_contrib);

        let chinchilla = AIContribution::new(
            "Training Compute-Optimal Large Language Models",
            vec![
                "Jordan Hoffmann",
                "Sebastian Borgeaud",
                "Arthur Mensch",
                "et al.",
            ],
            2022,
            "NeurIPS",
        )
        .with_categories(vec![
            ContributionCategory::Theory,
            ContributionCategory::Optimization,
        ])
        .with_concepts(vec![
            "Chinchilla",
            "scaling laws",
            "compute optimal",
            "data efficiency",
        ])
        .with_summary(
            "Derived optimal ratio of model size to training data for fixed compute budget.",
        )
        .with_impact("Changed how LLMs are trained. Showed previous models were undertrained.")
        .with_equations(vec![
            r"L(N, D) = A/N^\alpha + B/D^\beta + E",
            r"N_{opt} \propto C^{0.5}, D_{opt} \propto C^{0.5}",
        ])
        .with_tags(vec!["scaling", "efficiency", "optimization"]);
        let mut chinchilla_contrib = chinchilla;
        chinchilla_contrib.builds_on = vec![gpt3_id];
        self.add(chinchilla_contrib);

        let chatgpt = AIContribution::new(
            "Training language models to follow instructions with human feedback",
            vec!["Long Ouyang", "Jeff Wu", "Xu Jiang", "et al."],
            2022,
            "NeurIPS",
        )
        .with_categories(vec![
            ContributionCategory::Algorithm,
            ContributionCategory::Language,
        ])
        .with_concepts(vec![
            "InstructGPT",
            "RLHF",
            "instruction following",
            "alignment",
            "human feedback",
        ])
        .with_summary("Used RLHF to align language models with human intent and instructions.")
        .with_impact("Foundation of ChatGPT. Made LLMs practically useful and safe.")
        .with_tags(vec!["alignment", "rlhf", "instruction-following"]);
        let mut instructgpt = chatgpt;
        instructgpt.builds_on = vec![gpt3_id];
        let instructgpt_id = self.add(instructgpt);

        let diffusion = AIContribution::new(
            "Denoising Diffusion Probabilistic Models",
            vec!["Jonathan Ho", "Ajay Jain", "Pieter Abbeel"],
            2020,
            "NeurIPS",
        )
        .with_categories(vec![ContributionCategory::Architecture, ContributionCategory::Vision])
        .with_concepts(vec!["diffusion model", "denoising", "score matching", "generative model"])
        .with_summary("Introduced diffusion models that generate images by reversing a noising process.")
        .with_impact("Foundation of DALL-E 2, Stable Diffusion, etc. Revolutionized image generation.")
        .with_equations(vec![
            r"q(x_t|x_{t-1}) = \mathcal{N}(x_t; \sqrt{1-\beta_t}x_{t-1}, \beta_t I)",
            r"p_\theta(x_{t-1}|x_t) = \mathcal{N}(x_{t-1}; \mu_\theta(x_t, t), \Sigma_\theta(x_t, t))",
        ])
        .with_tags(vec!["generative", "diffusion", "image-generation"]);
        self.add(diffusion);

        let gpt4 = AIContribution::new(
            "GPT-4 Technical Report",
            vec!["OpenAI"],
            2023,
            "arXiv",
        )
        .with_categories(vec![ContributionCategory::Architecture, ContributionCategory::Language])
        .with_concepts(vec!["GPT-4", "multimodal", "reasoning", "emergent capabilities"])
        .with_summary("Multimodal model with significantly improved reasoning and reliability.")
        .with_impact("Demonstrated near-human performance on many benchmarks. Enabled complex AI applications.")
        .with_tags(vec!["llm", "multimodal", "reasoning", "state-of-art"]);
        let mut gpt4_contrib = gpt4;
        gpt4_contrib.builds_on = vec![instructgpt_id, gpt3_id];
        self.add(gpt4_contrib);
    }
}

impl Default for AIHistoryKB {
    fn default() -> Self {
        Self::with_history()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kb_creation() {
        let kb = AIHistoryKB::with_history();
        assert!(!kb.contributions.is_empty());
    }

    #[test]
    fn test_chronological() {
        let kb = AIHistoryKB::with_history();
        let chrono = kb.chronological();

        // First should be McCulloch-Pitts 1943
        assert_eq!(chrono[0].year, 1943);

        // Should be ordered
        for i in 1..chrono.len() {
            assert!(chrono[i].year >= chrono[i - 1].year);
        }
    }

    #[test]
    fn test_by_era() {
        let kb = AIHistoryKB::with_history();

        let foundations = kb.by_era(AIEra::Foundations);
        assert!(!foundations.is_empty());

        for contrib in foundations {
            assert!(contrib.year >= 1943 && contrib.year <= 1955);
        }
    }

    #[test]
    fn test_search_concept() {
        let kb = AIHistoryKB::with_history();

        let backprop = kb.search_concept("backpropagation");
        assert!(!backprop.is_empty());
    }

    #[test]
    fn test_search_author() {
        let kb = AIHistoryKB::with_history();

        let hinton = kb.search_author("geoffrey hinton");
        assert!(!hinton.is_empty());
    }

    #[test]
    fn test_era_date_ranges() {
        assert_eq!(AIEra::Foundations.date_range(), (1943, 1955));
        assert_eq!(AIEra::BirthOfAI.date_range(), (1956, 1969));
        assert_eq!(AIEra::KnowledgeSystems.date_range(), (1970, 1979));
        assert_eq!(AIEra::ExpertSystems.date_range(), (1980, 1987));
        assert_eq!(AIEra::AIWinter.date_range(), (1988, 1993));
        assert_eq!(AIEra::StatisticalML.date_range(), (1994, 2005));
        assert_eq!(AIEra::DeepLearningDawn.date_range(), (2006, 2011));
        assert_eq!(AIEra::DeepLearningRevolution.date_range(), (2012, 2016));
        assert_eq!(AIEra::TransformerEra.date_range(), (2017, 2019));
        assert_eq!(AIEra::LLMEra.date_range(), (2020, 2025));
    }

    #[test]
    fn test_contribution_new_defaults() {
        let c = AIContribution::new("Test Paper", vec!["Alice", "Bob"], 2020, "NeurIPS");
        assert_eq!(c.title, "Test Paper");
        assert_eq!(c.authors, vec!["Alice", "Bob"]);
        assert_eq!(c.year, 2020);
        assert_eq!(c.venue, "NeurIPS");
        assert_eq!(c.era, AIEra::LLMEra);
        assert!(c.categories.is_empty());
        assert!(c.concepts.is_empty());
        assert!(c.summary.is_empty());
        assert!(c.impact.is_empty());
        assert!(c.equations.is_empty());
        assert!(c.tags.is_empty());
        assert!(c.builds_on.is_empty());
        assert!(c.enabled.is_empty());
    }

    #[test]
    fn test_contribution_builder_chain() {
        let c = AIContribution::new("Attention Is All You Need", vec!["Vaswani"], 2017, "NeurIPS")
            .with_categories(vec![ContributionCategory::Architecture, ContributionCategory::Language])
            .with_concepts(vec!["transformer", "self-attention"])
            .with_summary("Introduced the Transformer architecture")
            .with_impact("Revolutionised NLP")
            .with_equations(vec![r"softmax(QK^T / \sqrt{d_k})V"])
            .with_tags(vec!["transformer", "attention"]);

        assert_eq!(c.categories.len(), 2);
        assert_eq!(c.concepts, vec!["transformer", "self-attention"]);
        assert_eq!(c.summary, "Introduced the Transformer architecture");
        assert_eq!(c.impact, "Revolutionised NLP");
        assert_eq!(c.equations.len(), 1);
        assert_eq!(c.tags, vec!["transformer", "attention"]);
    }

    #[test]
    fn test_contribution_era_determination() {
        assert_eq!(AIContribution::new("A", vec!["X"], 1943, "J").era, AIEra::Foundations);
        assert_eq!(AIContribution::new("A", vec!["X"], 1960, "J").era, AIEra::BirthOfAI);
        assert_eq!(AIContribution::new("A", vec!["X"], 1975, "J").era, AIEra::KnowledgeSystems);
        assert_eq!(AIContribution::new("A", vec!["X"], 1985, "J").era, AIEra::ExpertSystems);
        assert_eq!(AIContribution::new("A", vec!["X"], 1990, "J").era, AIEra::AIWinter);
        assert_eq!(AIContribution::new("A", vec!["X"], 2000, "J").era, AIEra::StatisticalML);
        assert_eq!(AIContribution::new("A", vec!["X"], 2010, "J").era, AIEra::DeepLearningDawn);
        assert_eq!(AIContribution::new("A", vec!["X"], 2015, "J").era, AIEra::DeepLearningRevolution);
        assert_eq!(AIContribution::new("A", vec!["X"], 2018, "J").era, AIEra::TransformerEra);
        assert_eq!(AIContribution::new("A", vec!["X"], 2023, "J").era, AIEra::LLMEra);
    }

    #[test]
    fn test_add_and_get() {
        let mut kb = AIHistoryKB::new();
        let c = AIContribution::new("Paper A", vec!["Author"], 2020, "Venue");
        let id = c.id;
        kb.add(c);

        let retrieved = kb.get(&id).unwrap();
        assert_eq!(retrieved.title, "Paper A");
        assert_eq!(retrieved.id, id);
    }

    #[test]
    fn test_get_missing_returns_none() {
        let kb = AIHistoryKB::new();
        assert!(kb.get(&Uuid::new_v4()).is_none());
    }

    #[test]
    fn test_by_year() {
        let mut kb = AIHistoryKB::new();
        kb.add(AIContribution::new("P1", vec!["A"], 2020, "V"));
        kb.add(AIContribution::new("P2", vec!["B"], 2020, "V"));
        kb.add(AIContribution::new("P3", vec!["C"], 2021, "V"));

        assert_eq!(kb.by_year(2020).len(), 2);
        assert_eq!(kb.by_year(2021).len(), 1);
        assert_eq!(kb.by_year(1900).len(), 0);
    }

    #[test]
    fn test_by_category() {
        let mut kb = AIHistoryKB::new();
        kb.add(
            AIContribution::new("P1", vec!["A"], 2020, "V")
                .with_categories(vec![ContributionCategory::Theory]),
        );
        kb.add(
            AIContribution::new("P2", vec!["B"], 2020, "V")
                .with_categories(vec![ContributionCategory::Theory, ContributionCategory::Algorithm]),
        );

        assert_eq!(kb.by_category(ContributionCategory::Theory).len(), 2);
        assert_eq!(kb.by_category(ContributionCategory::Algorithm).len(), 1);
        assert_eq!(kb.by_category(ContributionCategory::Hardware).len(), 0);
    }

    #[test]
    fn test_lineage() {
        let mut kb = AIHistoryKB::new();
        let grandparent = AIContribution::new("G", vec!["A"], 1950, "V");
        let gp_id = grandparent.id;
        kb.add(grandparent);

        let mut parent = AIContribution::new("P", vec!["B"], 1960, "V");
        parent.builds_on.push(gp_id);
        let p_id = parent.id;
        kb.add(parent);

        let mut child = AIContribution::new("C", vec!["C"], 1970, "V");
        child.builds_on.push(p_id);
        let c_id = child.id;
        kb.add(child);

        let lineage = kb.lineage(&c_id);
        assert_eq!(lineage.len(), 3);
        assert_eq!(lineage[0].id, c_id);
        assert_eq!(lineage[1].id, p_id);
        assert_eq!(lineage[2].id, gp_id);
    }

    #[test]
    fn test_lineage_empty_for_root() {
        let mut kb = AIHistoryKB::new();
        let root = AIContribution::new("Root", vec!["A"], 1943, "V");
        let root_id = root.id;
        kb.add(root);

        let lineage = kb.lineage(&root_id);
        assert_eq!(lineage.len(), 1);
        assert_eq!(lineage[0].id, root_id);
    }

    #[test]
    fn test_empty_kb() {
        let kb = AIHistoryKB::new();
        assert!(kb.chronological().is_empty());
        assert!(kb.by_era(AIEra::Foundations).is_empty());
        assert!(kb.search_concept("anything").is_empty());
        assert!(kb.search_author("anyone").is_empty());
    }

    #[test]
    fn test_default_is_with_history() {
        let kb = AIHistoryKB::default();
        assert!(!kb.contributions.is_empty());
    }

    #[test]
    fn test_search_concept_case_insensitive() {
        let mut kb = AIHistoryKB::new();
        kb.add(
            AIContribution::new("P", vec!["A"], 2020, "V")
                .with_concepts(vec!["Backpropagation"]),
        );
        assert_eq!(kb.search_concept("backpropagation").len(), 1);
        assert_eq!(kb.search_concept("BACKPROPAGATION").len(), 1);
    }

    #[test]
    fn test_search_author_case_insensitive() {
        let mut kb = AIHistoryKB::new();
        kb.add(AIContribution::new("P", vec!["Geoffrey Hinton"], 2020, "V"));
        assert_eq!(kb.search_author("geoffrey hinton").len(), 1);
        assert_eq!(kb.search_author("GEOFFREY HINTON").len(), 1);
    }
}
