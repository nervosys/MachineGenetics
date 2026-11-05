// Every error code the builder can produce, with the family of the schema that
// serves it, when it applies, and the fix the schema gives. prototype/src/builder.rs
// refuses a JSON spec before any artifact exists, so these codes have no span and
// no severity, and they are not `Diagnostic`s (kb/rules/diagnostics.mg holds those).
//
// `build_rule_row` is id, family, when, fix. `--build=schema` serves each family's
// `errors` table from these rows, in code order, so the rows are the served text.
// `enforced_by` names the builder, which emits every code here.
//
// There is no K0005: the knowledge-base codes are K0001-K0004 and K0006-K0008.
//
// prototype/src/builder.rs holds these codes to this file both ways, and a test
// triggers each one by a witness spec.

kb Builder {
    fact instance_of("builder", SymbolicComponent);

    // ── net: the net spec, at the root of the schema ──

    fact instance_of("B0000", BuildRule);
    fact build_rule_row("B0000", "net", "spec JSON is malformed", "emit valid JSON matching spec_format");
    fact enforced_by("B0000", "builder");

    fact instance_of("B0001", BuildRule);
    fact build_rule_row("B0001", "net", "net name is empty", "set a non-empty \"net\" field");
    fact enforced_by("B0001", "builder");

    fact instance_of("B0002", BuildRule);
    fact build_rule_row("B0002", "net", "net has no layers", "add at least one layer");
    fact enforced_by("B0002", "builder");

    fact instance_of("B0003", BuildRule);
    fact build_rule_row("B0003", "net", "unknown op", "use an op listed in `ops`");
    fact enforced_by("B0003", "builder");

    fact instance_of("B0004", BuildRule);
    fact build_rule_row("B0004", "net", "wrong dim count for the op", "match the op's `dims` arity");
    fact enforced_by("B0004", "builder");

    fact instance_of("B0005", BuildRule);
    fact build_rule_row("B0005", "net", "a dim is not a positive integer", "use positive integers for dims");
    fact enforced_by("B0005", "builder");

    fact instance_of("B0006", BuildRule);
    fact build_rule_row("B0006", "net", "shape mismatch in the layer chain", "make each Linear's input dim equal the previous output dim");
    fact enforced_by("B0006", "builder");

    // ── kb: the knowledge-base spec ──

    fact instance_of("K0001", BuildRule);
    fact build_rule_row("K0001", "kb", "kb name is empty", "set a non-empty \"kb\" field");
    fact enforced_by("K0001", "builder");

    fact instance_of("K0002", BuildRule);
    fact build_rule_row("K0002", "kb", "kb has no facts and no rules", "add at least one fact or rule");
    fact enforced_by("K0002", "builder");

    fact instance_of("K0003", BuildRule);
    fact build_rule_row("K0003", "kb", "an identifier is invalid", "use [A-Za-z_][A-Za-z0-9_]* for predicates, rules, params, terms");
    fact enforced_by("K0003", "builder");

    fact instance_of("K0004", BuildRule);
    fact build_rule_row("K0004", "kb", "a predicate is used at two arities", "use a consistent arity for each predicate");
    fact enforced_by("K0004", "builder");

    fact instance_of("K0006", BuildRule);
    fact build_rule_row("K0006", "kb", "a rule body references an unknown predicate", "reference a declared fact-predicate or rule");
    fact enforced_by("K0006", "builder");

    fact instance_of("K0007", BuildRule);
    fact build_rule_row("K0007", "kb", "a head param is not bound by the body", "use each head param as an argument in some body literal");
    fact enforced_by("K0007", "builder");

    fact instance_of("K0008", BuildRule);
    fact build_rule_row("K0008", "kb", "the logic engine refuses the kb (E0570)", "give every rule at least one body literal; the message says what else");
    fact enforced_by("K0008", "builder");

    // ── agent: the agent spec ──

    fact instance_of("A0001", BuildRule);
    fact build_rule_row("A0001", "agent", "agent name is not a valid identifier", "use [A-Za-z_][A-Za-z0-9_]*");
    fact enforced_by("A0001", "builder");

    fact instance_of("A0002", BuildRule);
    fact build_rule_row("A0002", "agent", "a capability is not an identifier", "use identifier capability names");
    fact enforced_by("A0002", "builder");

    fact instance_of("A0003", BuildRule);
    fact build_rule_row("A0003", "agent", "a requires_approval is not an identifier", "use identifier operation names");
    fact enforced_by("A0003", "builder");

    // ── swarm: the swarm spec ──

    fact instance_of("S0001", BuildRule);
    fact build_rule_row("S0001", "swarm", "swarm name is not a valid identifier", "use [A-Za-z_][A-Za-z0-9_]*");
    fact enforced_by("S0001", "builder");

    fact instance_of("S0002", BuildRule);
    fact build_rule_row("S0002", "swarm", "agent type is not a valid identifier", "use [A-Za-z_][A-Za-z0-9_]*");
    fact enforced_by("S0002", "builder");

    fact instance_of("S0003", BuildRule);
    fact build_rule_row("S0003", "swarm", "size is not positive", "use a positive integer size");
    fact enforced_by("S0003", "builder");

    fact instance_of("S0004", BuildRule);
    fact build_rule_row("S0004", "swarm", "unknown topology", "use star, ring, mesh, broadcast, or tree");
    fact enforced_by("S0004", "builder");

    fact instance_of("S0005", BuildRule);
    fact build_rule_row("S0005", "swarm", "unknown consensus", "use majority, unanimous, weighted, or quorum");
    fact enforced_by("S0005", "builder");

    fact instance_of("S0006", BuildRule);
    fact build_rule_row("S0006", "swarm", "transport is not an encodable rmi_* identifier", "use rmi_quic, rmi_tcp, or rmi_grpc");
    fact enforced_by("S0006", "builder");

    // ── unified: the unified spec, which holds net and kb items ──

    fact instance_of("U0001", BuildRule);
    fact build_rule_row("U0001", "unified", "no items", "add at least one net or kb item");
    fact enforced_by("U0001", "builder");

    fact instance_of("U0002", BuildRule);
    fact build_rule_row("U0002", "unified", "an item is neither a net nor a kb", "each item must carry a \"net\" or \"kb\" key");
    fact enforced_by("U0002", "builder");

    fact instance_of("U0003", BuildRule);
    fact build_rule_row("U0003", "unified", "two items share a name", "give each item a unique name");
    fact enforced_by("U0003", "builder");

    // ── the tables of builder.rs: its ops, swarm topologies and consensus strategies ──
    // `every_op_is_stated`, `every_topology_and_consensus_is_stated` hold each table
    // to these facts, in both directions. Arity and transforms are text, as written.

    fact build_op("Linear", "2", "true", "fully-connected (in, out) -> out");
    fact build_op("Embedding", "2", "true", "lookup table (vocab, dim) -> dim");
    fact build_op("Conv2d", "2", "true", "2D convolution (in_ch, out_ch) -> out_ch");
    fact build_op("LayerNorm", "1", "false", "layer normalization (dim)");
    fact build_op("ReLU", "0", "false", "rectified linear activation");
    fact build_op("GELU", "0", "false", "Gaussian error linear activation");
    fact build_op("Tanh", "0", "false", "hyperbolic tangent activation");
    fact build_op("Sigmoid", "0", "false", "logistic activation");
    fact build_op("Softmax", "0", "false", "softmax over the feature dim");
    fact build_op("Dropout", "0", "false", "stochastic regularization (identity at inference)");

    fact build_topology("star");
    fact build_topology("ring");
    fact build_topology("mesh");
    fact build_topology("broadcast");
    fact build_topology("tree");

    fact build_consensus("majority");
    fact build_consensus("unanimous");
    fact build_consensus("weighted");
    fact build_consensus("quorum");
}
