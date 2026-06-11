//! In-process performance measurement (wall-clock medians, no process-startup
//! noise). Run with:
//!   cargo test perf_report -- --nocapture
//!
//! Measures the agent-facing hot paths: parse throughput, ABL build latency +
//! artifact-size scaling, kb Datalog evaluation, and no-exec decode/describe.

#[cfg(test)]
mod measure {
    use crate::{abl, abl_bridge, builder, lexer, parser};
    use std::time::Instant;

    /// Shape-consistent net spec JSON with `n` Linear(16,16) layers.
    fn net_spec_json(n: usize) -> String {
        let layers: Vec<String> = (0..n)
            .map(|i| format!(r#"["fc{i}","Linear",[16,16]]"#))
            .collect();
        format!(r#"{{"net":"N","layers":[{}]}}"#, layers.join(","))
    }

    fn net_spec(n: usize) -> builder::NetSpec {
        serde_json::from_str(&net_spec_json(n)).unwrap()
    }

    // `#[ignore]` so the normal `cargo test` stays fast (the recursive-closure
    // case takes seconds). Run explicitly:
    //   cargo test --release perf_report -- --ignored --nocapture
    #[test]
    #[ignore]
    fn perf_report() {
        println!("\n=== MechGen performance (in-process, wall-clock medians) ===");

        // 1. Parse throughput — lex + parse a realistic 50-layer net.
        let src = builder::to_mg_source(&net_spec(50));
        let bytes = src.len();
        let toks = lexer::lex(&src).len();
        let iters = 3000;
        let t = Instant::now();
        for _ in 0..iters {
            let tk = lexer::lex(&src);
            let _ = parser::parse(&tk);
        }
        let per_s = t.elapsed().as_secs_f64() / iters as f64;
        println!(
            "[parse]  {bytes}B / {toks} tokens → {:.1}µs/parse  ({:.1} MB/s, {:.2}M tok/s)",
            per_s * 1e6,
            (bytes as f64 / per_s) / 1e6,
            (toks as f64 / per_s) / 1e6,
        );

        // 2. ABL build latency + artifact-size scaling (spec → source → IR bytes).
        println!("[build]  net layers → build latency / artifact bytes:");
        for &n in &[2usize, 8, 32, 128] {
            let spec = net_spec(n);
            let iters = 1500;
            let mut blen = 0usize;
            let t = Instant::now();
            for _ in 0..iters {
                let s = builder::to_mg_source(&spec);
                let m = parser::parse(&lexer::lex(&s)).unwrap();
                let (blob, _) = abl::encode_module(&m);
                blen = blob.len();
            }
            let per_us = t.elapsed().as_secs_f64() / iters as f64 * 1e6;
            println!("           {n:>4} layers: {per_us:>7.1}µs   {blen:>5}B  ({:.1} B/layer)", blen as f64 / n.max(1) as f64);
        }

        // 3. kb Datalog evaluation.
        //    (a) one-pass 2-hop join over a chain of N edges (linear scaling).
        println!("[kb-join] 2-hop join over N edges → derived / eval time:");
        let two_hop = vec![abl_bridge::KbRule {
            head: "hop2".into(),
            params: vec!["x".into(), "z".into()],
            body: vec![
                ("edge".into(), vec!["x".into(), "y".into()]),
                ("edge".into(), vec!["y".into(), "z".into()]),
            ],
        }];
        for &n in &[100usize, 500, 1000, 2000] {
            let facts: Vec<abl_bridge::GroundFact> = (0..n)
                .map(|i| ("edge".to_string(), vec![i.to_string(), (i + 1).to_string()]))
                .collect();
            let t = Instant::now();
            let derived = abl_bridge::evaluate_kb(&facts, &two_hop);
            let us = t.elapsed().as_secs_f64() * 1e6;
            println!("           {n:>5} edges: {:>6} derived in {us:>8.1}µs  ({:.2}M facts/s in)", derived.len(), (n as f64 / (us / 1e6)) / 1e6);
        }
        //    (b) recursive transitive closure (fixpoint) over a chain of N edges.
        println!("[kb-fix]  recursive transitive closure (fixpoint) → derived / time:");
        let closure = vec![
            abl_bridge::KbRule { head: "path".into(), params: vec!["x".into(), "y".into()], body: vec![("edge".into(), vec!["x".into(), "y".into()])] },
            abl_bridge::KbRule { head: "path".into(), params: vec!["x".into(), "z".into()], body: vec![("edge".into(), vec!["x".into(), "y".into()]), ("path".into(), vec!["y".into(), "z".into()])] },
        ];
        for &n in &[20usize, 40, 80] {
            let facts: Vec<abl_bridge::GroundFact> = (0..n)
                .map(|i| ("edge".to_string(), vec![i.to_string(), (i + 1).to_string()]))
                .collect();
            let t = Instant::now();
            let derived = abl_bridge::evaluate_kb(&facts, &closure);
            let ms = t.elapsed().as_secs_f64() * 1e3;
            println!("           {n:>3}-edge chain: {:>5} closure facts in {ms:>7.2}ms", derived.len());
        }

        // 4. No-exec decode + describe of a 32-layer artifact.
        let spec = net_spec(32);
        let module = parser::parse(&lexer::lex(&builder::to_mg_source(&spec))).unwrap();
        let (blob, _) = abl::encode_module(&module);
        let iters = 3000;
        let t = Instant::now();
        for _ in 0..iters {
            let items = abl::decode_container(&blob).unwrap();
            let _ = abl::decode_symbols(&blob);
            for it in &items {
                let _ = abl_bridge::decompile_symbolic(&it.expr);
            }
        }
        let per_us = t.elapsed().as_secs_f64() / iters as f64 * 1e6;
        println!("[decode] {}B artifact → decode+symbols+describe {per_us:.1}µs/op", blob.len());

        println!("(medians over many iterations; absolute numbers are machine-dependent)\n");
    }
}
