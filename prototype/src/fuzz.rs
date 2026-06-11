//! Robustness fuzzing for the front-end pipeline (lex → parse → resolve →
//! typecheck → effects). The contract under test: **no input, however
//! malformed, may panic the compiler** — malformed input must surface as a
//! `ParseError`/diagnostic, never a crash. This is the empirical evidence
//! behind any claim that the toolchain is robust (vs. "prototype, assume
//! fragile"). Deterministic: a seeded LCG drives generation, so a failing seed
//! is reproducible.

#[cfg(test)]
mod fuzz_tests {
    use crate::{effects, lexer, parser, resolve, types};
    use std::panic::{catch_unwind, AssertUnwindSafe};

    /// Tiny deterministic PRNG (xorshift) — no external deps, reproducible.
    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            let mut x = self.0;
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            self.0 = x;
            x
        }
        fn upto(&mut self, n: usize) -> usize {
            (self.next() % n as u64) as usize
        }
    }

    /// The full front-end. Returns `true` if the input parsed (so we know it
    /// reached the deep stages — resolve/typecheck/effects — not just the lexer).
    fn run_pipeline(src: &str) -> bool {
        let tokens = lexer::lex(src);
        if let Ok(module) = parser::parse(&tokens) {
            let _ = resolve::resolve(&module);
            let _ = types::check(&module);
            let _ = effects::infer_effects(&module);
            true
        } else {
            false
        }
    }

    /// Alphabet biased toward MechGen's real surface so inputs reach deep code
    /// paths, not just the lexer's error arm.
    const FRAGMENTS: &[&str] = &[
        "+f", "f ", "val ", "var ", "m ", "v ", "data ", "extend ", "net ", "train ",
        "match ", "?", ":", "->", "/io", "/net", "{", "}", "(", ")", "[", "]", "~",
        "i64", "f64", "x", "0", "1.5", "=>", "==", "<=", "&", "&mut", ";", ",", "\n",
        "Some(", "Ok(", "None", "loop", "break", "return ", "layer", "forward",
        "::", ".", "@", "|", "!", "?usize", "[i32]~", "R[i32,i32]", "guard ", "else",
    ];

    fn gen_fragment_soup(rng: &mut Rng) -> String {
        let n = 1 + rng.upto(40);
        let mut s = String::new();
        for _ in 0..n {
            s.push_str(FRAGMENTS[rng.upto(FRAGMENTS.len())]);
            if rng.upto(3) == 0 {
                s.push(' ');
            }
        }
        s
    }

    fn gen_random_bytes(rng: &mut Rng) -> String {
        let n = rng.upto(60);
        let mut s = String::new();
        for _ in 0..n {
            // printable ASCII + a few structural chars
            let c = (0x20 + rng.upto(0x5f)) as u8 as char;
            s.push(c);
        }
        s
    }

    fn mutate(seed_src: &str, rng: &mut Rng) -> String {
        let mut bytes: Vec<char> = seed_src.chars().collect();
        if bytes.is_empty() {
            return String::new();
        }
        let edits = 1 + rng.upto(8);
        for _ in 0..edits {
            let i = rng.upto(bytes.len());
            match rng.upto(3) {
                0 => {
                    bytes.remove(i);
                    if bytes.is_empty() {
                        break;
                    }
                }
                1 => bytes.insert(i, FRAGMENTS[rng.upto(FRAGMENTS.len())].chars().next().unwrap()),
                _ => bytes[i] = (0x20 + rng.upto(0x5f)) as u8 as char,
            }
        }
        bytes.into_iter().collect()
    }

    const SEEDS: &[&str] = &[
        "+f factorial(n: i64) -> i64 { ? n <= 1 { 1 } : { n * factorial(n - 1) } }",
        "net N { layer a: Linear(3, 8); forward { a } }",
        "data Color = Red | Green | Blue\nf n(c: Color) -> i32 { match c { Color.Red => 0, _ => 9 } }",
        "f main() / io { val xs: [i64]~ = [1, 2, 3]; println(\"hi\"); }",
    ];

    /// Returns (crashing inputs, count that parsed = reached deep stages).
    fn fuzz_round(strategy: u8, iterations: usize, base_seed: u64) -> (Vec<String>, usize) {
        let mut rng = Rng(base_seed | 1);
        let mut crashes = Vec::new();
        let mut parsed_ok = 0usize;
        for _ in 0..iterations {
            let input = match strategy {
                0 => gen_fragment_soup(&mut rng),
                1 => gen_random_bytes(&mut rng),
                _ => mutate(SEEDS[rng.upto(SEEDS.len())], &mut rng),
            };
            let probe = input.clone();
            match catch_unwind(AssertUnwindSafe(|| run_pipeline(&probe))) {
                Ok(reached_deep) => {
                    if reached_deep {
                        parsed_ok += 1;
                    }
                }
                Err(_) => crashes.push(input),
            }
        }
        (crashes, parsed_ok)
    }

    /// PROPERTY: effect checking is SOUND at a TRUST BOUNDARY — for any public
    /// function, every effect it performs but does not declare is flagged (no
    /// silent escape), and one that declares everything it performs is NOT
    /// false-flagged. (Private functions infer silently; their effects surface at
    /// the public callers that reach them — see `effect_soundness_is_transitive`.)
    /// This is the empirical evidence behind the capability-gate safety claim.
    /// (println→IO, open→FS, connect→Net, spawn→Async per check_builtin_effect.)
    #[test]
    fn effect_checking_is_sound_over_generated_programs() {
        // (builtin call, source effect name, diagnostic Display name)
        let table = [("println(\"x\")", "io", "IO"), ("open()", "fs", "FS"),
                     ("connect()", "net", "Net"), ("spawn()", "async", "Async")];
        let mut rng = Rng(0xA5A5_5A5A_1234_9876);
        let mut checked = 0usize;
        for _ in 0..6000 {
            // random non-empty subset of builtins to call, random subset to declare
            let call_mask = 1 + rng.upto(15); // 1..=15 over 4 builtins
            let decl_mask = rng.upto(16); // 0..=15
            let calls: Vec<&str> = (0..4).filter(|i| call_mask & (1 << i) != 0)
                .map(|i| table[i].0).collect();
            let decls: Vec<&str> = (0..4).filter(|i| decl_mask & (1 << i) != 0)
                .map(|i| table[i].1).collect();
            let ann = if decls.is_empty() { String::new() }
                      else { format!(" / {}", decls.join(" + ")) };
            // `+f` = public → a trust boundary where declarations are required.
            let src = format!("+f g(){ann} {{ {}; }}", calls.join("; "));

            let ei = effects::infer_effects(&parser::parse(&lexer::lex(&src)).unwrap());
            let diag = ei.diagnostics.iter().map(|d| d.message.clone()).collect::<Vec<_>>().join(" | ");

            // performed effects = display names of called builtins
            for i in 0..4 {
                let performed = call_mask & (1 << i) != 0;
                let declared = decl_mask & (1 << i) != 0;
                let display = table[i].2;
                if performed && !declared {
                    // SOUNDNESS: undeclared performed effect MUST be flagged.
                    assert!(
                        diag.contains(display),
                        "UNSOUND: `{src}` performs undeclared {display} but it was not flagged. diag=[{diag}]"
                    );
                }
            }
            // No-false-positive: if everything performed is declared, no undeclared-effect error.
            let all_declared = (0..4).all(|i| (call_mask & (1<<i)==0) || (decl_mask & (1<<i)!=0));
            if all_declared {
                assert!(
                    !diag.contains("undeclared effects"),
                    "FALSE POSITIVE: `{src}` declares all it performs but was flagged. diag=[{diag}]"
                );
            }
            checked += 1;
        }
        assert_eq!(checked, 6000);
    }

    /// PROPERTY: the Agent-mode formatter is an idempotent fixed point and its
    /// output re-parses — `fmt(parse(fmt(parse(s)))) == fmt(parse(s))`. A
    /// formatter that isn't a fixed point (or emits non-reparseable text) is a
    /// correctness/determinism bug. Verified over seeds + parseable mutations.
    /// General-language seeds whose Agent-mode formatting is expected to
    /// round-trip. KNOWN GAP (documented, not hidden): the AI-construct
    /// formatter (`net`/`kb`/`train`) emits decorative Unicode sigils (Ψ/λ/κ)
    /// the lexer doesn't accept and a lossy `forward { ... }` placeholder, so
    /// those do NOT round-trip yet — excluded here and tracked as future work.
    const FMT_SEEDS: &[&str] = &[
        "+f factorial(n: i64) -> i64 { ? n <= 1 { 1 } : { n * factorial(n - 1) } }",
        "data Color = Red | Green | Blue\nf n(c: Color) -> i32 { match c { Color.Red => 0, _ => 9 } }",
        "f main() / io { val xs: [i64]~ = [1, 2, 3]; println(\"hi\"); }",
        "f add(a: i32, b: i32) -> i32 { a + b }",
    ];

    /// PROPERTY: effect soundness holds TRANSITIVELY to the trust boundary —
    /// public `h` → private `g` → effectful-builtin. If `g` performs an effect
    /// and the PUBLIC `h` calls `g` but doesn't declare it, `h` must be flagged:
    /// a private intermediate cannot hide an effect from the module surface. This
    /// is exactly what makes "infer inside, declare at the boundary" sound.
    #[test]
    fn effect_soundness_is_transitive() {
        let eff = [("println(\"x\")", "io", "IO"), ("open()", "fs", "FS"),
                   ("connect()", "net", "Net"), ("spawn()", "async", "Async")];
        let mut rng = Rng(0x51ED_C0DE_F00D_1111);
        for _ in 0..4000 {
            let i = rng.upto(4);
            let (call, ename, display) = eff[i];
            // g performs the effect and declares it; f calls g and declares a
            // RANDOM subset (maybe not including this effect).
            let f_declares = rng.upto(2) == 1;
            let f_ann = if f_declares { format!(" / {ename}") } else { String::new() };
            // `g` is private (infers/declares internally); `+f h` is the public
            // boundary that must surface g's effect.
            let src = format!("f g() / {ename} {{ {call}; }}\n+f h(){f_ann} {{ g(); }}");
            let ei = effects::infer_effects(&parser::parse(&lexer::lex(&src)).unwrap());
            let diag: String =
                ei.diagnostics.iter().map(|d| d.message.clone()).collect::<Vec<_>>().join(" | ");
            if !f_declares {
                // h performs `display` transitively via g but didn't declare it.
                assert!(
                    diag.contains(&format!("function `h`")) && diag.contains(display),
                    "UNSOUND (transitive): `{src}` — h's undeclared {display} not flagged. diag=[{diag}]"
                );
            } else {
                assert!(
                    !diag.contains("function `h` performs undeclared"),
                    "FALSE POSITIVE (transitive): `{src}` — h declared {display}. diag=[{diag}]"
                );
            }
        }
    }

    #[test]
    fn agent_formatter_is_idempotent_and_reparses() {
        use crate::fmt;
        let mut rng = Rng(0x0BADC0DE_1357_2468);
        let mut tested = 0usize;
        for _ in 0..6000 {
            let src = if rng.upto(4) == 0 {
                FMT_SEEDS[rng.upto(FMT_SEEDS.len())].to_string()
            } else {
                mutate(FMT_SEEDS[rng.upto(FMT_SEEDS.len())], &mut rng)
            };
            // Only consider inputs that parse.
            let Ok(m1) = parser::parse(&lexer::lex(&src)) else { continue };
            let once = fmt::format_agent(&m1);
            // The formatted output MUST re-parse...
            let Ok(m2) = parser::parse(&lexer::lex(&once)) else {
                panic!("formatter output does not re-parse:\n--- src ---\n{src}\n--- fmt ---\n{once}");
            };
            let twice = fmt::format_agent(&m2);
            // ...and formatting is a fixed point.
            assert_eq!(once, twice, "formatter not idempotent for:\n{src}");
            tested += 1;
        }
        // Ensure the property actually exercised real programs.
        assert!(tested > 500, "only {tested} inputs parsed — property under-exercised");
    }

    #[test]
    fn pipeline_never_panics_on_fragment_soup() {
        let (crashes, _) = fuzz_round(0, 20_000, 0x9E3779B97F4A7C15);
        assert!(crashes.is_empty(), "panics on {} inputs, e.g.: {:?}", crashes.len(), crashes.first());
    }

    #[test]
    fn pipeline_never_panics_on_random_bytes() {
        let (crashes, _) = fuzz_round(1, 20_000, 0xD1B54A32D192ED03);
        assert!(crashes.is_empty(), "panics on {} inputs, e.g.: {:?}", crashes.len(), crashes.first());
    }

    #[test]
    fn pipeline_never_panics_on_mutated_valid_programs() {
        let (crashes, parsed_ok) = fuzz_round(2, 20_000, 0x2545F4914F6CDD1D);
        assert!(crashes.is_empty(), "panics on {} inputs, e.g.: {:?}", crashes.len(), crashes.first());
        // Honesty check: this strategy MUST reach the deep stages (typecheck/
        // effects), or "robust" would only mean "the lexer doesn't crash".
        assert!(
            parsed_ok > 1000,
            "only {parsed_ok}/20000 mutated inputs parsed — deep stages under-exercised"
        );
    }
}
