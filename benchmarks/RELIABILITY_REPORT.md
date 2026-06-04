# MechGen Agent-Write Reliability Report

Backend: **file-oracle**.  Generated from `benchmarks/tasks/*.json` (100 tasks).

## Summary

| Stage | Pass | Total | Rate |
|---|---:|---:|---:|
| Lex (no error tokens) | 100 | 100 | 100.0% |
| Parse (LL(1) accepts) | 99 | 100 | 99.0% |
| Self-heal proposed a fix (on failures) | 0 | 1 | 0.0% |
| Self-heal made it re-parse | 0 | 1 | 0.0% |
| Structural-heal re-parse (brace balance at EOF) | 1 | 1 | 100.0% |

**Effective pass rate (parse OR pattern-heal OR structural-heal):** 100 / 100 = 100.0%

## Per-category breakdown

| Category | Tasks | Lex OK | Parse OK | Lex % | Parse % |
|---|---:|---:|---:|---:|---:|
| agent-orchestration | 10 | 10 | 10 | 100.0% | 100.0% |
| algorithms | 15 | 15 | 15 | 100.0% | 100.0% |
| basic-io | 10 | 10 | 10 | 100.0% | 100.0% |
| concurrency | 10 | 10 | 10 | 100.0% | 100.0% |
| data-structures | 15 | 15 | 15 | 100.0% | 100.0% |
| error-handling | 5 | 5 | 5 | 100.0% | 100.0% |
| full-applications | 10 | 10 | 9 | 100.0% | 90.0% |
| generics-traits | 5 | 5 | 5 | 100.0% | 100.0% |
| systems | 10 | 10 | 10 | 100.0% | 100.0% |
| web-network | 10 | 10 | 10 | 100.0% | 100.0% |

## Failures (1)

| Task | Category | Lex errors | Parse error |
|---|---|---:|---|
| app-008 | full-applications | 0 | 40:67: expected expression, found KwF 'f' |

## Per-task pipeline latency (lex + parse)

| Percentile | µs |
|---|---:|
| p50 | 16 |
| p95 | 126 |
| p99 | 404 |

---
_Backend interface: `CandidateAgent::propose(&Task) -> Result<String, String>`. Wire a real LLM by implementing this trait and replacing `FileOracleAgent` in `prototype/src/bin/reliability_bench.rs`._
