//! Agent-facing manifest of the `MechGen-parse` CLI itself.
//!
//! An agent driving this compiler should never need its prose docs: the same
//! progressive-disclosure pattern RecursiveMachineIntelligence and agentic-eval ship — a
//! token-compact root index (`--manifest`) plus on-demand expansion
//! (`--describe <mode>`) — applied to the compiler's command surface.
//!
//! Every mode entry carries its **effect class** (agentic-eval taxonomy:
//! `pure` / `read_local` / `write_local` / `network` / `exec`) so an agent
//! policy can gate invocations without trial-running them, plus its argument
//! shape and output channel. Deterministic: fixed entry order, no map
//! iteration.

/// One CLI mode (a `--flag`-selected behavior of the binary).
#[derive(Debug, Clone, Copy)]
pub struct CliMode {
    /// The flag that selects the mode (e.g. `--check`).
    pub flag: &'static str,
    /// Argument shape after the flag (`[]` if none).
    pub args: &'static str,
    /// One-line summary.
    pub summary: &'static str,
    /// Effect class under the agentic-eval taxonomy.
    pub effect: &'static str,
    /// Expanded detail: behavior, defaults, output format.
    pub detail: &'static str,
}

/// Every CLI mode, in the order `main` matches them (deterministic).
pub const MODES: &[CliMode] = &[
    CliMode {
        flag: "--manifest",
        args: "",
        summary: "print this token-compact capability index (read this first)",
        effect: "pure",
        detail: "Root discovery for agents: lists every mode with its effect class.\n\
                 Expand any entry with --describe <flag-without-dashes>. Deterministic output.",
    },
    CliMode {
        flag: "--describe",
        args: "<mode>",
        summary: "expand one manifest entry (e.g. --describe check)",
        effect: "pure",
        detail: "Prints the full detail for one mode: argument shape, behavior, defaults,\n\
                 output format, effect class. Unknown names list the valid ones.",
    },
    CliMode {
        flag: "--check",
        args: "<file.mg> [--json]",
        summary: "parse + typecheck + effect-check; structured diagnostics; no output files",
        effect: "read_local",
        detail: "Full front-end pass (lex, parse, resolve, typecheck, effects) without\n\
                 lowering. Exit 0 = clean. Diagnostics go to stderr with spans.\n\
                 With --json: emit a deterministic, machine-readable diagnostic stream\n\
                 on stdout — {code, severity, line, col, category, message, fix} per\n\
                 diagnostic, sorted (byte-stable). Parse structurally; don't scrape prose.\n\
                 The cheapest way for an agent to validate generated MechGen.",
    },
    CliMode {
        flag: "--fmt-compact",
        args: "<file.mg> [out]",
        summary: "reformat to Agent mode (token-compact symbols)",
        effect: "write_local",
        detail: "Deterministic formatter to the Agent-mode surface (symbol keywords,\n\
                 elision). Writes to [out] or stdout. Byte-stable: fmt(fmt(x)) == fmt(x).",
    },
    CliMode {
        flag: "--fmt-expand",
        args: "<file.mg> [out]",
        summary: "reformat to Human mode (Rust-style keywords)",
        effect: "write_local",
        detail: "Inverse of --fmt-compact; same determinism guarantee.",
    },
    CliMode {
        flag: "--target=abl-bytes",
        args: "<file.mg> [out.abl]",
        summary: "lower Agentic Binary Language-routed items to a framed binary Agentic Binary Language container",
        effect: "write_local",
        detail: "Container: magic \"Agentic Binary Language\" + u16 version + u32 count + per-item\n\
                 (name_len, name, expr_len, expr). Without [out]: stdout summary only\n\
                 (read_local). Byte-stable for caching/diffing.",
    },
    CliMode {
        flag: "--from=abl-bytes",
        args: "<file.abl>",
        summary: "decode an Agentic Binary Language container back to a summary",
        effect: "read_local",
        detail: "Round-trip check for the binary path; prints item names and sizes.",
    },
    CliMode {
        flag: "--run=abl-bytes",
        args: "<file.abl> [--backend=<name>]",
        summary: "dispatch a compiled Agentic Binary Language container through a compute backend",
        effect: "read_local",
        detail: "Executes each item via run_pipeline on the selected Backend\n\
                 (cpu default; cuda with --features cuda + driver; subprocess backends\n\
                 via --backends-file are EXEC effect — gate accordingly).\n\
                 Prints per-item output shape + checksum; `// gpu_ops:` line on CUDA.",
    },
    CliMode {
        flag: "--target=abl-compute",
        args: "<file.mg>",
        summary: "lower nets and run a forward pass on the compute backend",
        effect: "read_local",
        detail: "End-to-end: parse → bridge → Agentic Binary Language → run_pipeline. Reports dispatched\n\
                 op count, unsupported ops, output checksum.",
    },
    CliMode {
        flag: "--target=abl-train",
        args: "<file.mg>",
        summary: "find train blocks and run SGD epochs (synthetic data defaults)",
        effect: "write_local",
        detail: "Defaults: 50 epochs, lr 0.05, y=sum(x) synthetic dataset. Writes\n\
                 checkpoint (.ckpt) when the train block names one. Prints per-step loss.",
    },
    CliMode {
        flag: "--target=abl-infer",
        args: "<file.mg>",
        summary: "load a checkpoint and run inference",
        effect: "read_local",
        detail: "Pairs with --target=abl-train; reads the .ckpt the source names.",
    },
    CliMode {
        flag: "--target=abl-generate",
        args: "<file.mg>",
        summary: "autoregressive generation from a trained LM checkpoint",
        effect: "read_local",
        detail: "Greedy decode using the checkpointed tiny-LM weights.",
    },
    CliMode {
        flag: "--target=abl-run",
        args: "<file.mg>",
        summary: "full pipeline: lower, train if needed, infer",
        effect: "write_local",
        detail: "Convenience composition of compute/train/infer.",
    },
    CliMode {
        flag: "--target=abl",
        args: "<file.mg>",
        summary: "print the Agentic Binary Language lowering of each routed item (text form)",
        effect: "read_local",
        detail: "Human/agent-readable Agentic Binary Language expressions; no binary output.",
    },
    CliMode {
        flag: "--pipeline",
        args: "<file.mg>",
        summary: "run the full self-healing compile pipeline with recovery report",
        effect: "read_local",
        detail: "Lex → heal → parse → elide → typecheck with ranked fix candidates\n\
                 on failure (structured recovery — the reliability-bench surface).",
    },
    CliMode {
        flag: "--emit-ontology",
        args: "[path]",
        summary: "dump the complete system ontology as JSON",
        effect: "write_local",
        detail: "Default path MECHGEN_ONTOLOGY.json. The deep machine-readable map of\n\
                 the language + compiler + Agentic Binary Language; --manifest is the cheap index.",
    },
    CliMode {
        flag: "--rap",
        args: "[addr]",
        summary: "serve the JSON-RPC agent protocol over TCP (default 127.0.0.1:9876)",
        effect: "network",
        detail: "Long-running server exposing parse/check/compute/NL endpoints to agents.\n\
                 NETWORK effect: binds a socket. UNAUTHENTICATED + UNENCRYPTED — defaults to\n\
                 loopback; refuses non-loopback binds unless MECHGEN_RAP_ALLOW_REMOTE=1.\n\
                 See SECURITY_AUDIT.md (MITRE ATT&CK T1190/T1071).",
    },
    CliMode {
        flag: "(default)",
        args: "[file.mg] (or stdin)",
        summary: "parse and report (token stats with --token-report)",
        effect: "read_local",
        detail: "Modifiers: --no-elision, --syntax=legacy, --token-report,\n\
                 --backend=<name>, --backends-file=<path>.",
    },
];

/// Token-compact root index of the CLI. Deterministic.
pub fn manifest() -> String {
    let mut s = String::with_capacity(2048);
    s.push_str("MechGen-parse v");
    s.push_str(env!("CARGO_PKG_VERSION"));
    s.push_str(" — MachineGenetics (MechGen) agentic-first compiler CLI, NERVOSYS. ");
    s.push_str("built-in framework: RecursiveMachineIntelligence (rmi). modes (flag [args] — summary {effect}):\n");
    for m in MODES {
        s.push_str("  ");
        s.push_str(m.flag);
        if !m.args.is_empty() {
            s.push(' ');
            s.push_str(m.args);
        }
        s.push_str(" — ");
        s.push_str(m.summary);
        s.push_str(" {");
        s.push_str(m.effect);
        s.push_str("}\n");
    }
    s.push_str("expand: --describe <mode> (flag name without leading dashes).\n");
    s
}

/// Expand one mode by name (with or without leading dashes / `=value` part).
pub fn describe(name: &str) -> Option<String> {
    let q = name.trim().trim_start_matches('-');
    MODES
        .iter()
        .find(|m| {
            let f = m.flag.trim_start_matches('-');
            f == q || f.split('=').next() == Some(q) || f.split('=').nth(1) == Some(q)
        })
        .map(|m| {
            format!(
                "{} {} — {} {{{}}}\n{}",
                m.flag, m.args, m.summary, m.effect, m.detail
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_is_compact_and_deterministic() {
        let a = manifest();
        let b = manifest();
        assert_eq!(a, b);
        assert!(a.len() < 4096, "CLI manifest too large: {} bytes", a.len());
        // Every mode line carries an effect class.
        for m in MODES {
            assert!(a.contains(m.flag));
            assert!(
                matches!(m.effect, "pure" | "read_local" | "write_local" | "network" | "exec"),
                "{} has unknown effect {}",
                m.flag,
                m.effect
            );
        }
    }

    #[test]
    fn describe_resolves_flags_with_and_without_dashes() {
        assert!(describe("--check").is_some());
        assert!(describe("check").is_some());
        assert!(describe("abl-bytes").is_some(), "matches --target=abl-bytes by value");
        assert!(describe("nonsense-mode").is_none());
        let rap = describe("rap").unwrap();
        assert!(rap.contains("{network}"), "rap is effect-classified: {rap}");
    }
}
