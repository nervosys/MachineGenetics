// framewerx::neurosymbolic — bridge between net (neural) and kb (symbolic).
//
// This is the reliability-via-ontology piece. A pure neural model can
// hallucinate; a pure symbolic model can't generalise. The hybrid shape
// composes both:
//
//   1. Neural branch produces a candidate output (e.g. a class index).
//   2. Symbolic branch checks the candidate against declared rules.
//   3. If the check fails, the caller falls back or requests refinement.
//
// Both branches lower to Agentic Binary Language: the neural side via existing
// op-family dispatch, the symbolic side via the SKB->RMI ontology adapter
// (`rmi_ontology_adapter.rs`).
//
// ── What this file used to say, and why it changed ───────────────────
//
// It declared `S Hybrid { neural: @Module, knowledge: @KnowledgeBase }` and
// called `self.neural.forward(x)`. Neither name resolves: `Module` lives in
// `module.mg` and there is no module system, `KnowledgeBase` is defined
// nowhere in the tree — and more fundamentally, **a `net` and a `kb` are
// declarations, not values**. You cannot store one in a struct field.
//
// The composition is therefore a *function*, and the symbolic side is a
// declared effect — which is what makes the validation mockable and what puts
// it in the caller's signature.

// ── The neural branch ────────────────────────────────────────────────

net Classifier {
    layer fc: Linear(8, 4)
    layer head: Linear(4, 3)
    forward { head(fc) }
}

// ── The symbolic branch ──────────────────────────────────────────────
//
// Declared facts and a rule over them. `kb` is a capability namespace that
// deliberately attributes no effect, so reaching the store you *want* gated
// goes through an `effect` block instead.

kb DomainRules {
    fact valid_class(0);
    fact valid_class(1);
    fact valid_class(2);
    rule prediction_ok(c: i32) { valid_class(c) }
}

effect Rules {
    f check(class: i32) -> bool;
    f explain(class: i32) -> s;
}

// ── The contract ─────────────────────────────────────────────────────
//
// Every hybrid prediction carries its symbolic-validation result, so the
// caller can branch on `verified` rather than trusting the number.

+S HybridOutput {
    value: i32,
    verified: bool,
    rationale: s,
}

// ── The composition ──────────────────────────────────────────────────
//
// `/ rules` is the whole point: a caller of `predict` can see that the answer
// passed through symbolic validation, and a test can substitute the validator
// with `handle … with Rules { … }` and never touch a knowledge base.

+f predict(candidate: i32) -> HybridOutput / rules {
    v verified = Rules.check(candidate)
    @HybridOutput {
        value: candidate,
        verified: verified,
        rationale: Rules.explain(candidate),
    }
}

// Fall back when the symbolic side rejects the neural answer.
+f predict_or(candidate: i32, fallback: i32) -> i32 / rules {
    v out = predict(candidate)
    ? out.verified { out.value } : { fallback }
}

// A test needs no model and no store: the handler is the validator.
@test
+f test_rejects_out_of_range() -> i32 {
    handle {
        predict_or(7, 0)
    } with Rules {
        check(class) => class < 3,
        explain(class) => "out of range",
    }
}
