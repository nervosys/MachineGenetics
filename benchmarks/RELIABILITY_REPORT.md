# MAGE Agent-Write Reliability Report

Backend: **perturbed-oracle**.  Generated from `benchmarks/tasks/*.json` (100 tasks).

## Summary

| Stage | Pass | Total | Rate |
|---|---:|---:|---:|
| Lex (no error tokens) | 100 | 100 | 100.0% |
| Parse (LL(1) accepts) | 27 | 100 | 27.0% |
| Self-heal proposed a fix (on failures) | 62 | 73 | 84.9% |
| Self-heal made it re-parse | 42 | 73 | 57.5% |
| Structural-heal re-parse (brace balance at EOF) | 1 | 73 | 1.4% |

**Effective pass rate (parse OR pattern-heal OR structural-heal):** 70 / 100 = 70.0%

## Per-category breakdown

| Category | Tasks | Lex OK | Parse OK | Lex % | Parse % |
|---|---:|---:|---:|---:|---:|
| agent-orchestration | 10 | 10 | 2 | 100.0% | 20.0% |
| algorithms | 15 | 15 | 6 | 100.0% | 40.0% |
| basic-io | 10 | 10 | 3 | 100.0% | 30.0% |
| concurrency | 10 | 10 | 3 | 100.0% | 30.0% |
| data-structures | 15 | 15 | 4 | 100.0% | 26.7% |
| error-handling | 5 | 5 | 1 | 100.0% | 20.0% |
| full-applications | 10 | 10 | 2 | 100.0% | 20.0% |
| generics-traits | 5 | 5 | 2 | 100.0% | 40.0% |
| systems | 10 | 10 | 2 | 100.0% | 20.0% |
| web-network | 10 | 10 | 2 | 100.0% | 20.0% |

## Failures (73)

| Task | Category | Lex errors | Parse error |
|---|---|---:|---|
| agent-001 | agent-orchestration | 0 | 2:33: expected KwF, found Semi ';' |
| agent-002 | agent-orchestration | 0 | 39:18: expected type, found Assign |
| agent-004 | agent-orchestration | 0 | 30:1: expected RBrace, found Eof '' |
| agent-005 | agent-orchestration | 0 | 36:5: expected expression, found RBrace '}' |
| agent-007 | agent-orchestration | 0 | 1:14: expected identifier, found Comma ',' |
| agent-008 | agent-orchestration | 0 | 26:7: expected identifier, found Eof '' |
| agent-009 | agent-orchestration | 0 | 22:17: expected expression, found Semi ';' |
| agent-010 | agent-orchestration | 0 | 42:48: expected expression, found Semi ';' |
| algo-001 | algorithms | 0 | 2:44: expected expression, found RBrace '}' |
| algo-003 | algorithms | 0 | 1:55: expected expression, found Comma ',' |
| algo-004 | algorithms | 0 | 7:22: expected expression, found Eof '' |
| algo-006 | algorithms | 0 | 13:24: expected type, found Assign |
| algo-008 | algorithms | 0 | 10:1: expected RBrace, found Eof '' |
| algo-009 | algorithms | 0 | 2:50: expected expression, found RBrace '}' |
| algo-010 | algorithms | 0 | 2:28: expected expression, found Semi ';' |
| algo-013 | algorithms | 0 | 10:1: expected RBrace, found Eof '' |
| algo-014 | algorithms | 0 | 14:39: expected expression, found Semi ';' |
| basic-001 | basic-io | 0 | 1:26: expected RBrace, found Eof '' |
| basic-003 | basic-io | 0 | 1:45: expected RBrace, found Eof '' |
| basic-005 | basic-io | 0 | 1:34: expected RBrace, found Eof '' |
| basic-006 | basic-io | 0 | 1:27: expected identifier, found Arrow '->' |
| basic-008 | basic-io | 0 | 1:27: expected expression, found Comma ',' |
| basic-009 | basic-io | 0 | 5:12: expected LBrace, found Eof '' |
| basic-010 | basic-io | 0 | 3:1: expected RBrace, found Eof '' |
| conc-001 | concurrency | 0 | 2:45: expected KwF, found Semi ';' |
| conc-004 | concurrency | 0 | 11:1: expected RBrace, found Eof '' |
| conc-005 | concurrency | 0 | 19:51: expected expression, found Semi ';' |
| conc-007 | concurrency | 0 | 1:16: expected identifier, found Comma ',' |
| conc-008 | concurrency | 0 | 16:2: expected RBrace, found Eof '' |
| conc-009 | concurrency | 0 | 8:36: expected expression, found Semi ';' |
| conc-010 | concurrency | 0 | 23:37: expected expression, found Semi ';' |
| ds-002 | data-structures | 0 | 1:28: expected identifier, found Plus '+' |
| ds-004 | data-structures | 0 | 5:1: expected RBrace, found Eof '' |
| ds-005 | data-structures | 0 | 1:31: expected identifier, found Arrow '->' |
| ds-007 | data-structures | 0 | 1:12: expected identifier, found Comma ',' |
| ds-008 | data-structures | 0 | 8:15: expected expression, found Eof '' |
| ds-009 | data-structures | 0 | 2:27: expected expression, found Semi ';' |
| ds-010 | data-structures | 0 | 7:5: expected expression, found RBrace '}' |
| ds-012 | data-structures | 0 | 1:30: expected expression, found Comma ',' |
| ds-013 | data-structures | 0 | 5:20: expected RBrace, found Eof '' |
| ds-014 | data-structures | 0 | 2:27: expected expression, found Semi ';' |
| ds-015 | data-structures | 0 | 12:26: expected LBrace, found KwVal 'val' |
| err-001 | error-handling | 0 | 1:43: expected expression, found Comma ',' |
| err-002 | error-handling | 0 | 10:6: expected expression, found Eof '' |
| err-003 | error-handling | 0 | 18:28: expected expression, found Semi ';' |
| err-004 | error-handling | 0 | 13:29: expected Assign, found Ident 'strings' |
| app-001 | full-applications | 0 | 1:12: expected identifier, found Comma ',' |
| app-002 | full-applications | 0 | 44:24: expected expression, found Eof '' |
| app-003 | full-applications | 0 | 22:29: expected expression, found Semi ';' |
| app-004 | full-applications | 0 | 32:15: expected FatArrow, found Ident 'l' |
| app-006 | full-applications | 0 | 64:1: expected RBrace, found Eof '' |
| app-007 | full-applications | 0 | 48:1: expected expression, found RBrace '}' |
| app-008 | full-applications | 0 | 40:67: expected expression, found KwF 'f' |
| app-009 | full-applications | 0 | 1:15: expected identifier, found Comma ',' |
| gt-002 | generics-traits | 0 | 8:5: expected item, found Ident |
| gt-004 | generics-traits | 0 | 40:1: expected RBrace, found Eof '' |
| gt-005 | generics-traits | 0 | 47:5: expected expression, found RBrace '}' |
| sys-001 | systems | 0 | 10:27: expected expression, found Semi ';' |
| sys-002 | systems | 0 | 12:45: expected RBrack, found IntLiteral '64' |
| sys-004 | systems | 0 | 23:1: expected RBrace, found Eof '' |
| sys-005 | systems | 0 | 9:1: expected expression, found RBrace '}' |
| sys-007 | systems | 0 | 1:13: expected KwF, found Comma ',' |
| sys-008 | systems | 0 | 22:10: expected RBrace, found Eof '' |
| sys-009 | systems | 0 | 2:48: expected expression, found Semi ';' |
| sys-010 | systems | 0 | 25:65: expected expression, found Semi ';' |
| web-001 | web-network | 0 | 9:3: expected RBrace, found Eof '' |
| web-002 | web-network | 0 | 9:32: expected expression, found Semi ';' |
| web-003 | web-network | 0 | 12:24: expected LBrace, found At '@' |
| web-005 | web-network | 0 | 9:1: expected RBrace, found Eof '' |
| web-006 | web-network | 0 | 8:63: expected expression, found Semi ';' |
| web-008 | web-network | 0 | 1:14: expected identifier, found Comma ',' |
| web-009 | web-network | 0 | 11:8: expected RBrace, found Eof '' |
| web-010 | web-network | 0 | 19:1: expected RBrace, found Eof '' |

## Per-task pipeline latency (lex + parse)

| Percentile | µs |
|---|---:|
| p50 | 30 |
| p95 | 257 |
| p99 | 365 |

---
_Backend interface: `CandidateAgent::propose(&Task) -> Result<String, String>`. Wire a real LLM by implementing this trait and replacing `FileOracleAgent` in `prototype/src/bin/reliability_bench.rs`._
