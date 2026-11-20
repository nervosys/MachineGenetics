# MAGE Token-Efficiency Report

Generated from `benchmarks/tasks/*.json` (99 tasks).  Both MAGE and Rust sources are re-tokenised with the same lexer rule (identifiers, literals, single-character sigils) before counting.

## Source bytes (what LLM BPE actually sees)

The most honest measurement for agent-input cost: raw source bytes and whitespace-stripped bytes. LLM BPE tokens correlate roughly with bytes (≈ 3–4 bytes / token for code), so this is what determines an agent's context-window and inference cost.

| Category | Tasks | MAGE bytes | Rust bytes | Ratio | Reduction | Dense MG | Dense RS | Dense ratio |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| agent-orchestration | 10 | 9742 | 9659 | 1.009 | -0.9% | 6495 | 7409 | 0.877 |
| algorithms | 15 | 3901 | 3294 | 1.184 | -18.4% | 2213 | 2109 | 1.049 |
| basic-io | 10 | 621 | 726 | 0.855 | 14.5% | 398 | 498 | 0.799 |
| concurrency | 9 | 4219 | 3437 | 1.228 | -22.8% | 2860 | 2620 | 1.092 |
| data-structures | 15 | 3354 | 3681 | 0.911 | 8.9% | 2260 | 2553 | 0.885 |
| error-handling | 5 | 2403 | 2536 | 0.948 | 5.2% | 1713 | 2034 | 0.842 |
| full-applications | 10 | 16524 | 15700 | 1.052 | -5.2% | 10441 | 11690 | 0.893 |
| generics-traits | 5 | 3284 | 3011 | 1.091 | -9.1% | 2082 | 2160 | 0.964 |
| systems | 10 | 6429 | 5961 | 1.079 | -7.9% | 4304 | 4307 | 0.999 |
| web-network | 10 | 4259 | 4711 | 0.904 | 9.6% | 3060 | 3675 | 0.833 |
| **Total** | **99** | **54736** | **52716** | **1.038** | **-3.8%** | **35826** | **39055** | **0.917** |

## Per-category aggregates (native lexers)

MAGE counted by `prototype::lexer` (atomic sigils like `+f` = 1 token). Rust counted by `proc-macro2` (group delimiters count as 2).

| Category | Tasks | MAGE | Rust | Ratio | Reduction |
|---|---:|---:|---:|---:|---:|
| agent-orchestration | 10 | 2335 | 2522 | 0.926 | 7.4% |
| algorithms | 15 | 1252 | 1075 | 1.165 | -16.5% |
| basic-io | 10 | 259 | 276 | 0.938 | 6.2% |
| concurrency | 9 | 1216 | 1080 | 1.126 | -12.6% |
| data-structures | 15 | 1168 | 1217 | 0.960 | 4.0% |
| error-handling | 5 | 728 | 786 | 0.926 | 7.4% |
| full-applications | 10 | 4228 | 4533 | 0.933 | 6.7% |
| generics-traits | 5 | 888 | 870 | 1.021 | -2.1% |
| systems | 10 | 1933 | 1772 | 1.091 | -9.1% |
| web-network | 10 | 1128 | 1329 | 0.849 | 15.1% |
| **Total** | **99** | **15135** | **15460** | **0.979** | **2.1%** |

## Shared-rule cross-check

Same naive tokeniser (whitespace + identifier + literal + single sigil) applied to both. Removes lexer-convention advantage; shows the savings that come from sigil grouping vs from raw character density.

| Category | MAGE | Rust | Ratio |
|---|---:|---:|---:|
| agent-orchestration | 2415 | 2522 | 0.958 |
| algorithms | 1298 | 1075 | 1.207 |
| basic-io | 270 | 276 | 0.978 |
| concurrency | 1254 | 1080 | 1.161 |
| data-structures | 1210 | 1217 | 0.994 |
| error-handling | 754 | 786 | 0.959 |
| full-applications | 4409 | 4533 | 0.973 |
| generics-traits | 914 | 870 | 1.051 |
| systems | 1998 | 1772 | 1.128 |
| web-network | 1179 | 1329 | 0.887 |
| **Total** | **15701** | **15460** | **1.016** |

## Claimed vs measured (corpus integrity)

| Category | MAGE claimed | Rust claimed | Claimed ratio |
|---|---:|---:|---:|
| agent-orchestration | 2168 | 2918 | 0.743 |
| algorithms | 953 | 1104 | 0.863 |
| basic-io | 208 | 246 | 0.846 |
| concurrency | 1005 | 1154 | 0.871 |
| data-structures | 849 | 1070 | 0.793 |
| error-handling | 648 | 705 | 0.919 |
| full-applications | 4089 | 4610 | 0.887 |
| generics-traits | 712 | 930 | 0.766 |
| systems | 1610 | 1898 | 0.848 |
| web-network | 1049 | 1358 | 0.772 |
| **Total** | **13291** | **15993** | **0.831** |

## Top 10 token savings (MAGE vs Rust)

| Task | Saving | MAGE tokens | Rust tokens |
|---|---:|---:|---:|
| basic-001 | 41.2% | 10 | 17 |
| web-005 | 38.6% | 51 | 83 |
| basic-003 | 38.1% | 13 | 21 |
| conc-002 | 38.1% | 70 | 113 |
| web-009 | 29.9% | 96 | 137 |
| conc-007 | 26.7% | 140 | 191 |
| web-001 | 24.8% | 85 | 113 |
| ds-002 | 20.8% | 42 | 53 |
| web-007 | 19.8% | 73 | 91 |
| agent-001 | 18.4% | 240 | 294 |

## Regressions (|claimed − measured| > 10%)

| Task | Lang | Claimed | Measured | Δ |
|---|---|---:|---:|---:|
| agent-002 | rust | 400 | 296 | -104 |
| agent-005 | mage | 210 | 288 | +78 |
| agent-005 | rust | 340 | 305 | -35 |
| agent-006 | mage | 210 | 234 | +24 |
| agent-006 | rust | 310 | 248 | -62 |
| agent-007 | rust | 260 | 230 | -30 |
| agent-009 | rust | 260 | 174 | -86 |
| agent-010 | mage | 220 | 270 | +50 |
| agent-010 | rust | 320 | 270 | -50 |
| algo-001 | mage | 24 | 30 | +6 |
| algo-001 | rust | 28 | 32 | +4 |
| algo-002 | mage | 68 | 88 | +20 |
| algo-003 | mage | 72 | 107 | +35 |
| algo-003 | rust | 96 | 107 | +11 |
| algo-004 | rust | 78 | 86 | +8 |
| algo-005 | mage | 24 | 34 | +10 |
| algo-005 | rust | 28 | 36 | +8 |
| algo-006 | mage | 98 | 124 | +26 |
| algo-006 | rust | 118 | 99 | -19 |
| algo-007 | mage | 44 | 59 | +15 |
| algo-008 | mage | 48 | 56 | +8 |
| algo-008 | rust | 56 | 50 | -6 |
| algo-009 | mage | 24 | 32 | +8 |
| algo-009 | rust | 28 | 33 | +5 |
| algo-010 | mage | 90 | 131 | +41 |
| algo-011 | mage | 46 | 59 | +13 |
| algo-012 | mage | 50 | 70 | +20 |
| algo-013 | mage | 50 | 72 | +22 |
| algo-014 | mage | 110 | 134 | +24 |
| algo-014 | rust | 136 | 113 | -23 |
| algo-015 | mage | 106 | 157 | +51 |
| basic-002 | mage | 16 | 19 | +3 |
| basic-002 | rust | 18 | 20 | +2 |
| basic-004 | mage | 22 | 25 | +3 |
| basic-005 | mage | 12 | 15 | +3 |
| basic-005 | rust | 14 | 16 | +2 |
| basic-006 | mage | 22 | 27 | +5 |
| basic-007 | mage | 20 | 23 | +3 |
| basic-008 | mage | 12 | 15 | +3 |
| basic-008 | rust | 14 | 16 | +2 |
| basic-009 | mage | 48 | 71 | +23 |
| basic-009 | rust | 56 | 68 | +12 |
| basic-010 | mage | 32 | 41 | +9 |
| basic-010 | rust | 36 | 40 | +4 |
| conc-001 | mage | 72 | 103 | +31 |
| conc-002 | mage | 52 | 70 | +18 |
| conc-002 | rust | 100 | 113 | +13 |
| conc-003 | rust | 128 | 112 | -16 |
| conc-004 | mage | 72 | 103 | +31 |
| conc-005 | mage | 150 | 220 | +70 |
| conc-006 | rust | 108 | 96 | -12 |
| conc-007 | mage | 120 | 140 | +20 |
| conc-009 | mage | 160 | 201 | +41 |
| conc-009 | rust | 190 | 156 | -34 |
| ds-001 | mage | 24 | 31 | +7 |
| ds-002 | mage | 36 | 42 | +6 |
| ds-003 | mage | 62 | 92 | +30 |
| ds-003 | rust | 86 | 100 | +14 |
| ds-004 | mage | 32 | 50 | +18 |
| ds-004 | rust | 40 | 50 | +10 |
| ds-006 | mage | 38 | 51 | +13 |
| ds-006 | rust | 52 | 59 | +7 |
| ds-007 | mage | 72 | 107 | +35 |
| ds-007 | rust | 108 | 127 | +19 |
| ds-008 | mage | 56 | 83 | +27 |
| ds-008 | rust | 74 | 83 | +9 |
| ds-009 | mage | 110 | 179 | +69 |
| ds-009 | rust | 142 | 179 | +37 |
| ds-010 | mage | 44 | 62 | +18 |
| ds-010 | rust | 58 | 71 | +13 |
| ds-011 | mage | 30 | 41 | +11 |
| ds-011 | rust | 38 | 44 | +6 |
| ds-012 | mage | 48 | 66 | +18 |
| ds-013 | rust | 56 | 69 | +13 |
| ds-014 | mage | 40 | 52 | +12 |
| ds-015 | mage | 130 | 185 | +55 |
| err-001 | rust | 55 | 46 | -9 |
| err-002 | rust | 120 | 135 | +15 |
| err-004 | rust | 150 | 167 | +17 |
| err-005 | mage | 100 | 180 | +80 |
| err-005 | rust | 170 | 208 | +38 |
| app-002 | mage | 320 | 357 | +37 |
| app-002 | rust | 480 | 419 | -61 |
| app-003 | mage | 290 | 392 | +102 |
| app-003 | rust | 450 | 395 | -55 |
| app-004 | rust | 560 | 685 | +125 |
| app-009 | rust | 400 | 357 | -43 |
| gt-001 | rust | 40 | 34 | -6 |
| gt-002 | rust | 140 | 106 | -34 |
| gt-003 | mage | 170 | 285 | +115 |
| gt-003 | rust | 280 | 309 | +29 |
| gt-004 | mage | 140 | 193 | +53 |
| gt-005 | rust | 290 | 256 | -34 |
| sys-001 | mage | 170 | 240 | +70 |
| sys-002 | mage | 170 | 252 | +82 |
| sys-003 | mage | 140 | 188 | +48 |
| sys-003 | rust | 180 | 153 | -27 |
| sys-005 | mage | 68 | 94 | +26 |
| sys-008 | mage | 160 | 232 | +72 |
| sys-009 | rust | 72 | 60 | -12 |
| sys-010 | mage | 160 | 183 | +23 |
| sys-010 | rust | 250 | 203 | -47 |
| web-001 | mage | 72 | 85 | +13 |
| web-002 | rust | 220 | 261 | +41 |
| web-003 | mage | 140 | 181 | +41 |
| web-004 | rust | 120 | 86 | -34 |
| web-005 | mage | 58 | 51 | -7 |
| web-005 | rust | 100 | 83 | -17 |
| web-008 | mage | 62 | 82 | +20 |
| web-009 | mage | 82 | 96 | +14 |
| web-010 | rust | 160 | 135 | -25 |

---
_Lexer rule used: see `prototype/src/bin/token_bench.rs` docs. Regression threshold: ±10%._
