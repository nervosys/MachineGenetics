//! Agent-facing self-description of the `forge` toolchain.
//!
//! Same progressive-disclosure pattern the MechGen-parse CLI, RecursiveMachine-
//! Intelligence, and agentic-eval ship: a token-compact root index
//! (`forge manifest`) plus on-demand expansion (`forge describe <command>`),
//! and a fully machine-readable form (`forge manifest --json`). An agent driving
//! Forge never needs its prose docs.
//!
//! Every command carries its **effect class** (agentic-eval taxonomy:
//! `pure` / `read_local` / `write_local` / `network` / `exec`) so an agent
//! policy can gate invocations without trial-running them, plus its argument
//! shape. Deterministic: fixed entry order, no map iteration.

/// One `forge` sub-command.
#[derive(Debug, Clone, Copy)]
pub struct ForgeCommand {
    /// The sub-command name (e.g. `check`).
    pub name: &'static str,
    /// Argument shape (`""` if none).
    pub args: &'static str,
    /// One-line summary.
    pub summary: &'static str,
    /// Effect class under the agentic-eval taxonomy.
    pub effect: &'static str,
    /// Expanded detail: behavior, defaults, output.
    pub detail: &'static str,
}

/// Every command, in a fixed (deterministic) order.
pub const COMMANDS: &[ForgeCommand] = &[
    ForgeCommand {
        name: "manifest",
        args: "[--json]",
        summary: "print this token-compact command index (read this first)",
        effect: "pure",
        detail: "Root discovery for agents: every command with its effect class and arg shape.\n\
                 With --json: a machine-readable object {tool, version, commands:[…]}. Deterministic.",
    },
    ForgeCommand {
        name: "describe",
        args: "<command>",
        summary: "expand one manifest entry (e.g. describe check)",
        effect: "pure",
        detail: "Full detail for one command: arg shape, behavior, output, effect class.\n\
                 Unknown names list the valid ones.",
    },
    ForgeCommand {
        name: "new",
        args: "<name>",
        summary: "scaffold a project (Forge.toml + src/main.mg) that checks and runs",
        effect: "write_local",
        detail: "Creates <name>/Forge.toml and <name>/src/main.mg (a verified-working\n\
                 program). The only command that writes outside an explicit target.",
    },
    ForgeCommand {
        name: "check",
        args: "[--json]",
        summary: "parse + typecheck the entry point via MechGen-parse",
        effect: "read_local",
        detail: "Discovers Forge.toml (walking up), runs the compiler front-end on the\n\
                 entry. Exit 0 = clean; the compiler diagnostic is surfaced on failure.\n\
                 With --json: {command, project, version, entry, ok, error?}.",
    },
    ForgeCommand {
        name: "build",
        args: "[--json]",
        summary: "check, then lower through the Agentic Binary Language IR",
        effect: "read_local",
        detail: "check + the binary-IR lowering summary. No native text-language backend\n\
                 yet, so a clean check is the gate. With --json: {command, project, ok, error?}.",
    },
    ForgeCommand {
        name: "run",
        args: "[fn] [--json]",
        summary: "execute the entry function (default: the manifest's `main`)",
        effect: "read_local",
        detail: "Evaluates `fn` via MechGen-parse --eval and prints the result. The\n\
                 evaluator is a pure tree-walker (no I/O). With --json:\n\
                 {command, project, fn, ok, result?, error?}.",
    },
    ForgeCommand {
        name: "info",
        args: "[--json]",
        summary: "print the resolved manifest (name, version, entry, …)",
        effect: "read_local",
        detail: "Reads Forge.toml and prints the resolved project fields. With --json:\n\
                 the manifest as an object.",
    },
];

/// Token-compact root index. Deterministic.
pub fn manifest() -> String {
    let mut s = String::with_capacity(1024);
    s.push_str("forge v");
    s.push_str(env!("CARGO_PKG_VERSION"));
    s.push_str(" — MechGen project toolchain, NERVOSYS. drives the MechGen-parse compiler. ");
    s.push_str("commands (name [args] — summary {effect}):\n");
    for c in COMMANDS {
        s.push_str("  ");
        s.push_str(c.name);
        if !c.args.is_empty() {
            s.push(' ');
            s.push_str(c.args);
        }
        s.push_str(" — ");
        s.push_str(c.summary);
        s.push_str(" {");
        s.push_str(c.effect);
        s.push_str("}\n");
    }
    s.push_str("expand: forge describe <command>.\n");
    s
}

/// Machine-readable manifest (deterministic JSON, hand-built — no serde needed
/// since these are static `&str`s with no special characters to escape).
pub fn manifest_json() -> String {
    let mut s = String::with_capacity(2048);
    s.push_str("{\n  \"tool\": \"forge\",\n  \"version\": \"");
    s.push_str(env!("CARGO_PKG_VERSION"));
    s.push_str("\",\n  \"commands\": [\n");
    for (i, c) in COMMANDS.iter().enumerate() {
        s.push_str("    {\"name\": \"");
        s.push_str(c.name);
        s.push_str("\", \"args\": \"");
        s.push_str(c.args);
        s.push_str("\", \"effect\": \"");
        s.push_str(c.effect);
        s.push_str("\", \"summary\": \"");
        s.push_str(c.summary);
        s.push('"');
        s.push('}');
        if i + 1 < COMMANDS.len() {
            s.push(',');
        }
        s.push('\n');
    }
    s.push_str("  ]\n}\n");
    s
}

/// Expand one command by name.
pub fn describe(name: &str) -> Option<String> {
    let q = name.trim().trim_start_matches('-');
    COMMANDS.iter().find(|c| c.name == q).map(|c| {
        format!("{} {} — {} {{{}}}\n{}", c.name, c.args, c.summary, c.effect, c.detail)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_lists_every_command_with_effect() {
        let m = manifest();
        for c in COMMANDS {
            assert!(m.contains(c.name), "manifest missing {}", c.name);
            assert!(m.contains(&format!("{{{}}}", c.effect)), "missing effect for {}", c.name);
        }
    }

    #[test]
    fn every_effect_is_in_the_taxonomy() {
        for c in COMMANDS {
            assert!(
                matches!(c.effect, "pure" | "read_local" | "write_local" | "network" | "exec"),
                "{} has out-of-taxonomy effect {}",
                c.name,
                c.effect
            );
        }
    }

    #[test]
    fn describe_resolves_with_and_without_dashes() {
        assert!(describe("check").is_some());
        assert!(describe("--check").is_some());
        assert!(describe("nope").is_none());
    }

    #[test]
    fn json_manifest_is_deterministic_and_complete() {
        let a = manifest_json();
        let b = manifest_json();
        assert_eq!(a, b);
        assert!(a.starts_with("{\n  \"tool\": \"forge\""));
        for c in COMMANDS {
            assert!(a.contains(&format!("\"name\": \"{}\"", c.name)));
        }
    }
}
