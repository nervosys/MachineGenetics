mod aci;
mod agent_runtime;
mod ast;
mod autograd;
mod backends;
mod bench;
mod builder;
#[cfg(feature = "cuda")]
mod cuda_backend;
mod certs;
mod cli_manifest;
mod codegen_bridge;
mod consensus;
mod cost;
mod cost_calibration;
mod crdt;
mod decompose;
mod effects;
mod eval;
mod elision;
mod evolve_gen;
mod ffi_gen;
mod fmt;
#[cfg(test)]
mod fuzz;
mod forge;
mod grammar;
mod heal;
mod hir;
mod hot_reload;
mod lease;
mod legacy;
mod lexer;
mod logic;
mod manifest;
mod mlir;
mod nl_engine;
mod ontology;
mod parser;
mod perf_annot;
mod perf_measure;
mod rain;
mod rap;
mod recover;
mod resolve;
mod rmi_ontology_adapter;
mod rmi_runtime_adapter;
mod abl;
mod abl_bridge;
mod abl_compute;
mod abl_shape;
mod sandbox;
mod semantic_vcs;
mod shape;
mod spine_bridge;
mod skb;
mod stdlib_ext;
mod swarm_bus;
mod swarm_sdk;
mod synthesis;
mod token_budget;
mod token_canonical;
mod types;
mod verify;

use std::io::Read;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let no_elision = args.iter().any(|a| a == "--no-elision");
    let syntax_legacy = args.iter().any(|a| a == "--syntax=legacy");
    let token_report = args.iter().any(|a| a == "--token-report");
    // `--check --json` emits a deterministic, machine-readable diagnostic
    // stream instead of human prose, so an agent parses errors structurally
    // (code/span/category/fix) rather than scraping stderr.
    let json_out = args.iter().any(|a| a == "--json");
    // Optional --backend=<name> selects hardware accelerator for any
    // subsequent --run=abl-bytes / --target=abl-run dispatch. Lives
    // outside the main flag table so it can attach to any dispatching
    // subcommand without per-command plumbing. Default: cpu.
    let backend_name: String = args
        .iter()
        .find_map(|a| a.strip_prefix("--backend=").map(str::to_string))
        .unwrap_or_else(|| "cpu".to_string());
    // Optional --backends-file <path> registers extra backend
    // descriptors at runtime. Stacks with env/home loading so
    // operators can layer per-deployment overrides.
    let backends_file: Option<String> = args
        .iter()
        .find_map(|a| a.strip_prefix("--backends-file=").map(str::to_string));
    if let Some(path) = &backends_file {
        match backends::register_descriptors_from_file(path) {
            Ok(n) => eprintln!("// registered {n} backend descriptor(s) from {path}"),
            Err(e) => eprintln!("// --backends-file: {e}"),
        }
    }
    // Collect positional-ish args (skip flag-style args)
    let filtered: Vec<&str> = args
        .iter()
        .skip(1)
        .filter(|a| {
            !matches!(
                a.as_str(),
                "--no-elision" | "--syntax=legacy" | "--syntax=canonical" | "--token-report"
                    | "--json"
            ) && !a.starts_with("--backend=")
              && !a.starts_with("--backends-file=")
        })
        .map(|s| s.as_str())
        .collect();

    match filtered.first().copied() {
        Some("--manifest") => {
            // Agent-facing capability index (cheap discovery root).
            print!("{}", cli_manifest::manifest());
        }
        Some("--describe") => {
            match filtered.get(1).copied().and_then(cli_manifest::describe) {
                Some(d) => println!("{d}"),
                None => {
                    eprintln!("unknown mode; valid modes:");
                    eprint!("{}", cli_manifest::manifest());
                    std::process::exit(2);
                }
            }
        }
        Some("--rap") => {
            let addr = filtered.get(1).copied().unwrap_or("127.0.0.1:9876");
            rap::serve(addr);
        }
        Some("--emit-ontology") => {
            // Dump the complete ontology to disk as static JSON.
            // `--emit-ontology [path]` (default: MECHGEN_ONTOLOGY.json).
            let out = filtered.get(1).copied().unwrap_or("MECHGEN_ONTOLOGY.json");
            let value = ontology::build();
            let json = serde_json::to_string_pretty(&value).unwrap_or_else(|e| {
                eprintln!("emit-ontology: serialize: {e}");
                std::process::exit(1);
            });
            if let Err(e) = std::fs::write(out, &json) {
                eprintln!("emit-ontology: write {out}: {e}");
                std::process::exit(1);
            }
            println!("wrote {} bytes to {out}", json.len());
        }
        Some("--fmt-compact") => {
            let path = filtered.get(1).unwrap_or_else(|| {
                eprintln!("Usage: MechGen-parse --fmt-compact <file>");
                std::process::exit(1);
            });
            let source = std::fs::read_to_string(path).unwrap_or_else(|e| {
                eprintln!("Error reading {path}: {e}");
                std::process::exit(1);
            });
            let source = if syntax_legacy {
                legacy::translate(&source)
            } else {
                source
            };
            let tokens = lexer::lex(&source);
            match parser::parse(&tokens) {
                Ok(module) => {
                    let module = if !no_elision {
                        elision::elide(&module)
                    } else {
                        module
                    };
                    println!("{}", fmt::format_agent(&module));
                }
                Err(e) => {
                    eprintln!("{path}:{}:{}: parse error: {}", e.line, e.col, e.message);
                    std::process::exit(1);
                }
            }
        }
        Some("--fmt-expand") => {
            let path = filtered.get(1).unwrap_or_else(|| {
                eprintln!("Usage: MechGen-parse --fmt-expand <file>");
                std::process::exit(1);
            });
            let source = std::fs::read_to_string(path).unwrap_or_else(|e| {
                eprintln!("Error reading {path}: {e}");
                std::process::exit(1);
            });
            let source = if syntax_legacy {
                legacy::translate(&source)
            } else {
                source
            };
            let tokens = lexer::lex(&source);
            match parser::parse(&tokens) {
                Ok(module) => {
                    let module = if !no_elision {
                        elision::elide(&module)
                    } else {
                        module
                    };
                    println!("{}", fmt::format_human(&module));
                }
                Err(e) => {
                    eprintln!("{path}:{}:{}: parse error: {}", e.line, e.col, e.message);
                    std::process::exit(1);
                }
            }
        }
        Some("--check") => {
            let path = filtered.get(1).unwrap_or_else(|| {
                eprintln!("Usage: MechGen-parse --check <file> [--no-elision] [--token-report]");
                std::process::exit(1);
            });
            let source = std::fs::read_to_string(path).unwrap_or_else(|e| {
                eprintln!("Error reading {path}: {e}");
                std::process::exit(1);
            });
            if json_out {
                run_check_json(&source, path, !no_elision, syntax_legacy);
            } else {
                run_check(&source, path, !no_elision, syntax_legacy, token_report);
            }
        }
        Some("--target=abl-bytes") => {
            let path = filtered.get(1).unwrap_or_else(|| {
                eprintln!("Usage: MechGen-parse --target=abl-bytes <file.mg> [<out.abl>]");
                std::process::exit(1);
            });
            let out_path = filtered.get(2).map(|s| s.to_string());
            let source = std::fs::read_to_string(path).unwrap_or_else(|e| {
                eprintln!("Error reading {path}: {e}");
                std::process::exit(1);
            });
            let source = if syntax_legacy {
                legacy::translate(&source)
            } else {
                source
            };
            let tokens = lexer::lex(&source);
            match parser::parse(&tokens) {
                Ok(module) => {
                    let module = if !no_elision {
                        elision::elide(&module)
                    } else {
                        module
                    };
                    run_emit_abl_bytes(&module, path, out_path.as_deref());
                }
                Err(e) => {
                    eprintln!("{path}:{}:{}: parse error: {}", e.line, e.col, e.message);
                    std::process::exit(1);
                }
            }
        }
        Some("--build=abl") => {
            // Tool-mediated construction: agent emits a compact JSON spec (not
            // source text) — a `net` (neural) or `kb` (symbolic) spec — which we
            // validate structurally (reject-by-construction) and lower to the
            // deterministic Agentic Binary Language artifact. See builder.rs.
            let path = filtered.get(1).unwrap_or_else(|| {
                eprintln!("Usage: MechGen-parse --build=abl <spec.json> [<out.abl>]");
                std::process::exit(1);
            });
            let out_path = filtered.get(2).map(|s| s.to_string());
            let json = std::fs::read_to_string(path).unwrap_or_else(|e| {
                eprintln!("Error reading {path}: {e}");
                std::process::exit(1);
            });
            // Detect the spec kind from its discriminating key.
            let value: serde_json::Value = match serde_json::from_str(&json) {
                Ok(v) => v,
                Err(e) => {
                    println!(
                        "{}",
                        serde_json::json!({"ok": false, "errors": [
                            {"code":"B0000","message": format!("malformed spec JSON: {e}"),
                             "fix":"emit a valid spec ({\"net\":..} or {\"kb\":..}); see --build=schema"}]})
                    );
                    std::process::exit(1);
                }
            };
            let reject = |errs: Vec<builder::BuildError>| {
                let arr: Vec<_> = errs.iter().map(|e| e.as_json()).collect();
                println!("{}", serde_json::json!({"ok": false, "errors": arr}));
                std::process::exit(1);
            };
            // `--fix`: attempt deterministic auto-repair (net/swarm) before rejecting.
            let fix = args.iter().any(|a| a == "--fix");
            let (src, kind, name, count) = if value.get("items").is_some() {
                let spec: builder::UnifiedSpec = serde_json::from_value(value).unwrap_or_else(|e| {
                    reject(vec![builder::BuildError::malformed(format!("bad unified spec: {e}"))]);
                    unreachable!()
                });
                let errs = builder::validate_unified(&spec);
                if !errs.is_empty() { reject(errs); }
                let n = spec.items.len();
                (builder::to_mg_source_unified(&spec), "unified", format!("{n}-item container"), n)
            } else if value.get("kb").is_some() {
                let spec: builder::KbSpec = serde_json::from_value(value).unwrap_or_else(|e| {
                    reject(vec![builder::BuildError::malformed(format!("bad kb spec: {e}"))]);
                    unreachable!()
                });
                let errs = builder::validate_kb(&spec);
                if !errs.is_empty() { reject(errs); }
                let n = spec.facts.len() + spec.rules.len();
                (builder::to_mg_source_kb(&spec), "kb", spec.kb.clone(), n)
            } else if value.get("swarm").is_some() {
                // `swarm` before `agent` — a swarm spec also has an "agent" field.
                let mut spec: builder::SwarmSpec = serde_json::from_value(value).unwrap_or_else(|e| {
                    reject(vec![builder::BuildError::malformed(format!("bad swarm spec: {e}"))]);
                    unreachable!()
                });
                let mut errs = builder::validate_swarm(&spec);
                if !errs.is_empty() && fix {
                    for f in builder::repair_swarm(&mut spec) { eprintln!("// fix: {f}"); }
                    errs = builder::validate_swarm(&spec);
                }
                if !errs.is_empty() { reject(errs); }
                (builder::to_mg_source_swarm(&spec), "swarm", spec.swarm.clone(), 1)
            } else if value.get("agent").is_some() {
                let spec: builder::AgentSpec = serde_json::from_value(value).unwrap_or_else(|e| {
                    reject(vec![builder::BuildError::malformed(format!("bad agent spec: {e}"))]);
                    unreachable!()
                });
                let errs = builder::validate_agent(&spec);
                if !errs.is_empty() { reject(errs); }
                let n = spec.capabilities.len();
                (builder::to_mg_source_agent(&spec), "agent", spec.agent.clone(), n)
            } else {
                let mut spec: builder::NetSpec = serde_json::from_value(value).unwrap_or_else(|e| {
                    reject(vec![builder::BuildError::malformed(format!("bad net spec: {e}"))]);
                    unreachable!()
                });
                let mut errs = builder::validate(&spec);
                if !errs.is_empty() && fix {
                    for f in builder::repair_net(&mut spec) { eprintln!("// fix: {f}"); }
                    errs = builder::validate(&spec);
                }
                if !errs.is_empty() { reject(errs); }
                let n = spec.layers.len();
                (builder::to_mg_source(&spec), "net", spec.net.clone(), n)
            };
            // Valid → construct canonical source → reuse the tested lowering.
            match parser::parse(&lexer::lex(&src)) {
                Ok(module) => {
                    run_emit_abl_bytes(&module, path, out_path.as_deref());
                    eprintln!("// built `{name}` from {kind} spec ({count} items) → Agentic Binary Language");
                }
                Err(e) => {
                    eprintln!("internal: generated {kind} failed to parse: {}:{} {}", e.line, e.col, e.message);
                    std::process::exit(1);
                }
            }
        }
        Some("--build=schema") => {
            // Tool-mediated construction, step 1: emit the typed, self-describing
            // construction schema. An agent fetches this ONCE (prompt-cacheable)
            // and then emits specs that validate first-try — the amortized-token
            // + discoverability half of the paradigm. Deterministic JSON.
            println!(
                "{}",
                serde_json::to_string_pretty(&builder::build_schema())
                    .unwrap_or_else(|_| "{}".to_string())
            );
        }
        Some("--describe=abl") => {
            // Tool-mediated construction, step 3: no-exec structured
            // introspection. Loading the artifact is pure bounds-checked data
            // decode — it NEVER executes code — and yields a machine-readable
            // description the agent can verify against its spec.
            let path = filtered.get(1).unwrap_or_else(|| {
                eprintln!("Usage: MechGen-parse --describe=abl <file.abl>");
                std::process::exit(1);
            });
            let blob = std::fs::read(path).unwrap_or_else(|e| {
                eprintln!("Error reading {path}: {e}");
                std::process::exit(1);
            });
            run_describe_abl_bytes(&blob);
        }
        Some("--run=abl") => {
            // Execute each item's semantics as a pure-data interpreter (no
            // arbitrary code runs): kb → forward-chain to fixpoint; agent →
            // capability-policy decisions; swarm → consensus over proposals.
            // Optional `--input <json>` supplies {"ops":[..]} for agents and
            // {"proposals":[..]} for swarms. Neural items defer to --run=abl-bytes.
            let path = filtered.get(1).unwrap_or_else(|| {
                eprintln!("Usage: MechGen-parse --run=abl <file.abl> [--input <json>]");
                std::process::exit(1);
            });
            let blob = std::fs::read(path).unwrap_or_else(|e| {
                eprintln!("Error reading {path}: {e}");
                std::process::exit(1);
            });
            let input: serde_json::Value = args
                .iter()
                .position(|a| a == "--input")
                .and_then(|i| args.get(i + 1))
                .and_then(|s| serde_json::from_str(s).ok())
                .unwrap_or(serde_json::Value::Null);
            run_eval_abl_bytes(&blob, &input);
        }
        Some("--eval") => {
            // Run a pure function and print its value — executes the §8 standard
            // vocabulary (map/filter/fold/…) and the arithmetic/control flow
            // around it. Usage: MechGen-parse --eval <file.mg> <fn> [int args...]
            let path = filtered.get(1).unwrap_or_else(|| {
                eprintln!("Usage: MechGen-parse --eval <file.mg> <fn> [int args...]");
                std::process::exit(1);
            });
            let func = filtered.get(2).map(|s| &**s).unwrap_or("main");
            let nums: Vec<i64> = filtered.iter().skip(3).filter_map(|s| s.parse().ok()).collect();
            let src = std::fs::read_to_string(path).unwrap_or_else(|e| {
                eprintln!("Error reading {path}: {e}");
                std::process::exit(1);
            });
            match eval::run_source(&src, func, &nums) {
                Ok(v) => println!("{v}"),
                Err(e) => {
                    eprintln!("eval error: {e}");
                    std::process::exit(1);
                }
            }
        }
        Some("--rain") => {
            // Digital rain: a Matrix-inspired dense-UTF-8 representation. One
            // glyph per token; writes <file>.rain (glyph stream) + <file>.legend.
            let path = filtered.get(1).unwrap_or_else(|| {
                eprintln!("Usage: MechGen-parse --rain <file.mg>");
                std::process::exit(1);
            });
            let src = std::fs::read_to_string(path).unwrap_or_else(|e| {
                eprintln!("Error reading {path}: {e}");
                std::process::exit(1);
            });
            let r = rain::encode(&src);
            let src_chars = src.chars().count();
            let _ = std::fs::write(format!("{path}.rain"), &r.stream);
            let _ = std::fs::write(format!("{path}.legend"), r.legend_text());
            println!("// MechGen digital rain — {path}");
            println!(
                "// source: {src_chars} chars / {} bytes   rain: {} glyphs ({} distinct)   legend: {} entries",
                src.len(), r.tokens(), r.distinct(), r.legend.len()
            );
            println!(
                "// CHAR compression: {:.2}x ({} → {} codepoints). NOTE: this is character/visual",
                src_chars as f64 / r.tokens().max(1) as f64, src_chars, r.tokens()
            );
            println!("// density, NOT token efficiency — see agentic-eval rain_tokens (BPE measured).");
            println!("// wrote {path}.rain + {path}.legend");
            println!("\n{}", r.stream);
        }
        Some("--spine=profile") | Some("--spine=swarm") | Some("--spine=frame") => {
            // SPINE collaboration bridge: emit SPINE-protocol-shaped JSON for an
            // ABL agent/swarm spec, or a binary collaboration frame for an .abl
            // artifact. See SPINE_COLLABORATION.md + spine_bridge.rs.
            let mode = filtered[0];
            let path = filtered.get(1).unwrap_or_else(|| {
                eprintln!("Usage: MechGen-parse {mode} <spec.json | file.abl>");
                std::process::exit(1);
            });
            let out = match mode {
                "--spine=frame" => {
                    let blob = std::fs::read(path).unwrap_or_else(|e| {
                        eprintln!("Error reading {path}: {e}");
                        std::process::exit(1);
                    });
                    spine_bridge::artifact_frame(&blob)
                }
                _ => {
                    let json = std::fs::read_to_string(path).unwrap_or_else(|e| {
                        eprintln!("Error reading {path}: {e}");
                        std::process::exit(1);
                    });
                    if mode == "--spine=swarm" {
                        match serde_json::from_str::<builder::SwarmSpec>(&json) {
                            Ok(s) => spine_bridge::swarm_task(&s),
                            Err(e) => {
                                eprintln!("bad swarm spec: {e}");
                                std::process::exit(1);
                            }
                        }
                    } else {
                        match serde_json::from_str::<builder::AgentSpec>(&json) {
                            Ok(a) => spine_bridge::agent_profile(&a),
                            Err(e) => {
                                eprintln!("bad agent spec: {e}");
                                std::process::exit(1);
                            }
                        }
                    }
                }
            };
            println!("{}", serde_json::to_string_pretty(&out).unwrap_or_else(|_| "{}".to_string()));
        }
        Some("--from=abl-bytes") => {
            let path = filtered.get(1).unwrap_or_else(|| {
                eprintln!("Usage: MechGen-parse --from=abl-bytes <file.abl>");
                std::process::exit(1);
            });
            let blob = std::fs::read(path).unwrap_or_else(|e| {
                eprintln!("Error reading {path}: {e}");
                std::process::exit(1);
            });
            run_decode_abl_bytes(&blob, path);
        }
        Some("--run=abl-bytes") => {
            let path = filtered.get(1).unwrap_or_else(|| {
                eprintln!("Usage: MechGen-parse --run=abl-bytes <file.abl>");
                std::process::exit(1);
            });
            let blob = std::fs::read(path).unwrap_or_else(|e| {
                eprintln!("Error reading {path}: {e}");
                std::process::exit(1);
            });
            run_dispatch_abl_bytes(&blob, path, &backend_name);
        }
        Some("--target=abl-generate") => {
            let path = filtered.get(1).unwrap_or_else(|| {
                eprintln!("Usage: MechGen-parse --target=abl-generate <file>");
                std::process::exit(1);
            });
            let source = std::fs::read_to_string(path).unwrap_or_else(|e| {
                eprintln!("Error reading {path}: {e}");
                std::process::exit(1);
            });
            let source = if syntax_legacy {
                legacy::translate(&source)
            } else {
                source
            };
            let tokens = lexer::lex(&source);
            match parser::parse(&tokens) {
                Ok(module) => {
                    let module = if !no_elision {
                        elision::elide(&module)
                    } else {
                        module
                    };
                    run_generate(&module, path);
                }
                Err(e) => {
                    eprintln!("{path}:{}:{}: parse error: {}", e.line, e.col, e.message);
                    std::process::exit(1);
                }
            }
        }
        Some("--target=abl-infer") => {
            let path = filtered.get(1).unwrap_or_else(|| {
                eprintln!("Usage: MechGen-parse --target=abl-infer <file>");
                std::process::exit(1);
            });
            let source = std::fs::read_to_string(path).unwrap_or_else(|e| {
                eprintln!("Error reading {path}: {e}");
                std::process::exit(1);
            });
            let source = if syntax_legacy {
                legacy::translate(&source)
            } else {
                source
            };
            let tokens = lexer::lex(&source);
            match parser::parse(&tokens) {
                Ok(module) => {
                    let module = if !no_elision {
                        elision::elide(&module)
                    } else {
                        module
                    };
                    run_infer(&module, path);
                }
                Err(e) => {
                    eprintln!("{path}:{}:{}: parse error: {}", e.line, e.col, e.message);
                    std::process::exit(1);
                }
            }
        }
        Some("--target=abl-train") => {
            let path = filtered.get(1).unwrap_or_else(|| {
                eprintln!("Usage: MechGen-parse --target=abl-train <file>");
                std::process::exit(1);
            });
            let source = std::fs::read_to_string(path).unwrap_or_else(|e| {
                eprintln!("Error reading {path}: {e}");
                std::process::exit(1);
            });
            let source = if syntax_legacy {
                legacy::translate(&source)
            } else {
                source
            };
            let tokens = lexer::lex(&source);
            match parser::parse(&tokens) {
                Ok(module) => {
                    let module = if !no_elision {
                        elision::elide(&module)
                    } else {
                        module
                    };
                    run_train(&module, path);
                }
                Err(e) => {
                    eprintln!("{path}:{}:{}: parse error: {}", e.line, e.col, e.message);
                    std::process::exit(1);
                }
            }
        }
        Some("--target=abl-compute") => {
            let path = filtered.get(1).unwrap_or_else(|| {
                eprintln!("Usage: MechGen-parse --target=abl-compute <file>");
                std::process::exit(1);
            });
            let source = std::fs::read_to_string(path).unwrap_or_else(|e| {
                eprintln!("Error reading {path}: {e}");
                std::process::exit(1);
            });
            let source = if syntax_legacy {
                legacy::translate(&source)
            } else {
                source
            };
            let tokens = lexer::lex(&source);
            match parser::parse(&tokens) {
                Ok(module) => {
                    let module = if !no_elision {
                        elision::elide(&module)
                    } else {
                        module
                    };
                    let lowered = abl_bridge::lower_module(&module);
                    let backend = rmi::compute::cpu::CpuBackend::new();
                    println!("// MechGen → Agentic Binary Language → CpuBackend dispatch for {path}");
                    for (name, expr) in &lowered.items {
                        // Pre-flight: infer shapes and report mismatches.
                        let shape_report = abl_shape::infer_shape(expr, &[8]);
                        for m in &shape_report.mismatches {
                            eprintln!(
                                "shape error in {name}: op {:?} expected last={} but got shape {:?}",
                                m.op, m.expected_last, m.got
                            );
                        }
                        let inferred = abl_compute::infer_input_shape(expr);
                        let shape: Vec<usize> = inferred.unwrap_or_else(|| vec![8]);
                        match abl_compute::run_pipeline(&backend, expr, &shape, 1.0) {
                            Ok(r) => println!(
                                "// {name}: dispatched={} unsupported={:?} output_sum={:.4} shape={:?} (input={:?})",
                                r.dispatched, r.unsupported, r.output_sum, r.output.shape, shape
                            ),
                            Err(e) => println!("// {name}: backend error: {e} (input={shape:?})"),
                        }
                    }
                }
                Err(e) => {
                    eprintln!("{path}:{}:{}: parse error: {}", e.line, e.col, e.message);
                    std::process::exit(1);
                }
            }
        }
        Some("--target=abl-run") => {
            let path = filtered.get(1).unwrap_or_else(|| {
                eprintln!("Usage: MechGen-parse --target=abl-run <file>");
                std::process::exit(1);
            });
            let source = std::fs::read_to_string(path).unwrap_or_else(|e| {
                eprintln!("Error reading {path}: {e}");
                std::process::exit(1);
            });
            let source = if syntax_legacy {
                legacy::translate(&source)
            } else {
                source
            };
            let tokens = lexer::lex(&source);
            match parser::parse(&tokens) {
                Ok(module) => {
                    let module = if !no_elision {
                        elision::elide(&module)
                    } else {
                        module
                    };
                    let lowered = abl_bridge::lower_module(&module);
                    let mut vm = rmi::lang::Vm::new();
                    println!("// MechGen → Agentic Binary Language → VM execution for {path}");
                    for (name, expr) in &lowered.items {
                        let families = abl_bridge::expr_op_families(expr);
                        let stub_families: Vec<_> = families
                            .iter()
                            .filter(|f| abl_bridge::is_stubbed_family(**f))
                            .map(|f| format!("{f:?}"))
                            .collect();
                        // JIT path: pure-math fragments compile, neural/symbolic/agent
                        // ops transparently fall back to the tree-walking interpreter.
                        match vm.eval_jit(expr) {
                            Ok(val) => println!(
                                "// {name}: ok  (hash={:016x} result={:?})",
                                expr.content_hash(),
                                val
                            ),
                            Err(_) if !stub_families.is_empty() => println!(
                                "// {name}: stub (hash={:016x} families={} — neural/symbolic/agent ops require compute backend, not VM)",
                                expr.content_hash(),
                                stub_families.join(",")
                            ),
                            Err(e) => println!(
                                "// {name}: err (hash={:016x} error={:?})",
                                expr.content_hash(),
                                e
                            ),
                        }
                    }
                    for diag in &lowered.diagnostics {
                        eprintln!("warning: {diag}");
                    }
                }
                Err(e) => {
                    eprintln!("{path}:{}:{}: parse error: {}", e.line, e.col, e.message);
                    std::process::exit(1);
                }
            }
        }
        Some("--target=abl") => {
            let path = filtered.get(1).unwrap_or_else(|| {
                eprintln!("Usage: MechGen-parse --target=abl <file>");
                std::process::exit(1);
            });
            let source = std::fs::read_to_string(path).unwrap_or_else(|e| {
                eprintln!("Error reading {path}: {e}");
                std::process::exit(1);
            });
            let source = if syntax_legacy {
                legacy::translate(&source)
            } else {
                source
            };
            let tokens = lexer::lex(&source);
            match parser::parse(&tokens) {
                Ok(module) => {
                    let module = if !no_elision {
                        elision::elide(&module)
                    } else {
                        module
                    };
                    let lowered = abl_bridge::lower_module(&module);
                    let (mlir_items, abl_items) = abl_bridge::OpFamilyRouter::partition(&module);
                    println!("// MechGen → Agentic Binary Language lowering for {path}");
                    println!("// MLIR-routed items: {}", mlir_items.len());
                    println!("// Agentic Binary Language-routed items: {}", abl_items.len());
                    for diag in &lowered.diagnostics {
                        eprintln!("warning: {diag}");
                    }
                    for (name, expr) in &lowered.items {
                        let bytes = rmi::lang::codec::Encoder::encode_expr_only(expr);
                        println!(
                            "// {name}: nodes={} depth={} hash={:016x} wire={}B",
                            expr.node_count(),
                            expr.depth(),
                            expr.content_hash(),
                            bytes.len()
                        );
                    }
                }
                Err(e) => {
                    eprintln!("{path}:{}:{}: parse error: {}", e.line, e.col, e.message);
                    std::process::exit(1);
                }
            }
        }
        Some("--pipeline") => {
            let path = filtered.get(1).unwrap_or_else(|| {
                eprintln!("Usage: MechGen-parse --pipeline <file> [--no-elision] [--syntax=legacy] [--token-report]");
                std::process::exit(1);
            });
            let source = std::fs::read_to_string(path).unwrap_or_else(|e| {
                eprintln!("Error reading {path}: {e}");
                std::process::exit(1);
            });
            run_pipeline(&source, path, !no_elision, syntax_legacy, token_report);
        }
        Some(path) => {
            let source = std::fs::read_to_string(path).unwrap_or_else(|e| {
                eprintln!("Error reading {path}: {e}");
                std::process::exit(1);
            });
            run_parse(&source, path, !no_elision, syntax_legacy, token_report);
        }
        None => {
            let mut source = String::new();
            std::io::stdin().read_to_string(&mut source).unwrap();
            run_parse(&source, "<stdin>", !no_elision, syntax_legacy, token_report);
        }
    }
}

/// Drive `--target=abl-train`: find each `train` block, locate its named
/// `net`, lower the net to Agentic Binary Language, run N epochs of SGD on a synthetic
/// dataset, and report per-step loss.
///
/// Defaults when the `.mg` source omits them:
/// - **epochs:** 50
/// - **learning rate:** 0.05
/// - **dataset:** four samples of the form `y = sum(x)` matching the
///   net's first Linear input dim and final output dim.
/// Magic bytes for the per-module Agentic Binary Language-bytes container format. Distinct
/// from the per-expression `MGPS` checkpoint magic.
const ABL_MAGIC: &[u8; 4] = b"ABL1";
// Single source of truth — these private decoders share the container version
// with the `abl` codec, so a bump (e.g. v3's REPEAT folding) can't drift.
const ABL_VERSION: u16 = abl::ABL_VERSION;

/// Drive `--target=abl-bytes`: lower every Agentic Binary Language-routed item in the module
/// to binary Agentic Binary Language via the bridge + RMI codec, then write a single framed
/// blob to disk (or stdout-summarise if no out path given).
///
/// Container layout:
/// ```text
///   magic    "Agentic Binary Language" (4 bytes)
///   version  u16 = 3   (v3: per-item exprs are REPEAT-folded)
///   count    u32  — number of items
///   for each item:
///     name_len u32
///     name     UTF-8 bytes
///     expr_len u32
///     expr     codec::Encoder::encode_expr_only output
/// ```
fn run_emit_abl_bytes(module: &ast::Module, src_path: &str, out_path: Option<&str>) {
    let lowered_diags = abl_bridge::lower_module(module).diagnostics;
    let (blob, per_item_summary) = abl::encode_module(module);
    if per_item_summary.is_empty() {
        println!("// {src_path}: no Agentic Binary Language-routed items (no net/kb/agent/swarm/train/evolve)");
        return;
    }

    let text_bytes = std::fs::metadata(src_path).map(|m| m.len()).unwrap_or(0);
    let total_ml = blob.len() as u64;

    println!("// MechGen → Agentic Binary Language bytes for {src_path}");
    println!(
        "// text source: {} bytes    Agentic Binary Language container: {} bytes    ratio: {:.3} ({:.1}% reduction)",
        text_bytes,
        total_ml,
        if text_bytes > 0 { total_ml as f64 / text_bytes as f64 } else { 0.0 },
        if text_bytes > 0 { (1.0 - total_ml as f64 / text_bytes as f64) * 100.0 } else { 0.0 },
    );
    for (name, sz, hash) in &per_item_summary {
        println!("//   {name}: {sz}B  hash={hash:016x}");
    }
    for d in &lowered_diags {
        eprintln!("warning: {d}");
    }

    if let Some(out) = out_path {
        match std::fs::write(out, &blob) {
            Ok(()) => println!("// wrote {} bytes to {}", blob.len(), out),
            Err(e) => eprintln!("write {out}: {e}"),
        }
    }
}

/// Drive `--describe=abl`: decode a `.abl` container as PURE DATA (no
/// execution) and emit a deterministic, machine-readable JSON description —
/// container size, per-item content hash, and the reconstructed layer
/// structure. This is the introspection half of the tool-mediated paradigm:
/// the agent verifies what it built without ever running it.
/// Drive `--run=abl`: evaluate each item's executable semantics. For `kb` items
/// this forward-chains the Horn-clause program to its least fixpoint and reports
/// the derived facts — a safe, terminating, pure-data interpretation.
fn run_eval_abl_bytes(blob: &[u8], input: &serde_json::Value) {
    let items = match abl::decode_container(blob) {
        Ok(i) => i,
        Err(e) => {
            println!("{}", serde_json::json!({ "ok": false, "error": e }));
            std::process::exit(1);
        }
    };
    let symbols = abl::decode_symbols(blob).unwrap_or_default();
    let name = |id: u32| symbols.get(id as usize).cloned().unwrap_or_default();
    // Optional execution input: agents take {"ops":[..]}, swarms {"proposals":[..]}.
    let ops: Vec<String> = input
        .get("ops")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
        .unwrap_or_default();
    let proposals: Vec<i64> = input
        .get("proposals")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|x| x.as_i64()).collect())
        .unwrap_or_default();
    let mut items_json = Vec::new();
    for item in &items {
        let sym = abl_bridge::decompile_symbolic(&item.expr);
        let is_kb = !sym.fact_syms.is_empty() || !sym.rule_syms.is_empty();
        if is_kb {
            let facts: Vec<abl_bridge::GroundFact> = sym
                .fact_syms
                .iter()
                .zip(&sym.fact_arg_syms)
                .map(|(&p, terms)| (name(p), terms.iter().map(|&t| name(t)).collect()))
                .collect();
            let rules: Vec<abl_bridge::KbRule> = sym
                .rule_syms
                .iter()
                .zip(&sym.rule_param_syms)
                .zip(&sym.rule_body_syms)
                .map(|((&r, params), body)| abl_bridge::KbRule {
                    head: name(r),
                    params: params.iter().map(|&p| name(p)).collect(),
                    body: body
                        .iter()
                        .map(|(p, a)| (name(*p), a.iter().map(|&x| name(x)).collect()))
                        .collect(),
                })
                .collect();
            let derived = abl_bridge::evaluate_kb(&facts, &rules);
            let derived_json: Vec<serde_json::Value> = derived
                .iter()
                .map(|(p, a)| serde_json::json!({ "pred": p, "args": a }))
                .collect();
            items_json.push(serde_json::json!({
                "name": item.name, "kind": "kb",
                "given_facts": facts.len(), "rules": rules.len(),
                "derived": derived_json,
            }));
        } else if let Some(ag) = abl_bridge::decompile_agentic(&item.expr) {
            if ag.is_swarm {
                let topology = ag.topology_sym.map(name).unwrap_or_else(|| "(none)".into());
                let consensus = ag.consensus_sym.map(name).unwrap_or_else(|| "majority".into());
                let r = abl_bridge::eval_swarm_consensus(ag.size.unwrap_or(1), &topology, &consensus, &proposals);
                items_json.push(serde_json::json!({
                    "name": item.name, "kind": "swarm",
                    "size": r.size, "topology": r.topology, "consensus": r.consensus,
                    "rounds_to_converge": r.rounds_to_converge,
                    "proposals": proposals,
                    "decided": r.decided, "reason": r.reason,
                }));
            } else {
                let caps: Vec<String> = ag.cap_syms.iter().map(|&id| name(id)).collect();
                let approvals: Vec<String> = ag.approval_syms.iter().map(|&id| name(id)).collect();
                if ops.is_empty() {
                    // No requested ops → report the policy surface.
                    items_json.push(serde_json::json!({
                        "name": item.name, "kind": "agent",
                        "capabilities": caps, "requires_approval": approvals,
                        "note": "policy surface — pass --input {\"ops\":[..]} to evaluate decisions",
                    }));
                } else {
                    let decisions: Vec<serde_json::Value> = abl_bridge::eval_agent_policy(&caps, &approvals, &ops)
                        .iter()
                        .map(|(op, d)| serde_json::json!({ "op": op, "decision": d.tag() }))
                        .collect();
                    items_json.push(serde_json::json!({
                        "name": item.name, "kind": "agent",
                        "capabilities": caps, "requires_approval": approvals,
                        "decisions": decisions,
                    }));
                }
            }
        } else {
            items_json.push(serde_json::json!({
                "name": item.name, "kind": "net",
                "note": "neural item — use --run=abl-bytes for the forward pass",
            }));
        }
    }
    let out = serde_json::json!({ "ok": true, "exec": "pure-data interpretation", "items": items_json });
    println!("{}", serde_json::to_string_pretty(&out).unwrap_or_else(|_| "{}".to_string()));
}

fn run_describe_abl_bytes(blob: &[u8]) {
    match abl::decode_container(blob) {
        Ok(items) => {
            // Symbol table (v2): lets us recover predicate/rule NAMES for kb items.
            let symbols = abl::decode_symbols(blob).unwrap_or_default();
            let sym_name = |id: u32| -> Option<String> { symbols.get(id as usize).cloned() };
            let items_json: Vec<serde_json::Value> = items
                .iter()
                .map(|item| {
                    let mut entry = serde_json::json!({
                        "name": item.name,
                        "expr_bytes": item.expr_bytes_len,
                        "content_hash": format!("{:016x}", item.expr.content_hash()),
                    });
                    let map = entry.as_object_mut().unwrap();
                    // Symbolic ops (RESOLVE/UNIFY) appear ONLY in kb artifacts, so a
                    // non-empty symbolic view is the precise net-vs-kb discriminator
                    // (decompile() would otherwise surface them as pseudo-"layers").
                    let sym = abl_bridge::decompile_symbolic(&item.expr);
                    let is_kb = !sym.fact_arities.is_empty() || !sym.rule_param_counts.is_empty();
                    // Agentic ops (SPAWN) appear only in agent/swarm artifacts.
                    let ag = if is_kb { None } else { abl_bridge::decompile_agentic(&item.expr) };
                    let net = abl_bridge::decompile(&item.expr, &item.name);
                    if let Some(ag) = &ag {
                        if ag.is_swarm {
                            map.insert("kind".into(), "swarm".into());
                            if let Some(n) = ag.spawn_sym.and_then(sym_name) {
                                map.insert("agent".into(), n.into());
                            }
                            if let Some(sz) = ag.size {
                                map.insert("size".into(), sz.into());
                            }
                            let comm = match (ag.has_send, ag.has_recv) {
                                (true, true) => "send-recv",
                                (true, false) => "send",
                                (false, true) => "recv",
                                (false, false) => "none",
                            };
                            map.insert("comm".into(), comm.into());
                            if let Some(n) = ag.topology_sym.and_then(sym_name) {
                                map.insert("topology".into(), n.into());
                            }
                            if let Some(n) = ag.consensus_sym.and_then(sym_name) {
                                map.insert("consensus".into(), n.into());
                            }
                            if let Some(n) = ag.transport_sym.and_then(sym_name) {
                                map.insert("transport".into(), n.into());
                            }
                        } else {
                            map.insert("kind".into(), "agent".into());
                            let caps: Vec<serde_json::Value> = ag
                                .cap_syms
                                .iter()
                                .filter_map(|&id| sym_name(id))
                                .map(serde_json::Value::from)
                                .collect();
                            map.insert("capabilities".into(), serde_json::Value::Array(caps));
                            let appr: Vec<serde_json::Value> = ag
                                .approval_syms
                                .iter()
                                .filter_map(|&id| sym_name(id))
                                .map(serde_json::Value::from)
                                .collect();
                            map.insert("requires_approval".into(), serde_json::Value::Array(appr));
                        }
                    } else if !is_kb && !net.net.layers.is_empty() {
                        // Neural item: reconstruct the layer chain.
                        let layers: Vec<serde_json::Value> = net
                            .net
                            .layers
                            .iter()
                            .map(|layer| {
                                let op = match &layer.layer_type {
                                    ast::Type::Path { segments, .. } => {
                                        segments.last().cloned().unwrap_or_default()
                                    }
                                    _ => "?".to_string(),
                                };
                                let dims: Vec<i64> = layer
                                    .args
                                    .iter()
                                    .filter_map(|a| match a {
                                        ast::Expr::Literal { value, .. } => value.parse::<i64>().ok(),
                                        _ => None,
                                    })
                                    .collect();
                                serde_json::json!({ "name": layer.name, "op": op, "dims": dims })
                            })
                            .collect();
                        map.insert("kind".into(), "net".into());
                        map.insert("layers".into(), serde_json::Value::Array(layers));
                    } else if !is_kb {
                        map.insert("kind".into(), "unknown".into());
                    } else {
                        // Symbolic (kb) item: recover full facts (predicate + ground
                        // terms) and rule signatures (name + param names).
                        map.insert("kind".into(), "kb".into());
                        let names = |ids: &[u32]| -> Vec<serde_json::Value> {
                            ids.iter().filter_map(|&id| sym_name(id)).map(serde_json::Value::from).collect()
                        };
                        let facts: Vec<serde_json::Value> = sym
                            .fact_syms
                            .iter()
                            .zip(&sym.fact_arg_syms)
                            .map(|(&id, terms)| {
                                let mut f = serde_json::Map::new();
                                if let Some(name) = sym_name(id) { f.insert("name".into(), name.into()); }
                                f.insert("args".into(), serde_json::Value::Array(names(terms)));
                                serde_json::Value::Object(f)
                            })
                            .collect();
                        let rules: Vec<serde_json::Value> = sym
                            .rule_syms
                            .iter()
                            .zip(&sym.rule_param_syms)
                            .zip(&sym.rule_body_syms)
                            .map(|((&id, params), body)| {
                                let mut r = serde_json::Map::new();
                                if let Some(name) = sym_name(id) { r.insert("name".into(), name.into()); }
                                r.insert("params".into(), serde_json::Value::Array(names(params)));
                                let body_json: Vec<serde_json::Value> = body
                                    .iter()
                                    .map(|(p, a)| serde_json::json!({
                                        "pred": sym_name(*p),
                                        "args": names(a),
                                    }))
                                    .collect();
                                r.insert("body".into(), serde_json::Value::Array(body_json));
                                serde_json::Value::Object(r)
                            })
                            .collect();
                        map.insert("facts".into(), serde_json::Value::Array(facts));
                        map.insert("rules".into(), serde_json::Value::Array(rules));
                    }
                    entry
                })
                .collect();
            let out = serde_json::json!({
                "ok": true,
                "container_bytes": blob.len(),
                "exec": false, // load is pure data decode — no code execution
                "items": items_json,
            });
            println!("{}", serde_json::to_string_pretty(&out).unwrap_or_else(|_| "{}".to_string()));
        }
        Err(e) => {
            println!("{}", serde_json::json!({ "ok": false, "error": e }));
            std::process::exit(1);
        }
    }
}

/// Drive `--from=abl-bytes`: read a `.abl` container, decode every item
/// via the RMI codec, decompile each to a MechGen `net`/`kb` declaration via
/// the bridge's existing decompiler, and print the resulting `.mg` source.
fn run_decode_abl_bytes(blob: &[u8], path: &str) {
    let mut pos = 0usize;
    fn take<'a>(buf: &'a [u8], pos: &mut usize, n: usize, what: &str) -> Option<&'a [u8]> {
        if *pos + n > buf.len() {
            eprintln!("{what}: unexpected EOF at offset {}", *pos);
            return None;
        }
        let s = &buf[*pos..*pos + n];
        *pos += n;
        Some(s)
    }
    let magic = match take(blob, &mut pos, 4, "magic") {
        Some(m) => m,
        None => return,
    };
    if magic != ABL_MAGIC {
        eprintln!("{path}: bad magic {:?} (expected Agentic Binary Language)", magic);
        return;
    }
    let ver = u16::from_le_bytes(match take(blob, &mut pos, 2, "version") {
        Some(b) => b.try_into().unwrap(),
        None => return,
    });
    if ver != ABL_VERSION {
        eprintln!("{path}: unsupported version {}", ver);
        return;
    }
    let count = u32::from_le_bytes(match take(blob, &mut pos, 4, "count") {
        Some(b) => b.try_into().unwrap(),
        None => return,
    }) as usize;

    println!("// Agentic Binary Language → MechGen decompiled view of {path}");
    println!("// container: {} bytes, {} item(s)", blob.len(), count);

    for i in 0..count {
        let nl = u32::from_le_bytes(match take(blob, &mut pos, 4, "name_len") {
            Some(b) => b.try_into().unwrap(),
            None => return,
        }) as usize;
        let name = match take(blob, &mut pos, nl, "name") {
            Some(b) => std::str::from_utf8(b).unwrap_or("<bad-utf8>").to_string(),
            None => return,
        };
        let el = u32::from_le_bytes(match take(blob, &mut pos, 4, "expr_len") {
            Some(b) => b.try_into().unwrap(),
            None => return,
        }) as usize;
        let expr_bytes = match take(blob, &mut pos, el, "expr") {
            Some(b) => b,
            None => return,
        };
        match rmi::lang::codec::Decoder::decode_expr_only(expr_bytes) {
            Ok(expr) => {
                // Expand any REPEAT folds back to the flat pipeline the
                // decompiler walks (Seq/Par only).
                let expr = abl_bridge::expand_repeats(&expr);
                let result = abl_bridge::decompile(&expr, &name);
                println!("\n// item {i}: {name} ({} bytes expr)", el);
                let mut layer_lines = Vec::new();
                for layer in &result.net.layers {
                    let type_name = match &layer.layer_type {
                        ast::Type::Path { segments, .. } => {
                            segments.last().cloned().unwrap_or_default()
                        }
                        _ => "?".to_string(),
                    };
                    let args = if layer.args.is_empty() {
                        String::new()
                    } else {
                        let parts: Vec<String> = layer.args.iter().filter_map(|a| match a {
                            ast::Expr::Literal { value, .. } => Some(value.clone()),
                            _ => None,
                        }).collect();
                        if parts.is_empty() { String::new() } else { format!("({})", parts.join(", ")) }
                    };
                    layer_lines.push(format!("    layer {}: {}{};", layer.name, type_name, args));
                }
                if !result.skipped.is_empty() {
                    println!("// skipped (no canonical name): {:?}", result.skipped);
                }
                println!("net {} {{", name);
                for l in &layer_lines {
                    println!("{l}");
                }
                println!("    forward {{ {} }}", result.net.layers.first().map(|l| l.name.as_str()).unwrap_or(""));
                println!("}}");
            }
            Err(e) => eprintln!("item {i}: decode error: {e:?}"),
        }
    }
}

/// Drive `--run=abl-bytes`: decode every item in a Agentic Binary Language container and
/// dispatch it to the CPU backend via `abl_compute::run_pipeline`,
/// completely **skipping the text round-trip**. This is the
/// agent-canonical execution path:
///
/// ```text
///   bytes (.abl) → Decoder → Expr → dispatch → CpuBackend → result
/// ```
///
/// Items that contain neural/symbolic/agent opcodes the dispatcher can
/// run are executed (activation chains, Linear with cached params, etc.);
/// items that lower entirely to stub opcodes are reported as `stub`
/// rather than failing.
fn run_dispatch_abl_bytes(blob: &[u8], path: &str, backend_name: &str) {
    use rmi::compute::cpu::CpuBackend;
    // Resolve agent's --backend=<name> choice into a SelectedBackend.
    // Falls back to CpuBackend on error so the dispatch still happens;
    // we surface the message so an agent knows why their request was
    // downgraded.
    let selected = match crate::backends::select_backend(backend_name) {
        Ok(b) => {
            if b.name() != "cpu" {
                eprintln!("// backend: {}", b.name());
            }
            Some(b)
        }
        Err(e) => {
            eprintln!("// backend selection: {e} - using cpu");
            None
        }
    };
    // CUDA dispatch (P98-P101): `--features cuda` + `--backend=cuda`
    // routes ops through CudaBackend's Backend impl. Per-op routes
    // currently bounce through CpuBackend internally (TODO markers
    // in cuda_backend.rs); replacing each with IA cuBLASLt / NVRTC
    // dispatch is the per-op ratchet.
    #[cfg(feature = "cuda")]
    if let Some(crate::backends::SelectedBackend::Cuda(b)) = &selected {
        eprintln!(
            "// CUDA device acquired (id={}); ops dispatch via Cuda Backend impl",
            b.device_id(),
        );
    }
    // If the selected backend is subprocess-dispatchable (P94), hand
    // off the full Agentic Binary Language blob + path metadata to the wrapper and
    // print whatever it returns. No per-item CPU dispatch below;
    // the wrapper owns the loop.
    if let Some(crate::backends::SelectedBackend::Subprocess { name, command }) = &selected {
        eprintln!("// dispatching via subprocess backend '{name}': {command}");
        match crate::backends::dispatch_via_subprocess(
            name, command, path, &[], blob,
        ) {
            Ok(r) => {
                println!(
                    "// {name}: ok={} dispatched={} output_shape={:?} output_sum={:.4}",
                    r.ok, r.dispatched, r.output_shape, r.output_sum
                );
                if let Some(err) = r.error {
                    eprintln!("// {name}: wrapper reported error: {err}");
                }
            }
            Err(e) => eprintln!("// {name}: subprocess dispatch failed: {e}"),
        }
        return;
    }
    let mut pos = 0usize;
    fn take<'a>(buf: &'a [u8], pos: &mut usize, n: usize, what: &str) -> Option<&'a [u8]> {
        if *pos + n > buf.len() {
            eprintln!("{what}: unexpected EOF at offset {}", *pos);
            return None;
        }
        let s = &buf[*pos..*pos + n];
        *pos += n;
        Some(s)
    }
    let magic = match take(blob, &mut pos, 4, "magic") {
        Some(m) => m,
        None => return,
    };
    if magic != ABL_MAGIC {
        eprintln!("{path}: bad magic {:?} (expected Agentic Binary Language)", magic);
        return;
    }
    let ver = u16::from_le_bytes(match take(blob, &mut pos, 2, "version") {
        Some(b) => b.try_into().unwrap(),
        None => return,
    });
    if ver != ABL_VERSION {
        eprintln!("{path}: unsupported version {}", ver);
        return;
    }
    let count = u32::from_le_bytes(match take(blob, &mut pos, 4, "count") {
        Some(b) => b.try_into().unwrap(),
        None => return,
    }) as usize;

    // P101: dispatch through whichever Backend the agent selected.
    // CPU is the floor; CUDA (when --features cuda + --backend=cuda)
    // routes through CudaBackend's Backend impl - per-op routes
    // currently bounce back to CPU via the impl's `cpu` field, but
    // the dispatch path itself is polymorphic, so each TODO swap in
    // cuda_backend.rs lights up real GPU dispatch with zero changes
    // to main.rs / abl_compute.rs.
    let cpu_backend = CpuBackend::new();
    #[cfg(feature = "cuda")]
    let backend: &dyn rmi::compute::Backend = match &selected {
        Some(crate::backends::SelectedBackend::Cuda(c)) => c,
        _ => &cpu_backend,
    };
    #[cfg(not(feature = "cuda"))]
    let backend: &dyn rmi::compute::Backend = &cpu_backend;
    let backend_name = backend.backend_type();
    println!("// Agentic Binary Language bytes → {backend_name:?} dispatch for {path}");
    println!("// container: {} bytes, {} item(s)", blob.len(), count);

    for i in 0..count {
        let nl = u32::from_le_bytes(match take(blob, &mut pos, 4, "name_len") {
            Some(b) => b.try_into().unwrap(),
            None => return,
        }) as usize;
        let name = match take(blob, &mut pos, nl, "name") {
            Some(b) => std::str::from_utf8(b).unwrap_or("<bad-utf8>").to_string(),
            None => return,
        };
        let el = u32::from_le_bytes(match take(blob, &mut pos, 4, "expr_len") {
            Some(b) => b.try_into().unwrap(),
            None => return,
        }) as usize;
        let expr_bytes = match take(blob, &mut pos, el, "expr") {
            Some(b) => b,
            None => return,
        };
        let expr = match rmi::lang::codec::Decoder::decode_expr_only(expr_bytes) {
            Ok(e) => abl_bridge::expand_repeats(&e), // unfold REPEAT before dispatch
            Err(e) => {
                eprintln!("item {i} ({name}): decode error: {e:?}");
                continue;
            }
        };
        // Diagnose stub-only items (Phase-4 classification) without trying
        // to dispatch — they'd just return unsupported.
        let families = abl_bridge::expr_op_families(&expr);
        let stub_families: Vec<_> = families
            .iter()
            .filter(|f| abl_bridge::is_stubbed_family(**f))
            .filter(|f| !matches!(**f, rmi::lang::OpFamily::Neural))
            .map(|f| format!("{f:?}"))
            .collect();
        if !stub_families.is_empty() && !families.contains(&rmi::lang::OpFamily::Neural) {
            println!(
                "//   item {i}: {name} ({el}B)  stub  families={}  (symbolic/agent — needs distributed runtime)",
                stub_families.join(",")
            );
            continue;
        }

        let shape: Vec<usize> = abl_compute::infer_input_shape(&expr)
            .unwrap_or_else(|| vec![8]);
        match abl_compute::run_pipeline(backend, &expr, &shape, 1.0) {
            Ok(r) => println!(
                "//   item {i}: {name} ({el}B)  dispatched={} unsupported={:?} output_sum={:.4} shape={:?} (input={:?})",
                r.dispatched, r.unsupported, r.output_sum, r.output.shape, shape
            ),
            Err(e) => eprintln!("//   item {i}: {name}: backend error: {e}"),
        }
    }

    // Post-dispatch observability: surface real-GPU op counters so a
    // caller can tell whether the cuBLASLt path was actually exercised
    // vs the CPU fallback. Only prints if there's something to report.
    #[cfg(feature = "cuda")]
    if let Some(crate::backends::SelectedBackend::Cuda(c)) = &selected {
        let mm = c.matmul_gpu_count();
        let ew = c.elementwise_gpu_count();
        if mm > 0 || ew > 0 {
            println!("// gpu_ops: matmul={mm} (cuBLASLt) elementwise={ew} (NVRTC)");
        } else {
            println!(
                "// gpu_ops: matmul=0 elementwise=0 (no GPU path taken — driver missing or all inputs ineligible)"
            );
        }
    }
}

/// Drive `--target=abl-generate`: load checkpoint + prompt, autoregressively
/// generate up to `max_tokens` new tokens via greedy argmax decoding.
///
/// The model is expected to be an LM: input is `[seq]` of integer token ids,
/// output is `[seq, vocab]` logits at each position. Generation takes the
/// last-position logits, argmaxes for the next token, and appends to the
/// running sequence.
fn run_generate(module: &ast::Module, path: &str) {
    use rmi::compute::cpu::CpuBackend;
    use rmi::compute::Backend;
    let backend = CpuBackend::new();

    println!("// MechGen → Agentic Binary Language → autoregressive generation for {path}");

    let mut nets: std::collections::HashMap<&str, &ast::NetDef> = Default::default();
    for item in &module.items {
        if let ast::ItemKind::Net(n) = &item.kind {
            nets.insert(n.name.as_str(), n);
        }
    }

    let mut found_any = false;
    for item in &module.items {
        let train = match &item.kind {
            ast::ItemKind::Train(t) => t,
            _ => continue,
        };
        found_any = true;
        let net = match nets.get(train.net.as_str()) {
            Some(n) => n,
            None => {
                eprintln!("generate `{}`: net `{}` not found", train.name, train.net);
                continue;
            }
        };
        // Load weights if a checkpoint exists; otherwise warn and use fresh.
        let ckpt_path = extract_string_literal(train.checkpoint.as_ref());
        let mut params = match ckpt_path.as_ref().and_then(|p| std::fs::read(p).ok()) {
            Some(blob) => match abl_compute::ParamStore::load(&blob, &backend) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("generate `{}`: load checkpoint: {e}", train.name);
                    continue;
                }
            },
            None => {
                eprintln!("generate `{}`: no checkpoint available — using fresh weights", train.name);
                abl_compute::ParamStore::new()
            }
        };

        let lowered = abl_bridge::NetTranslator::translate(net);

        // Extract prompt: either nested `[[1, 2, 3]]` or flat `[1, 2, 3]`.
        let mut tokens: Vec<usize> = match extract_nested_floats(train.prompt.as_ref()) {
            Some(rows) if !rows.is_empty() => rows[0].iter().map(|v| v.round() as usize).collect(),
            _ => match extract_flat_floats(train.prompt.as_ref()) {
                Some(flat) if !flat.is_empty() => flat.iter().map(|v| v.round() as usize).collect(),
                _ => {
                    eprintln!("generate `{}`: no `prompt:` array of token ids — nothing to seed", train.name);
                    continue;
                }
            },
        };
        let max_new = extract_int_from_expr(train.max_tokens.as_ref()).unwrap_or(16) as usize;
        let temperature = extract_f32_from_expr(train.temperature.as_ref()).unwrap_or(0.0);
        let top_k = extract_int_from_expr(train.top_k.as_ref())
            .map(|n| n.max(0) as usize)
            .unwrap_or(0);
        let top_p = extract_f32_from_expr(train.top_p.as_ref()).unwrap_or(0.0).clamp(0.0, 1.0);
        let mut rng_state: u64 = extract_int_from_expr(train.seed.as_ref())
            .map(|n| n as u64)
            .unwrap_or(0xC0FFEE_5EEDu64);

        // Determine vocab size from the last Linear in the net (output head).
        let vocab = last_linear_out(net).unwrap_or_else(|| {
            // Fallback: try EMBED args (the embedding's vocab matches the head).
            embedding_vocab(net).unwrap_or(0)
        });
        if vocab == 0 {
            eprintln!("generate `{}`: cannot determine vocab size from net — need final Linear", train.name);
            continue;
        }

        let sample_mode = if temperature > 0.0 {
            let mut parts = vec![format!("T={temperature}")];
            if top_k > 0 { parts.push(format!("top_k={top_k}")); }
            if top_p > 0.0 { parts.push(format!("top_p={top_p}")); }
            format!("sampling({})", parts.join(", "))
        } else {
            "argmax".to_string()
        };
        println!(
            "// generate `{}` → net `{}`  prompt={:?} max_tokens={} vocab={} mode={} ckpt={}",
            train.name, train.net, tokens, max_new, vocab, sample_mode,
            ckpt_path.as_deref().unwrap_or("<none>")
        );

        for step in 0..max_new {
            // Run forward on the current sequence.
            let input_f: Vec<f32> = tokens.iter().map(|&t| t as f32).collect();
            let handle = match backend.from_slice_f32(&input_f, &[input_f.len()]) {
                Ok(h) => h,
                Err(e) => {
                    eprintln!("  step {step}: alloc: {e}");
                    break;
                }
            };
            let out_handle = match abl_compute::forward_pass(&backend, &lowered.expr, handle, &mut params) {
                Ok(h) => h,
                Err(e) => {
                    eprintln!("  step {step}: forward: {e}");
                    break;
                }
            };
            let bytes = backend.copy_to_host(&out_handle).unwrap_or_default();
            let logits: Vec<f32> = bytes
                .chunks_exact(4)
                .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                .collect();
            // logits shape: [seq, vocab] (or [seq * vocab] flat). Last-position
            // logits live in the tail.
            if logits.len() < vocab {
                eprintln!("  step {step}: output too small ({}) for vocab {}", logits.len(), vocab);
                break;
            }
            let tail = &logits[logits.len() - vocab..];
            let next = if temperature > 0.0 {
                sample_token(tail, temperature, top_k, top_p, &mut rng_state)
            } else {
                tail.iter()
                    .enumerate()
                    .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
                    .map(|(i, _)| i)
                    .unwrap_or(0)
            };
            tokens.push(next);
        }

        println!("// generated sequence: {:?}", tokens);
    }

    if !found_any {
        eprintln!("// no `train` blocks found in {path}");
    }
}

/// Sample a token index from logits via temperature + top-k + top-p.
///
/// Steps:
/// 1. Scale logits by `1/temperature` (sharpens or flattens).
/// 2. If `top_k > 0`, keep only the k highest logits (others → −∞).
/// 3. If `top_p > 0`, after softmax keep the smallest set whose cumulative
///    probability ≥ p (others zeroed). Top-k is applied first if both set.
/// 4. Re-normalise the surviving probabilities and CDF-walk a uniform draw.
fn sample_token(logits: &[f32], temperature: f32, top_k: usize, top_p: f32, rng_state: &mut u64) -> usize {
    let inv_t = 1.0 / temperature.max(1e-6);
    let mut scaled: Vec<f32> = logits.iter().map(|&l| l * inv_t).collect();

    if top_k > 0 && top_k < scaled.len() {
        let mut sorted = scaled.clone();
        sorted.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
        let threshold = sorted[top_k - 1];
        for v in scaled.iter_mut() {
            if *v < threshold {
                *v = f32::NEG_INFINITY;
            }
        }
    }

    // Softmax with max-subtraction.
    let max = scaled.iter().copied().filter(|v| v.is_finite()).fold(f32::MIN, f32::max);
    let mut probs: Vec<f32> = scaled.iter().map(|&v| {
        if v.is_finite() { (v - max).exp() } else { 0.0 }
    }).collect();
    let sum: f32 = probs.iter().sum();
    if sum <= 0.0 { return 0; }
    for p in probs.iter_mut() { *p /= sum; }

    // Top-p nucleus: walk sorted probs until cumsum ≥ p; zero everything else.
    if top_p > 0.0 && top_p < 1.0 {
        let mut indexed: Vec<(usize, f32)> = probs.iter().copied().enumerate().collect();
        indexed.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let mut keep = std::collections::HashSet::new();
        let mut cum = 0.0f32;
        for (i, p) in &indexed {
            keep.insert(*i);
            cum += *p;
            if cum >= top_p { break; }
        }
        for (i, p) in probs.iter_mut().enumerate() {
            if !keep.contains(&i) {
                *p = 0.0;
            }
        }
        let s: f32 = probs.iter().sum();
        if s > 0.0 {
            for p in probs.iter_mut() { *p /= s; }
        }
    }

    // CDF walk.
    *rng_state = rng_state
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    let u = ((*rng_state >> 33) as u32 as f32) / (u32::MAX as f32);
    let mut acc = 0.0f32;
    for (i, &p) in probs.iter().enumerate() {
        acc += p;
        if acc >= u {
            return i;
        }
    }
    probs.len() - 1
}

/// Extract a flat array literal of floats: `[1.0, 2.0, 3.0]` → vec.
fn extract_flat_floats(expr: Option<&ast::Expr>) -> Option<Vec<f32>> {
    match expr? {
        ast::Expr::ArrayLit { elements } => elements.iter().map(extract_f32_literal).collect(),
        _ => None,
    }
}

/// Find the output dim of the last Linear layer in a net (the LM head).
fn last_linear_out(net: &ast::NetDef) -> Option<usize> {
    for layer in net.layers.iter().rev() {
        let is_linear = matches!(
            &layer.layer_type,
            ast::Type::Path { segments, .. } if segments.last().map(|s| s.as_str()) == Some("Linear")
        );
        if !is_linear {
            continue;
        }
        let dims: Vec<i64> = layer.args.iter().filter_map(|a| match a {
            ast::Expr::Literal { value, kind: ast::LiteralKind::Int } => value.parse().ok(),
            _ => None,
        }).collect();
        if dims.len() >= 2 {
            return Some(dims[1] as usize);
        }
    }
    None
}

/// Find the vocab size from the first Embedding layer.
fn embedding_vocab(net: &ast::NetDef) -> Option<usize> {
    for layer in &net.layers {
        let is_emb = matches!(
            &layer.layer_type,
            ast::Type::Path { segments, .. }
                if matches!(segments.last().map(|s| s.as_str()), Some("Embedding") | Some("Embed"))
        );
        if !is_emb {
            continue;
        }
        let dims: Vec<i64> = layer.args.iter().filter_map(|a| match a {
            ast::Expr::Literal { value, kind: ast::LiteralKind::Int } => value.parse().ok(),
            _ => None,
        }).collect();
        if !dims.is_empty() {
            return Some(dims[0] as usize);
        }
    }
    None
}

/// Drive `--target=abl-infer`: for each `train` block, load its checkpoint,
/// run forward on its `inputs:` data, and print per-sample predictions.
/// Requires both `checkpoint:` and `inputs:` to be set.
fn run_infer(module: &ast::Module, path: &str) {
    use rmi::compute::cpu::CpuBackend;
    use rmi::compute::Backend;
    let backend = CpuBackend::new();

    println!("// MechGen → Agentic Binary Language → CpuBackend inference for {path}");

    let mut nets: std::collections::HashMap<&str, &ast::NetDef> = Default::default();
    for item in &module.items {
        if let ast::ItemKind::Net(n) = &item.kind {
            nets.insert(n.name.as_str(), n);
        }
    }

    let mut found_any = false;
    for item in &module.items {
        let train = match &item.kind {
            ast::ItemKind::Train(t) => t,
            _ => continue,
        };
        found_any = true;
        let net = match nets.get(train.net.as_str()) {
            Some(n) => n,
            None => {
                eprintln!("infer `{}`: net `{}` not found", train.name, train.net);
                continue;
            }
        };
        let ckpt_path = match extract_string_literal(train.checkpoint.as_ref()) {
            Some(p) => p,
            None => {
                eprintln!("infer `{}`: no `checkpoint:` field — nothing to load", train.name);
                continue;
            }
        };
        let blob = match std::fs::read(&ckpt_path) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("infer `{}`: read {ckpt_path}: {e}", train.name);
                continue;
            }
        };
        let mut params = match abl_compute::ParamStore::load(&blob, &backend) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("infer `{}`: load checkpoint: {e}", train.name);
                continue;
            }
        };

        let lowered = abl_bridge::NetTranslator::translate(net);
        let (in_dim, out_dim) = first_last_linear_dims(net).unwrap_or((1, 1));

        let xs = match extract_nested_floats(train.inputs.as_ref()) {
            Some(xs) if !xs.is_empty() => xs,
            _ => {
                eprintln!(
                    "infer `{}`: no `inputs:` array literal — nothing to predict",
                    train.name
                );
                continue;
            }
        };

        println!(
            "// infer `{}` → net `{}`  in_dim={} out_dim={} samples={} checkpoint={} weights={}",
            train.name, train.net, in_dim, out_dim, xs.len(), ckpt_path, params.len()
        );

        for (i, row) in xs.iter().enumerate() {
            if row.len() != in_dim {
                eprintln!("  sample {i}: dim mismatch (have {}, expected {})", row.len(), in_dim);
                continue;
            }
            let h = match backend.from_slice_f32(row, &[1, in_dim]) {
                Ok(h) => h,
                Err(e) => {
                    eprintln!("  sample {i}: alloc: {e}");
                    continue;
                }
            };
            match abl_compute::forward_pass(&backend, &lowered.expr, h, &mut params) {
                Ok(out) => {
                    let bytes = backend.copy_to_host(&out).unwrap_or_default();
                    let preds: Vec<f32> = bytes
                        .chunks_exact(4)
                        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                        .collect();
                    let pred_str = preds.iter().map(|v| format!("{v:.4}")).collect::<Vec<_>>().join(", ");
                    let input_str = row.iter().map(|v| format!("{v:.4}")).collect::<Vec<_>>().join(", ");
                    println!("//   input=[{input_str}] → pred=[{pred_str}]");
                }
                Err(e) => eprintln!("  sample {i}: forward: {e}"),
            }
        }
    }

    if !found_any {
        eprintln!("// no `train` blocks found in {path}");
    }
}

fn run_train(module: &ast::Module, path: &str) {
    use rmi::compute::cpu::CpuBackend;
    let backend = CpuBackend::new();

    println!("// MechGen → Agentic Binary Language → SGD training for {path}");

    // Index nets by name so train blocks can look them up.
    let mut nets: std::collections::HashMap<&str, &ast::NetDef> = Default::default();
    for item in &module.items {
        if let ast::ItemKind::Net(n) = &item.kind {
            nets.insert(n.name.as_str(), n);
        }
    }

    let mut found_any = false;
    for item in &module.items {
        let train = match &item.kind {
            ast::ItemKind::Train(t) => t,
            _ => continue,
        };
        found_any = true;

        let net = match nets.get(train.net.as_str()) {
            Some(n) => n,
            None => {
                eprintln!(
                    "train `{}`: net `{}` not found in module",
                    train.name, train.net
                );
                continue;
            }
        };

        let lowered = abl_bridge::NetTranslator::translate(net);
        if !lowered.unknown_layers.is_empty() {
            eprintln!(
                "train `{}`: net `{}` has unknown layers {:?} — using IDENTITY fallback",
                train.name, train.net, lowered.unknown_layers
            );
        }

        // Determine input / output dims from the net's first and last Linear.
        let (in_dim, out_dim) = first_last_linear_dims(net).unwrap_or((2, 1));
        let epochs = extract_int_from_expr(train.epochs.as_ref()).unwrap_or(50) as usize;
        // Learning rate: prefer `optimizer: SGD(0.01)` if present, else default 0.05.
        let lr = extract_lr_from_optimizer(train.optimizer.as_ref()).unwrap_or(0.05);

        // Dataset selection precedence:
        //   1. inline `inputs:` + `targets:` array literals
        //   2. `dataset:` CSV file path (first in_dim cols = x, last out_dim = y)
        //   3. fallback synthetic y = sum(x)
        let (x, y, batch, dataset_source) = if let (Some(xs), Some(ys)) = (
            extract_nested_floats(train.inputs.as_ref()),
            extract_nested_floats(train.targets.as_ref()),
        ) {
            let bs = xs.len();
            let xs_flat: Vec<f32> = xs.iter().flatten().copied().collect();
            let ys_flat: Vec<f32> = ys.iter().flatten().copied().collect();
            if xs_flat.len() != bs * in_dim || ys_flat.len() != bs * out_dim {
                eprintln!(
                    "train `{}`: dataset dims mismatch — got {}x{} inputs, {}x{} targets; expected {}x{} and {}x{}",
                    train.name, xs.len(), xs_flat.len() / bs.max(1), ys.len(), ys_flat.len() / bs.max(1), bs, in_dim, bs, out_dim
                );
                continue;
            }
            (xs_flat, ys_flat, bs, "inline".to_string())
        } else if let Some(csv_path) = extract_string_literal(train.dataset.as_ref()) {
            match load_csv(&csv_path, in_dim, out_dim) {
                Ok((xs, ys, n)) => (xs, ys, n, format!("csv:{csv_path}")),
                Err(e) => {
                    eprintln!("train `{}`: dataset error: {e}", train.name);
                    continue;
                }
            }
        } else {
            let batch = 4usize;
            let mut x = Vec::with_capacity(batch * in_dim);
            let mut y = Vec::with_capacity(batch * out_dim);
            for b in 0..batch {
                let base = (b + 1) as f32 * 0.25;
                for i in 0..in_dim {
                    x.push(base + (i as f32) * 0.1);
                }
                let target = x[b * in_dim..(b + 1) * in_dim].iter().sum::<f32>();
                for _ in 0..out_dim {
                    y.push(target);
                }
            }
            (x, y, batch, "synthetic".to_string())
        };

        // Optimizer: pick from `optimizer: Adam(...)` vs `SGD(...)`.
        let optim = extract_optimizer(train.optimizer.as_ref());
        // Loss: pick from `loss: CrossEntropy` vs `MSE`.
        let loss_kind = extract_loss(train.loss.as_ref());
        // Optional checkpoint path: load weights if file exists; save after.
        let ckpt_path = extract_string_literal(train.checkpoint.as_ref());
        // Mini-batch size: defaults to full-batch (entire train set per step).
        let batch_size = extract_int_from_expr(train.batch_size.as_ref())
            .map(|n| n.max(1) as usize);
        // Early-stopping patience: 0/None disables.
        let patience = extract_int_from_expr(train.patience.as_ref())
            .map(|n| n.max(0) as usize);
        // Validation split: hold out the last `val_split` fraction.
        let split = extract_f32_from_expr(train.val_split.as_ref()).unwrap_or(0.0).clamp(0.0, 0.99);
        let n_val = ((batch as f32) * split) as usize;
        let n_train = batch.saturating_sub(n_val);
        let train_x = &x[..n_train * in_dim];
        let train_y = &y[..n_train * out_dim];
        let val_x = &x[n_train * in_dim..];
        let val_y = &y[n_train * out_dim..];

        let bs = batch_size.unwrap_or(n_train).min(n_train).max(1);
        let batches_per_epoch = (n_train + bs - 1) / bs;
        let clip = extract_f32_from_expr(train.clip_grad.as_ref()).filter(|v| *v > 0.0);
        let wd = extract_f32_from_expr(train.weight_decay.as_ref()).filter(|v| *v > 0.0);
        let tied = matches!(
            train.tied_embeddings.as_ref(),
            Some(ast::Expr::Literal { value, kind: ast::LiteralKind::Bool }) if value == "true"
        ) || matches!(
            train.tied_embeddings.as_ref(),
            Some(ast::Expr::Ident { name }) if name == "true" || name == "1b"
        );
        let warmup = extract_int_from_expr(train.warmup_steps.as_ref())
            .map(|n| n.max(0) as u64)
            .unwrap_or(0);
        let schedule = match train.lr_schedule.as_ref() {
            Some(ast::Expr::Ident { name }) if name == "cosine" => LrSchedule::Cosine,
            Some(ast::Expr::Ident { name }) if name == "plateau" => LrSchedule::Plateau,
            _ => LrSchedule::None,
        };
        let plateau_pat = extract_int_from_expr(train.plateau_patience.as_ref())
            .map(|n| n.max(1) as usize)
            .unwrap_or(5);
        let lr_factor = extract_f32_from_expr(train.lr_factor.as_ref())
            .filter(|v| *v > 0.0 && *v < 1.0)
            .unwrap_or(0.5);
        let total_steps = (epochs as u64) * (batches_per_epoch as u64);
        println!(
            "// train `{}` → net `{}`  in_dim={} out_dim={} epochs={} lr={} optim={} loss={} dataset={} train={} val={} batch_size={} patience={} clip={} warmup={} sched={}",
            train.name, train.net, in_dim, out_dim, epochs, lr, optim_label(optim),
            loss_label(loss_kind), dataset_source, n_train, n_val, bs,
            patience.map(|p| p.to_string()).unwrap_or_else(|| "off".to_string()),
            clip.map(|c| format!("{c}")).unwrap_or_else(|| "off".to_string()),
            warmup,
            match schedule {
                LrSchedule::Cosine => "cosine",
                LrSchedule::Plateau => "plateau",
                LrSchedule::None => "none",
            },
        );

        // Parameter-count report: walk net.layers, sum weight + bias counts
        // for each known op. Doesn't allocate anything in ParamStore yet
        // (those happen lazily on first forward).
        let (per_layer_params, total_params) = count_params(net);
        if total_params > 0 {
            println!("// train `{}` parameters: {} total", train.name, total_params);
            for (name, count) in &per_layer_params {
                println!("//   {name}: {count}");
            }
        }
        if let Some(wd) = wd {
            println!("// train `{}`: weight_decay={}", train.name, wd);
        }
        if tied {
            println!("// train `{}`: tied_embeddings=true", train.name);
        }
        // Compute tied-weight signature: embedding (V, E) + final Linear (E, V).
        let tied_keys: Option<(Vec<i64>, Vec<i64>)> = if tied {
            tied_weight_keys(net)
        } else {
            None
        };
        if tied && tied_keys.is_none() {
            eprintln!(
                "train `{}`: tied_embeddings requested but model lacks Embedding(V,E) + final Linear(E,V)",
                train.name
            );
        }

        // Plateau-scheduling: tracked externally; the closure below honours
        // warmup + cosine but the plateau schedule is applied to `current_lr`
        // in the epoch loop after each val-loss check.
        let mut current_lr = lr;
        let step_lr = |step_idx: u64, base_lr: f32| -> f32 {
            if warmup > 0 && step_idx < warmup {
                return base_lr * (step_idx as f32 / warmup as f32);
            }
            match schedule {
                LrSchedule::Cosine if total_steps > warmup => {
                    let progress = (step_idx.saturating_sub(warmup)) as f32
                        / (total_steps.saturating_sub(warmup)) as f32;
                    let progress = progress.clamp(0.0, 1.0);
                    0.5 * base_lr * (1.0 + (std::f32::consts::PI * progress).cos())
                }
                _ => base_lr,
            }
        };

        let mut params = match ckpt_path.as_ref().and_then(|p| std::fs::read(p).ok()) {
            Some(blob) => match abl_compute::ParamStore::load(&blob, &backend) {
                Ok(p) => {
                    println!("// train `{}`: loaded {} weight tensors from {}", train.name, p.len(), ckpt_path.as_deref().unwrap());
                    p
                }
                Err(e) => {
                    eprintln!("train `{}`: checkpoint load failed: {e}", train.name);
                    abl_compute::ParamStore::new()
                }
            },
            None => abl_compute::ParamStore::new(),
        };
        let mut state = abl_compute::OptimState::new();
        state.clip_grad = clip;
        state.weight_decay = wd;
        let mut first_loss = None;
        let mut last_loss = 0.0f32;
        let mut last_val = f32::NAN;
        let report_every = (epochs / 5).max(1);
        // Simple LCG for deterministic shuffling.
        let mut rng_state: u64 = 0x9E37_79B9_7F4A_7C15u64.wrapping_add(epochs as u64);
        let mut best_val = f32::INFINITY;
        let mut epochs_since_improvement = 0usize;
        // Plateau tracking is independent of early-stop's counter so a
        // model can use one without enabling the other.
        let mut plateau_best = f32::INFINITY;
        let mut plateau_wait = 0usize;
        let mut early_stopped_at: Option<usize> = None;
        let mut idx: Vec<usize> = (0..n_train).collect();
        let mut batch_x = vec![0.0f32; bs * in_dim];
        let mut batch_y = vec![0.0f32; bs * out_dim];

        'epoch_loop: for step in 0..epochs {
            // Shuffle when mini-batching, leave alone for full-batch.
            if batch_size.is_some() && n_train > 1 {
                for i in (1..n_train).rev() {
                    rng_state = rng_state
                        .wrapping_mul(6364136223846793005)
                        .wrapping_add(1442695040888963407);
                    let j = (rng_state >> 33) as usize % (i + 1);
                    idx.swap(i, j);
                }
            }

            let mut epoch_loss_sum = 0.0f32;
            for batch_i in 0..batches_per_epoch {
                let start = batch_i * bs;
                let end = (start + bs).min(n_train);
                let cur_bs = end - start;
                // Pack the mini-batch into contiguous buffers.
                for (k, &row) in idx[start..end].iter().enumerate() {
                    batch_x[k * in_dim..(k + 1) * in_dim]
                        .copy_from_slice(&train_x[row * in_dim..(row + 1) * in_dim]);
                    batch_y[k * out_dim..(k + 1) * out_dim]
                        .copy_from_slice(&train_y[row * out_dim..(row + 1) * out_dim]);
                }
                // OptimState.step advances inside train_one_step; we use its
                // current value here for the LR schedule. For plateau the
                // base is `current_lr` (mutated by the plateau guard).
                let base = match schedule {
                    LrSchedule::Plateau => current_lr,
                    _ => lr,
                };
                let cur_lr = step_lr(state.step, base);
                let r = match abl_compute::train_one_step_with_optim_loss(
                    &backend,
                    &lowered.expr,
                    &batch_x[..cur_bs * in_dim],
                    &[cur_bs, in_dim],
                    &batch_y[..cur_bs * out_dim],
                    &[cur_bs, out_dim],
                    cur_lr,
                    optim,
                    loss_kind,
                    &mut params,
                    &mut state,
                ) {
                    Ok(r) => r,
                    Err(e) => {
                        eprintln!("train `{}`: epoch {step} batch {batch_i} failed: {e}", train.name);
                        break 'epoch_loop;
                    }
                };
                epoch_loss_sum += r.loss;
                // Tied embeddings: after each batch, copy embedding table
                // (V, E) into the final Linear's weight (E, V) transposed.
                if let Some((emb_key, head_key)) = &tied_keys {
                    if let Err(e) = sync_tied_embedding(&backend, &mut params, emb_key, head_key) {
                        eprintln!("train `{}`: tie-weights: {e}", train.name);
                    }
                }
            }
            let epoch_loss = epoch_loss_sum / batches_per_epoch as f32;
            if first_loss.is_none() {
                first_loss = Some(epoch_loss);
            }
            last_loss = epoch_loss;
            let val_loss = if n_val > 0 {
                compute_val_loss(&backend, &lowered.expr, val_x, in_dim, val_y, out_dim, n_val, &mut params)
                    .unwrap_or(f32::NAN)
            } else {
                f32::NAN
            };
            last_val = val_loss;
            if step == 0 || step == epochs - 1 || step % report_every == 0 {
                if n_val > 0 {
                    println!("//   epoch {step:>4}: train_loss={:.6} val_loss={:.6}", epoch_loss, val_loss);
                } else {
                    println!("//   epoch {step:>4}: loss={:.6}", epoch_loss);
                }
            }
            // Plateau LR schedule: drop current LR when val loss hasn't
            // improved for `plateau_patience` epochs. Independent counter
            // so the plateau schedule works whether or not early-stop is on.
            if matches!(schedule, LrSchedule::Plateau) && n_val > 0 && val_loss.is_finite() {
                if val_loss < plateau_best - 1e-6 {
                    plateau_best = val_loss;
                    plateau_wait = 0;
                } else {
                    plateau_wait += 1;
                    if plateau_wait >= plateau_pat {
                        let new_lr = current_lr * lr_factor;
                        println!(
                            "//   epoch {step}: plateau LR drop {:.6} → {:.6} (no improvement {} epochs)",
                            current_lr, new_lr, plateau_pat
                        );
                        current_lr = new_lr;
                        plateau_wait = 0;
                    }
                }
            }
            // Early stopping: only when val is available + patience set.
            if let (Some(p), true) = (patience, n_val > 0) {
                if p > 0 {
                    if val_loss < best_val - 1e-6 {
                        best_val = val_loss;
                        epochs_since_improvement = 0;
                    } else {
                        epochs_since_improvement += 1;
                        if epochs_since_improvement >= p {
                            early_stopped_at = Some(step);
                            println!("//   epoch {step}: early stop (val didn't improve in {p} epochs; best={best_val:.6})");
                            break;
                        }
                    }
                }
            }
        }
        let first_loss = first_loss.unwrap_or(f32::NAN);
        let delta = first_loss - last_loss;
        let pct = if first_loss > 1e-9 { delta / first_loss * 100.0 } else { 0.0 };
        let stop_note = match early_stopped_at {
            Some(e) => format!(" early_stop@{e}"),
            None => String::new(),
        };
        if n_val > 0 {
            println!(
                "// train `{}` done: first_loss={:.6} last_loss={:.6} reduction={:.2}% final_val={:.6}{}",
                train.name, first_loss, last_loss, pct, last_val, stop_note
            );
        } else {
            println!(
                "// train `{}` done: first_loss={:.6} last_loss={:.6} reduction={:.2}%{}",
                train.name, first_loss, last_loss, pct, stop_note
            );
        }

        // Persist trained weights if requested.
        if let Some(path) = &ckpt_path {
            match params.save(&backend) {
                Ok(blob) => match std::fs::write(path, &blob) {
                    Ok(()) => println!("// train `{}`: saved {} weight tensors to {} ({} bytes)", train.name, params.len(), path, blob.len()),
                    Err(e) => eprintln!("train `{}`: write checkpoint {path}: {e}", train.name),
                },
                Err(e) => eprintln!("train `{}`: serialize checkpoint: {e}", train.name),
            }
        }
    }

    if !found_any {
        eprintln!("// no `train` blocks found in {path}");
    }
}

fn first_last_linear_dims(net: &ast::NetDef) -> Option<(usize, usize)> {
    let mut first_in: Option<usize> = None;
    let mut last_out: Option<usize> = None;
    for layer in &net.layers {
        let name = match &layer.layer_type {
            ast::Type::Path { segments, .. } => segments.last().map(|s| s.as_str()),
            _ => None,
        };
        let dims: Vec<i64> = layer
            .args
            .iter()
            .filter_map(|a| match a {
                ast::Expr::Literal { value, kind: ast::LiteralKind::Int } => value.parse().ok(),
                _ => None,
            })
            .collect();
        match name {
            // Embedding(vocab, embed) at the head means the actual model
            // input is a sequence of indices, in-dim = 1 per token.
            Some("Embedding") | Some("Embed") if first_in.is_none() => {
                first_in = Some(1);
            }
            // Linear(in, out [, bias]) is both input candidate and output.
            Some("Linear") if dims.len() >= 2 => {
                if first_in.is_none() {
                    first_in = Some(dims[0] as usize);
                }
                last_out = Some(dims[1] as usize);
            }
            _ => {}
        }
    }
    match (first_in, last_out) {
        (Some(a), Some(b)) => Some((a, b)),
        _ => None,
    }
}

fn extract_int_from_expr(expr: Option<&ast::Expr>) -> Option<i64> {
    match expr? {
        ast::Expr::Literal { value, kind: ast::LiteralKind::Int } => value.parse().ok(),
        _ => None,
    }
}

/// Extract a learning rate from an `optimizer: ...` expression.
///
/// Recognises `SGD(0.01)`, `Adam(0.001)`, etc. — pulls the first
/// floating-point arg. Falls back to `None` if the optimizer carries no
/// numeric arg.
/// Extract an array-of-arrays float literal from a MechGen Expr.
/// Recognises `[[1.0, 2.0], [3.0, 4.0]]`-style ArrayLit nesting.
fn extract_nested_floats(expr: Option<&ast::Expr>) -> Option<Vec<Vec<f32>>> {
    let outer = match expr? {
        ast::Expr::ArrayLit { elements } => elements,
        _ => return None,
    };
    let mut rows = Vec::with_capacity(outer.len());
    for row_expr in outer {
        let row = match row_expr {
            ast::Expr::ArrayLit { elements } => elements,
            _ => return None,
        };
        let mut row_vals = Vec::with_capacity(row.len());
        for v in row {
            row_vals.push(extract_f32_literal(v)?);
        }
        rows.push(row_vals);
    }
    Some(rows)
}

fn extract_f32_literal(expr: &ast::Expr) -> Option<f32> {
    match expr {
        ast::Expr::Literal { value, kind: ast::LiteralKind::Float } => value.parse().ok(),
        ast::Expr::Literal { value, kind: ast::LiteralKind::Int } => value.parse().ok(),
        ast::Expr::Unary { op, operand } if op == "-" => extract_f32_literal(operand).map(|v| -v),
        _ => None,
    }
}

/// Extract a string literal from an Expr. Handles plain string literals.
fn extract_string_literal(expr: Option<&ast::Expr>) -> Option<String> {
    match expr? {
        ast::Expr::Literal { value, kind: ast::LiteralKind::String } => {
            // Surface literals include their quotes; strip them.
            let v = value.trim_matches('"').to_string();
            Some(v)
        }
        _ => None,
    }
}

/// Load a CSV dataset. Skips empty lines and any line starting with `#`.
/// Each remaining row must have exactly `in_dim + out_dim` comma-separated
/// floats. Returns `(inputs_flat, targets_flat, n_rows)`.
fn load_csv(path: &str, in_dim: usize, out_dim: usize) -> Result<(Vec<f32>, Vec<f32>, usize), String> {
    let content = std::fs::read_to_string(path).map_err(|e| format!("read {path}: {e}"))?;
    let want = in_dim + out_dim;
    let mut xs = Vec::new();
    let mut ys = Vec::new();
    let mut n = 0usize;
    for (lineno, line) in content.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let cols: Vec<&str> = line.split(',').map(|c| c.trim()).collect();
        if cols.len() != want {
            return Err(format!(
                "{path}:{}: expected {} columns ({} inputs + {} targets), got {}",
                lineno + 1, want, in_dim, out_dim, cols.len()
            ));
        }
        for (i, c) in cols.iter().enumerate() {
            let v: f32 = c.parse().map_err(|e| {
                format!("{path}:{}: column {}: parse error: {e}", lineno + 1, i + 1)
            })?;
            if i < in_dim {
                xs.push(v);
            } else {
                ys.push(v);
            }
        }
        n += 1;
    }
    if n == 0 {
        return Err(format!("{path}: empty dataset"));
    }
    Ok((xs, ys, n))
}

/// Decide which [`abl_compute::Optimizer`] the `optimizer:` field requests.
///
/// `SGD` / `SGD(lr)` → `Optimizer::Sgd`. `Adam` / `Adam(lr)` → Adam with
/// default hyperparameters. Defaults to SGD when absent.
fn extract_optimizer(expr: Option<&ast::Expr>) -> abl_compute::Optimizer {
    let name = match expr {
        Some(ast::Expr::Call { func, .. }) => match func.as_ref() {
            ast::Expr::Ident { name } => name.as_str(),
            _ => return abl_compute::Optimizer::Sgd,
        },
        Some(ast::Expr::Ident { name }) => name.as_str(),
        _ => return abl_compute::Optimizer::Sgd,
    };
    match name {
        "Adam" | "ADAM" | "adam" => abl_compute::Optimizer::adam_default(),
        _ => abl_compute::Optimizer::Sgd,
    }
}

/// Pick a [`abl_compute::Loss`] from a `loss:` expression. Recognises
/// `CrossEntropy`, `CE`, otherwise defaults to MSE.
fn extract_loss(expr: Option<&ast::Expr>) -> abl_compute::Loss {
    let name = match expr {
        Some(ast::Expr::Ident { name }) => name.as_str(),
        Some(ast::Expr::Call { func, .. }) => match func.as_ref() {
            ast::Expr::Ident { name } => name.as_str(),
            _ => return abl_compute::Loss::Mse,
        },
        _ => return abl_compute::Loss::Mse,
    };
    match name {
        "CrossEntropy" | "CE" | "cross_entropy" => abl_compute::Loss::CrossEntropy,
        _ => abl_compute::Loss::Mse,
    }
}

/// Estimate trainable parameter counts per layer + total.
///
/// Walks `net.layers`, inspects each layer's type and int args, and computes
/// the same `[weight] + [bias?]` count that the dispatcher would allocate.
/// Returns `(per_layer_named_counts, total)`. Unknown/parameterless layers
/// contribute 0.
fn count_params(net: &ast::NetDef) -> (Vec<(String, usize)>, usize) {
    let mut entries = Vec::new();
    let mut total = 0usize;
    for layer in &net.layers {
        let name = match &layer.layer_type {
            ast::Type::Path { segments, .. } => segments.last().map(|s| s.as_str()).unwrap_or(""),
            _ => "",
        };
        let dims: Vec<i64> = layer.args.iter().filter_map(|a| match a {
            ast::Expr::Literal { value, kind: ast::LiteralKind::Int } => value.parse().ok(),
            _ => None,
        }).collect();
        let count = match (name, dims.as_slice()) {
            ("Linear", [a, b]) => (*a as usize) * (*b as usize),
            ("Linear", [a, b, bias]) => {
                let w = (*a as usize) * (*b as usize);
                w + if *bias != 0 { *b as usize } else { 0 }
            }
            ("Conv2D", [ic, oc, k]) => (*oc as usize) * (*ic as usize) * (*k as usize) * (*k as usize),
            ("Conv2D", [ic, oc, k, bias]) => {
                let w = (*oc as usize) * (*ic as usize) * (*k as usize) * (*k as usize);
                w + if *bias != 0 { *oc as usize } else { 0 }
            }
            ("Embedding", [v, e]) | ("Embed", [v, e]) => (*v as usize) * (*e as usize),
            ("LearnedPE", [m, e]) | ("LearnedPositionalEmbedding", [m, e]) => (*m as usize) * (*e as usize),
            ("Attention", [_in, model]) => 4 * (*model as usize) * (*model as usize), // rough: Q/K/V/O each [in, model]
            ("Attention", [in_d, model, _h]) => 4 * (*in_d as usize) * (*model as usize),
            ("Attention", [in_d, model, _h, _c]) => 4 * (*in_d as usize) * (*model as usize),
            ("LayerNorm", [d]) => 2 * (*d as usize), // γ + β
            _ => 0,
        };
        if count > 0 {
            entries.push((format!("{} {}", layer.name, name), count));
            total += count;
        }
    }
    (entries, total)
}

/// Locate the (Embedding key, final-Linear-head key) pair for tied weights.
///
/// Returns `Some(embedding_key, head_key)` only when the net has both an
/// `Embedding(V, E)` layer and a trailing `Linear(E, V[, bias])` layer with
/// matching `V` and `E`. The keys match the ParamStore-canonical forms used
/// in `dispatch_embed` and the Linear dispatch path.
fn tied_weight_keys(net: &ast::NetDef) -> Option<(Vec<i64>, Vec<i64>)> {
    let mut embedding: Option<(i64, i64)> = None;
    let mut last_linear: Option<(i64, i64)> = None;
    for layer in &net.layers {
        let name = match &layer.layer_type {
            ast::Type::Path { segments, .. } => segments.last().map(|s| s.as_str()),
            _ => None,
        };
        let dims: Vec<i64> = layer.args.iter().filter_map(|a| match a {
            ast::Expr::Literal { value, kind: ast::LiteralKind::Int } => value.parse().ok(),
            _ => None,
        }).collect();
        match name {
            Some("Embedding") | Some("Embed") if dims.len() >= 2 && embedding.is_none() => {
                embedding = Some((dims[0], dims[1]));
            }
            Some("Linear") if dims.len() >= 2 => {
                last_linear = Some((dims[0], dims[1]));
            }
            _ => {}
        }
    }
    match (embedding, last_linear) {
        (Some((v, e)), Some((le, lv))) if v == lv && e == le => {
            Some((vec![v, e], vec![e, v]))
        }
        _ => None,
    }
}

/// Copy the embedding table (`[V, E]`) into the head Linear's weight
/// (`[E, V]`) by transposing rows ↔ columns.
fn sync_tied_embedding(
    backend: &rmi::compute::cpu::CpuBackend,
    params: &mut abl_compute::ParamStore,
    emb_key: &[i64],
    head_key: &[i64],
) -> Result<(), String> {
    use rmi::compute::Backend;
    let v = emb_key[0] as usize;
    let e = emb_key[1] as usize;
    let emb_handle = params
        .get_handle(rmi::lang::Op::EMBED, emb_key)
        .ok_or_else(|| format!("embedding key {:?} not in ParamStore", emb_key))?;
    let emb_bytes = backend.copy_to_host(&emb_handle).map_err(|e| e.to_string())?;
    let emb: Vec<f32> = emb_bytes
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();
    if emb.len() != v * e {
        return Err(format!(
            "tied: embedding has {} f32s, expected {} × {} = {}",
            emb.len(), v, e, v * e
        ));
    }
    // Transpose: head[i*v + j] = emb[j*e + i]
    let mut head = vec![0.0f32; e * v];
    for i in 0..e {
        for j in 0..v {
            head[i * v + j] = emb[j * e + i];
        }
    }
    let new_h = backend.from_slice_f32(&head, &[e, v]).map_err(|e| e.to_string())?;
    params.replace_public(rmi::lang::Op::LINEAR, head_key, new_h);
    Ok(())
}

#[derive(Clone, Copy)]
enum LrSchedule {
    None,
    Cosine,
    Plateau,
}

fn loss_label(loss: abl_compute::Loss) -> &'static str {
    match loss {
        abl_compute::Loss::Mse => "MSE",
        abl_compute::Loss::CrossEntropy => "CrossEntropy",
    }
}

fn optim_label(opt: abl_compute::Optimizer) -> &'static str {
    match opt {
        abl_compute::Optimizer::Sgd => "SGD",
        abl_compute::Optimizer::Adam { .. } => "Adam",
    }
}

/// Compute MSE loss on a held-out validation set without updating weights.
fn compute_val_loss(
    backend: &rmi::compute::cpu::CpuBackend,
    expr: &rmi::lang::Expr,
    val_x: &[f32],
    in_dim: usize,
    val_y: &[f32],
    out_dim: usize,
    n_val: usize,
    params: &mut abl_compute::ParamStore,
) -> Option<f32> {
    use rmi::compute::Backend;
    // When in_dim=1 the model is index-driven (Embedding head). Avoid
    // shaping val_x as [n_val, 1] which would feed a 2-D handle into
    // `dispatch_embed` and inflate the output shape with a stray dim.
    let shape: Vec<usize> = if in_dim == 1 {
        vec![n_val]
    } else {
        vec![n_val, in_dim]
    };
    let handle = backend.from_slice_f32(val_x, &shape).ok()?;
    let out_handle = abl_compute::forward_pass(backend, expr, handle, params).ok()?;
    let out_bytes = backend.copy_to_host(&out_handle).ok()?;
    let pred: Vec<f32> = out_bytes
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();
    let want = n_val * out_dim;
    if pred.len() != want || val_y.len() != want {
        return None;
    }
    let mse: f32 = pred
        .iter()
        .zip(val_y.iter())
        .map(|(p, t)| (p - t).powi(2))
        .sum::<f32>()
        / (want as f32);
    Some(mse)
}

fn extract_f32_from_expr(expr: Option<&ast::Expr>) -> Option<f32> {
    match expr? {
        ast::Expr::Literal { value, kind: ast::LiteralKind::Float } => value.parse().ok(),
        ast::Expr::Literal { value, kind: ast::LiteralKind::Int } => value.parse().ok(),
        _ => None,
    }
}

fn extract_lr_from_optimizer(expr: Option<&ast::Expr>) -> Option<f32> {
    let call = match expr? {
        ast::Expr::Call { args, .. } => args,
        _ => return None,
    };
    for arg in call {
        match arg {
            ast::Expr::Literal { value, kind: ast::LiteralKind::Float } => {
                if let Ok(n) = value.parse::<f32>() {
                    return Some(n);
                }
            }
            ast::Expr::Literal { value, kind: ast::LiteralKind::Int } => {
                if let Ok(n) = value.parse::<f32>() {
                    return Some(n);
                }
            }
            _ => {}
        }
    }
    None
}

fn run_parse(source: &str, filename: &str, do_elision: bool, legacy: bool, token_report: bool) {
    let source = if legacy {
        legacy::translate(source)
    } else {
        source.to_string()
    };
    let tokens = lexer::lex(&source);

    let mut error_count = 0;
    for tok in &tokens {
        if tok.kind == lexer::TokenKind::Error {
            eprintln!(
                "{filename}:{}:{}: lexer error: unexpected character",
                tok.span.line, tok.span.col
            );
            error_count += 1;
        }
    }

    match parser::parse(&tokens) {
        Ok(module) => {
            let module = if do_elision {
                elision::elide(&module)
            } else {
                module
            };
            if token_report {
                let report = token_budget::report(&module);
                eprintln!("{}", report.display());
                println!("{}", serde_json::to_string_pretty(&report).unwrap());
            } else {
                println!("{}", serde_json::to_string_pretty(&module).unwrap());
            }
        }
        Err(e) => {
            eprintln!(
                "{filename}:{}:{}: parse error: {}",
                e.line, e.col, e.message
            );
            std::process::exit(1);
        }
    }

    if error_count > 0 {
        std::process::exit(1);
    }
}

/// Machine-readable diagnostic stream for `--check --json`.
///
/// Deterministic by construction: diagnostics are sorted by (line, col, code,
/// message) and the schema is stable, so identical input yields byte-identical
/// output and an agent parses errors structurally — `{code, severity, line,
/// col, category, message, fix}` — instead of scraping human prose. Every
/// diagnostic carries a stable error code and an actionable `fix` hint.
fn run_check_json(source: &str, filename: &str, do_elision: bool, legacy: bool) {
    use hir::Severity;

    let source = if legacy { legacy::translate(source) } else { source.to_string() };

    let mut diags: Vec<serde_json::Value> = Vec::new();
    let mut push = |sev: &str, line: u32, col: u32, code: &str, cat: &str, msg: String, fix: &str| {
        diags.push(serde_json::json!({
            "severity": sev,
            "line": line,
            "col": col,
            "code": code,
            "category": cat,
            "message": msg,
            "fix": fix,
        }));
    };

    let tokens = lexer::lex(&source);
    for tok in &tokens {
        if tok.kind == lexer::TokenKind::Error {
            push(
                "error",
                tok.span.line as u32,
                tok.span.col as u32,
                "E0000",
                "LexError",
                "unexpected character".to_string(),
                "remove or correct the invalid token",
            );
        }
    }

    let parsed = parser::parse(&tokens);
    let module = match parsed {
        Ok(m) => Some(m),
        Err(e) => {
            let cat = hir::DiagnosticCategory::SyntaxError;
            push(
                "error",
                e.line as u32,
                e.col as u32,
                cat.code(),
                "SyntaxError",
                e.message.clone(),
                cat.fix_hint(),
            );
            None
        }
    };

    // Effect (capability) surface, populated when the module parses.
    let mut effect_surface: Vec<serde_json::Value> = Vec::new();
    if let Some(module) = module {
        let module = if do_elision { elision::elide(&module) } else { module };
        let mut all: Vec<hir::Diagnostic> = Vec::new();
        all.extend(resolve::resolve(&module).diagnostics);
        all.extend(types::check(&module).diagnostics);
        let effect_infer = effects::infer_effects(&module);
        all.extend(effect_infer.diagnostics.clone());
        for (func, declared, inferred) in effect_infer.effect_surface() {
            effect_surface.push(serde_json::json!({
                "function": func,
                "declared": declared,
                "inferred": inferred,
            }));
        }
        for d in &all {
            let cat = d.category.unwrap_or(hir::DiagnosticCategory::Other);
            let (line, col) = d.span.map(|s| (s.line, s.col)).unwrap_or((0, 0));
            let sev = match d.severity {
                Severity::Error => "error",
                Severity::Warning => "warning",
                Severity::Info => "note",
            };
            let code = d.id.clone().unwrap_or_else(|| cat.code().to_string());
            push(sev, line, col, &code, &format!("{cat:?}"), d.message.clone(), cat.fix_hint());
        }

        // Typed-composition gate (§4.5): reject shape-mismatched net compositions.
        for d in &abl_shape::check_module_shapes(&module) {
            push(
                "error",
                0,
                0,
                "E0710",
                "ShapeError",
                d.message.clone(),
                "align the layer dims so each layer's output dim matches the next layer's input dim",
            );
        }
    }

    // Deterministic order so output is cacheable/diffable across runs.
    diags.sort_by(|a, b| {
        let key = |v: &serde_json::Value| {
            (
                v["line"].as_u64().unwrap_or(0),
                v["col"].as_u64().unwrap_or(0),
                v["code"].as_str().unwrap_or("").to_string(),
                v["message"].as_str().unwrap_or("").to_string(),
            )
        };
        key(a).cmp(&key(b))
    });

    let error_count = diags.iter().filter(|d| d["severity"] == "error").count();
    let out = serde_json::json!({
        "ok": error_count == 0,
        "file": filename,
        "error_count": error_count,
        "diagnostics": diags,
        // Capability surface: per-function declared vs inferred effects, so an
        // agent runtime can gate/sandbox generated code before running it.
        "effects": effect_surface,
    });
    println!("{}", serde_json::to_string_pretty(&out).unwrap());
    if error_count > 0 {
        std::process::exit(1);
    }
}

fn run_check(source: &str, filename: &str, do_elision: bool, legacy: bool, token_report: bool) {
    // Phase 0: Legacy syntax translation (if active).
    let source = if legacy {
        legacy::translate(source)
    } else {
        source.to_string()
    };

    // Phase 1: Lex.
    let tokens = lexer::lex(&source);
    let mut total_errors = 0;

    for tok in &tokens {
        if tok.kind == lexer::TokenKind::Error {
            eprintln!(
                "{filename}:{}:{}: lexer error: unexpected character",
                tok.span.line, tok.span.col
            );
            total_errors += 1;
        }
    }

    // Phase 2: Parse.
    let module = match parser::parse(&tokens) {
        Ok(m) => m,
        Err(e) => {
            eprintln!(
                "{filename}:{}:{}: parse error: {}",
                e.line, e.col, e.message
            );
            std::process::exit(1);
        }
    };

    // Phase 2.5: Safety elision (agentic mode default).
    let module = if do_elision {
        elision::elide(&module)
    } else {
        module
    };

    // Phase 3: Name resolution.
    let resolver = resolve::resolve(&module);
    for diag in &resolver.diagnostics {
        eprintln!("{filename}: {diag}");
        if diag.severity == hir::Severity::Error {
            total_errors += 1;
        }
    }

    // Phase 4: Type checking.
    let checker = types::check(&module);
    for diag in &checker.diagnostics {
        eprintln!("{filename}: {diag}");
        if diag.severity == hir::Severity::Error {
            total_errors += 1;
        }
    }

    // Phase 5: Effect inference.
    let effect_infer = effects::infer_effects(&module);
    for diag in &effect_infer.diagnostics {
        eprintln!("{filename}: {diag}");
        if diag.severity == hir::Severity::Error {
            total_errors += 1;
        }
    }

    // Report.
    let sym_count = resolver.symbols.len();
    let fn_count = effect_infer.inferred.len();

    // Phase 5.5: Contract verification.
    let verifications = verify::verify_module(&module);
    let contract_count = verifications.len();
    let verified_count = verifications
        .iter()
        .filter(|v| v.status == verify::VerifyStatus::Verified)
        .count();
    let failed_count = verifications
        .iter()
        .filter(|v| v.status == verify::VerifyStatus::Failed)
        .count();

    // Phase 5.6: Typed-composition gate — a shape-mismatched `net` composition
    // (`stack`/`residual`/`branch`/`wrap` whose layer dims don't line up) is
    // rejected here with an actionable diagnostic (§4.5).
    let shape_diags = abl_shape::check_module_shapes(&module);
    for d in &shape_diags {
        eprintln!("{filename}: error: {}", d.message);
        total_errors += 1;
    }

    // Phase 6: Self-healing — generate fix candidates for all diagnostics.
    let mut all_diagnostics: Vec<hir::Diagnostic> = Vec::new();
    all_diagnostics.extend(resolver.diagnostics.iter().cloned());
    all_diagnostics.extend(checker.diagnostics.iter().cloned());
    all_diagnostics.extend(effect_infer.diagnostics.iter().cloned());

    let healed = heal::heal(&all_diagnostics);
    let fix_count: usize = healed.iter().map(|h| h.fixes.len()).sum();

    eprintln!();
    eprintln!("=== Analysis Summary ===");
    eprintln!("  Symbols resolved: {sym_count}");
    eprintln!("  Functions analyzed: {fn_count}");

    // Print effect annotations.
    for (name, effects) in &effect_infer.inferred {
        if effects.is_empty() {
            eprintln!("  f {name}: pure");
        } else {
            let fx: Vec<String> = effects.iter().map(|e| e.to_string()).collect();
            eprintln!("  f {name}: {{ {} }}", fx.join(", "));
        }
    }

    eprintln!("  Errors: {total_errors}");

    // Contract verification report.
    if contract_count > 0 {
        eprintln!(
            "  Contracts checked: {contract_count} (verified: {verified_count}, failed: {failed_count})"
        );
        for v in &verifications {
            let symbol = match v.status {
                verify::VerifyStatus::Verified => "✓",
                verify::VerifyStatus::Partial => "~",
                verify::VerifyStatus::Failed => "✗",
                verify::VerifyStatus::Trivial => "-",
            };
            if v.status != verify::VerifyStatus::Trivial {
                eprintln!("    {symbol} {}: {:?}", v.fqn, v.status);
            }
        }
    }

    if fix_count > 0 {
        eprintln!("  Fix candidates: {fix_count}");
        for h in &healed {
            if !h.fixes.is_empty() {
                eprintln!("    ▸ {}: {} fix(es)", h.diagnostic.message, h.fixes.len());
                for fix in &h.fixes {
                    eprintln!(
                        "      - [conf={:.0}%] {}",
                        fix.confidence * 100.0,
                        fix.description
                    );
                }
            }
        }
    }

    // Token budget report.
    if token_report {
        let report = token_budget::report(&module);
        eprintln!("{}", report.display());
        println!("{}", serde_json::to_string_pretty(&report).unwrap());
    }

    if total_errors > 0 {
        std::process::exit(1);
    } else {
        eprintln!("  Status: OK");
    }
}

fn run_pipeline(source: &str, filename: &str, do_elision: bool, legacy: bool, token_report: bool) {
    eprintln!("╔══════════════════════════════════════════════════════════════╗");
    eprintln!("║  MechGen End-to-End Pipeline                                  ║");
    eprintln!("╚══════════════════════════════════════════════════════════════╝");
    eprintln!();

    let mut total_errors = 0;

    // ── Phase 0: Legacy syntax translation ───────────────────────────
    let source = if legacy {
        eprintln!("▸ Phase 0: Legacy syntax translation (Rust → MechGen)");
        let translated = legacy::translate(source);
        eprintln!("  ✓ translated to canonical syntax");
        translated
    } else {
        source.to_string()
    };

    // ── Phase 1: Lex ─────────────────────────────────────────────────
    eprintln!("▸ Phase 1/7: Lexical analysis");
    let tokens = lexer::lex(&source);
    let mut lex_errors = 0;
    for tok in &tokens {
        if tok.kind == lexer::TokenKind::Error {
            eprintln!(
                "  {filename}:{}:{}: lexer error",
                tok.span.line, tok.span.col
            );
            lex_errors += 1;
        }
    }
    let token_count = tokens.len();
    eprintln!("  ✓ {token_count} tokens, {lex_errors} errors");
    total_errors += lex_errors;

    // ── Phase 2: Parse ───────────────────────────────────────────────
    eprintln!("▸ Phase 2/7: Parsing");
    let module = match parser::parse(&tokens) {
        Ok(m) => {
            eprintln!("  ✓ {} top-level items", m.items.len());
            m
        }
        Err(e) => {
            eprintln!("  ✗ parse error at {}:{}: {}", e.line, e.col, e.message);
            std::process::exit(1);
        }
    };

    // ── Phase 2.5: Safety elision ────────────────────────────────────
    let module = if do_elision {
        eprintln!("▸ Phase 2.5: Safety elision (agentic mode)");
        let elided = elision::elide(&module);
        eprintln!("  ✓ safety annotations stripped");
        elided
    } else {
        eprintln!("▸ Phase 2.5: Safety elision — SKIPPED (--no-elision)");
        module
    };

    // ── Phase 3: Name resolution ─────────────────────────────────────
    eprintln!("▸ Phase 3/7: Name resolution");
    let resolver = resolve::resolve(&module);
    for diag in &resolver.diagnostics {
        eprintln!("  {filename}: {diag}");
        if diag.severity == hir::Severity::Error {
            total_errors += 1;
        }
    }
    eprintln!("  ✓ {} symbols resolved", resolver.symbols.len());

    // ── Phase 4: Type checking ───────────────────────────────────────
    eprintln!("▸ Phase 4/7: Type checking");
    let checker = types::check(&module);
    for diag in &checker.diagnostics {
        eprintln!("  {filename}: {diag}");
        if diag.severity == hir::Severity::Error {
            total_errors += 1;
        }
    }
    eprintln!("  ✓ {} type diagnostics", checker.diagnostics.len());

    // ── Phase 4.5: AI subsystem compilation ──────────────────────────
    eprintln!("▸ Phase 4.5: AI subsystem compilation");
    let mut ai_errors = 0;
    let mut ai_info: Vec<String> = Vec::new();

    // Shape inference on net blocks.
    for item in &module.items {
        if let ast::ItemKind::Net(net) = &item.kind {
            let mut infer = shape::ShapeInfer::new();
            // Use a fresh variable as placeholder input shape.
            let input_shape = vec![infer.fresh_dim(), infer.fresh_dim()];
            let out_shape = infer.infer_net(net, &input_shape);
            for diag in &infer.diagnostics {
                eprintln!("  {filename}: {diag}");
                if diag.severity == hir::Severity::Error {
                    ai_errors += 1;
                }
            }
            let resolved = infer.resolve_shape(&out_shape);
            let dims: Vec<String> = resolved.iter().map(|d| format!("{d:?}")).collect();
            ai_info.push(format!(
                "net {}: output shape [{}]",
                net.name,
                dims.join(", ")
            ));
        }
    }

    // Autograd on train blocks.
    for item in &module.items {
        if let ast::ItemKind::Train(train) = &item.kind {
            let tape = autograd::build_tape_from_train(train);
            let loss_id = if tape.nodes.is_empty() {
                0
            } else {
                tape.nodes.len() - 1
            };
            let param_names: Vec<String> = tape
                .nodes
                .iter()
                .filter_map(|n| {
                    if let autograd::GradOp::Param(name) = &n.op {
                        Some(name.clone())
                    } else {
                        None
                    }
                })
                .collect();
            let grad_result = autograd::backward(&tape, loss_id, &param_names);
            ai_info.push(format!(
                "train {}: {} forward ops, {} backward ops",
                train.name,
                tape.nodes.len(),
                grad_result.mlir_ops.len()
            ));
            for diag in &grad_result.diagnostics {
                eprintln!("  {filename}: {diag}");
                if diag.severity == hir::Severity::Error {
                    ai_errors += 1;
                }
            }
        }
    }

    // Logic engine on kb blocks.
    for item in &module.items {
        if let ast::ItemKind::Kb(kb_def) = &item.kind {
            let mut kb = logic::build_kb(kb_def);
            kb.materialize();
            for diag in &kb.diagnostics {
                eprintln!("  {filename}: {diag}");
                if diag.severity == hir::Severity::Error {
                    ai_errors += 1;
                }
            }
            ai_info.push(format!(
                "kb {}: {} facts materialized",
                kb.name,
                kb.fact_count()
            ));
        }
    }

    // Evolve codegen on evolve blocks.
    for item in &module.items {
        if let ast::ItemKind::Evolve(evolve_def) = &item.kind {
            match evolve_gen::build_evolve_plan(evolve_def) {
                Ok(plan) => {
                    let mlir_ops = plan.emit_mlir();
                    ai_info.push(format!(
                        "evolve {}: pop={}, gen={}, {} MLIR ops",
                        plan.name,
                        plan.population_size,
                        plan.generations,
                        mlir_ops.len()
                    ));
                }
                Err(diags) => {
                    for diag in &diags {
                        eprintln!("  {filename}: {diag}");
                        if diag.severity == hir::Severity::Error {
                            ai_errors += 1;
                        }
                    }
                }
            }
        }
    }

    for info in &ai_info {
        eprintln!("  ✓ {info}");
    }
    if ai_info.is_empty() {
        eprintln!("  - no AI subsystem blocks");
    }
    total_errors += ai_errors;

    // ── Phase 5: Effect inference ────────────────────────────────────
    eprintln!("▸ Phase 5/7: Effect inference");
    let effect_infer = effects::infer_effects(&module);
    for diag in &effect_infer.diagnostics {
        eprintln!("  {filename}: {diag}");
        if diag.severity == hir::Severity::Error {
            total_errors += 1;
        }
    }
    for (name, fx) in &effect_infer.inferred {
        if fx.is_empty() {
            eprintln!("  f {name}: pure");
        } else {
            let effects: Vec<String> = fx.iter().map(|e| e.to_string()).collect();
            eprintln!("  f {name}: {{ {} }}", effects.join(", "));
        }
    }

    // ── Phase 5.5: Contract verification ────────────────────────────
    eprintln!("▸ Phase 5.5: Contract verification");
    let verifications = verify::verify_module(&module);
    let contract_total = verifications.len();
    let contract_verified = verifications
        .iter()
        .filter(|v| v.status == verify::VerifyStatus::Verified)
        .count();
    let contract_failed = verifications
        .iter()
        .filter(|v| v.status == verify::VerifyStatus::Failed)
        .count();
    if contract_total > 0 {
        eprintln!(
            "  ✓ {contract_total} symbols checked (verified: {contract_verified}, failed: {contract_failed})"
        );
        for v in &verifications {
            if v.status != verify::VerifyStatus::Trivial {
                let sym = match v.status {
                    verify::VerifyStatus::Verified => "✓",
                    verify::VerifyStatus::Partial => "~",
                    verify::VerifyStatus::Failed => "✗",
                    verify::VerifyStatus::Trivial => "-",
                };
                eprintln!("    {sym} {}: {:?}", v.fqn, v.status);
            }
        }
    } else {
        eprintln!("  - no contracts to verify");
    }

    // ── Phase 6: MLIR lowering ───────────────────────────────────────
    eprintln!("▸ Phase 6/7: MLIR lowering");
    let mlir_output = mlir::emit(&module, &effect_infer);
    let mlir_lines = mlir_output.lines().count();
    eprintln!("  ✓ {mlir_lines} lines of MLIR generated");

    // ── Phase 7: Self-healing ─────────────────────────────────────────
    eprintln!("▸ Phase 7/7: Self-healing analysis");
    let mut all_diags: Vec<hir::Diagnostic> = Vec::new();
    all_diags.extend(resolver.diagnostics.iter().cloned());
    all_diags.extend(checker.diagnostics.iter().cloned());
    all_diags.extend(effect_infer.diagnostics.iter().cloned());

    let healed = heal::heal(&all_diags);
    let fix_count: usize = healed.iter().map(|h| h.fixes.len()).sum();
    eprintln!(
        "  ✓ {} diagnostics analyzed, {} fix candidates",
        all_diags.len(),
        fix_count
    );

    if fix_count > 0 {
        for h in &healed {
            for fix in &h.fixes {
                eprintln!(
                    "    ▸ [conf={:.0}%] {}",
                    fix.confidence * 100.0,
                    fix.description
                );
            }
        }
    }

    // ── Token Budget Report ────────────────────────────────────────
    if token_report {
        eprintln!("▸ Token Budget Report:");
        let budget_report = token_budget::report(&module);
        eprintln!("{}", budget_report.display());
    }

    // ── Summary ──────────────────────────────────────────────────────
    eprintln!();
    eprintln!("═══ Pipeline Summary ═══════════════════════════════════════════");
    eprintln!("  Source:          {filename}");
    eprintln!("  Tokens:          {token_count}");
    eprintln!("  Items:           {}", module.items.len());
    eprintln!("  Symbols:         {}", resolver.symbols.len());
    eprintln!("  Functions:       {}", effect_infer.inferred.len());
    eprintln!("  Contracts:       {contract_total} (verified: {contract_verified})");
    eprintln!("  AI subsystems:   {}", ai_info.len());
    eprintln!("  MLIR lines:      {mlir_lines}");
    eprintln!("  Fix candidates:  {fix_count}");
    eprintln!("  Errors:          {total_errors}");

    if total_errors > 0 {
        eprintln!("  Status:          FAIL");
        eprintln!("════════════════════════════════════════════════════════════════");
        std::process::exit(1);
    } else {
        eprintln!("  Status:          OK");
        eprintln!("════════════════════════════════════════════════════════════════");
    }

    // Print output to stdout.
    if token_report {
        let budget_report = token_budget::report(&module);
        println!("{}", serde_json::to_string_pretty(&budget_report).unwrap());
    } else {
        println!("{mlir_output}");
    }
}
