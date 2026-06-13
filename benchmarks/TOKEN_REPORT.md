# MAGE Token-Efficiency Report

Generated from `benchmarks/tasks/*.json` (100 tasks).  Both MAGE and Rust sources are re-tokenised with the same lexer rule (identifiers, literals, single-character sigils) before counting.

## Source bytes (what LLM BPE actually sees)

The most honest measurement for agent-input cost: raw source bytes and whitespace-stripped bytes. LLM BPE tokens correlate roughly with bytes (≈ 3–4 bytes / token for code), so this is what determines an agent's context-window and inference cost.

| Category | Tasks | MAGE bytes | Rust bytes | Ratio | Reduction | Dense MG | Dense RS | Dense ratio |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| agent-orchestration | 10 | 9778 | 9369 | 1.044 | -4.4% | 6530 | 7183 | 0.909 |
| algorithms | 15 | 3867 | 3294 | 1.174 | -17.4% | 2191 | 2109 | 1.039 |
| basic-io | 10 | 621 | 726 | 0.855 | 14.5% | 398 | 498 | 0.799 |
| concurrency | 10 | 4639 | 3832 | 1.211 | -21.1% | 3082 | 2924 | 1.054 |
| data-structures | 15 | 3323 | 3681 | 0.903 | 9.7% | 2238 | 2553 | 0.877 |
| error-handling | 5 | 2429 | 2536 | 0.958 | 4.2% | 1739 | 2034 | 0.855 |
| full-applications | 10 | 16549 | 15235 | 1.086 | -8.6% | 10470 | 11307 | 0.926 |
| generics-traits | 5 | 3290 | 3011 | 1.093 | -9.3% | 2088 | 2160 | 0.967 |
| systems | 10 | 6404 | 5943 | 1.078 | -7.8% | 4287 | 4292 | 0.999 |
| web-network | 10 | 4175 | 4596 | 0.908 | 9.2% | 3034 | 3572 | 0.849 |
| **Total** | **100** | **55075** | **52223** | **1.055** | **-5.5%** | **36057** | **38632** | **0.933** |

## Per-category aggregates (native lexers)

MAGE counted by `prototype::lexer` (atomic sigils like `+f` = 1 token). Rust counted by `proc-macro2` (group delimiters count as 2).

| Category | Tasks | MAGE | Rust | Ratio | Reduction |
|---|---:|---:|---:|---:|---:|
| agent-orchestration | 10 | 2355 | 2442 | 0.964 | 3.6% |
| algorithms | 15 | 1239 | 1075 | 1.153 | -15.3% |
| basic-io | 10 | 259 | 276 | 0.938 | 6.2% |
| concurrency | 10 | 1318 | 1191 | 1.107 | -10.7% |
| data-structures | 15 | 1154 | 1217 | 0.948 | 5.2% |
| error-handling | 5 | 754 | 786 | 0.959 | 4.1% |
| full-applications | 10 | 4265 | 4378 | 0.974 | 2.6% |
| generics-traits | 5 | 890 | 870 | 1.023 | -2.3% |
| systems | 10 | 1913 | 1770 | 1.081 | -8.1% |
| web-network | 10 | 1121 | 1305 | 0.859 | 14.1% |
| **Total** | **100** | **15268** | **15310** | **0.997** | **0.3%** |

## Shared-rule cross-check

Same naive tokeniser (whitespace + identifier + literal + single sigil) applied to both. Removes lexer-convention advantage; shows the savings that come from sigil grouping vs from raw character density.

| Category | MAGE | Rust | Ratio |
|---|---:|---:|---:|
| agent-orchestration | 2435 | 2442 | 0.997 |
| algorithms | 1283 | 1075 | 1.193 |
| basic-io | 270 | 276 | 0.978 |
| concurrency | 1357 | 1191 | 1.139 |
| data-structures | 1196 | 1217 | 0.983 |
| error-handling | 780 | 786 | 0.992 |
| full-applications | 4446 | 4378 | 1.016 |
| generics-traits | 916 | 870 | 1.053 |
| systems | 1978 | 1770 | 1.118 |
| web-network | 1170 | 1305 | 0.897 |
| **Total** | **15831** | **15310** | **1.034** |

## Claimed vs measured (corpus integrity)

| Category | MAGE claimed | Rust claimed | Claimed ratio |
|---|---:|---:|---:|
| agent-orchestration | 1920 | 2950 | 0.651 |
| algorithms | 914 | 1104 | 0.828 |
| basic-io | 208 | 246 | 0.846 |
| concurrency | 1014 | 1282 | 0.791 |
| data-structures | 802 | 1070 | 0.750 |
| error-handling | 460 | 705 | 0.652 |
| full-applications | 2960 | 4570 | 0.648 |
| generics-traits | 640 | 930 | 0.688 |
| systems | 1398 | 1888 | 0.740 |
| web-network | 894 | 1350 | 0.662 |
| **Total** | **11210** | **16095** | **0.696** |

## Top 10 token savings (MAGE vs Rust)

| Task | Saving | MAGE tokens | Rust tokens |
|---|---:|---:|---:|
| basic-001 | 41.2% | 10 | 17 |
| web-005 | 38.6% | 51 | 83 |
| basic-003 | 38.1% | 13 | 21 |
| conc-002 | 38.1% | 70 | 113 |
| web-009 | 29.9% | 96 | 137 |
| conc-007 | 26.7% | 140 | 191 |
| ds-002 | 20.8% | 42 | 53 |
| agent-001 | 17.0% | 244 | 294 |
| web-003 | 16.2% | 181 | 216 |
| ds-007 | 15.7% | 107 | 127 |

## Regressions (|claimed − measured| > 10%)

| Task | Lang | Claimed | Measured | Δ |
|---|---|---:|---:|---:|
| agent-001 | mage | 190 | 244 | +54 |
| agent-002 | mage | 260 | 294 | +34 |
| agent-002 | rust | 400 | 296 | -104 |
| agent-003 | rust | 200 | 158 | -42 |
| agent-004 | mage | 160 | 222 | +62 |
| agent-005 | mage | 210 | 288 | +78 |
| agent-005 | rust | 340 | 305 | -35 |
| agent-006 | mage | 210 | 234 | +24 |
| agent-006 | rust | 310 | 248 | -62 |
| agent-007 | mage | 160 | 222 | +62 |
| agent-007 | rust | 260 | 230 | -30 |
| agent-008 | mage | 200 | 252 | +52 |
| agent-008 | rust | 280 | 210 | -70 |
| agent-009 | rust | 260 | 174 | -86 |
| agent-010 | mage | 220 | 270 | +50 |
| agent-010 | rust | 320 | 270 | -50 |
| algo-001 | mage | 24 | 30 | +6 |
| algo-001 | rust | 28 | 32 | +4 |
| algo-002 | mage | 68 | 88 | +20 |
| algo-003 | mage | 72 | 107 | +35 |
| algo-003 | rust | 96 | 107 | +11 |
| algo-004 | mage | 60 | 86 | +26 |
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
| conc-003 | mage | 110 | 139 | +29 |
| conc-003 | rust | 128 | 112 | -16 |
| conc-004 | mage | 72 | 103 | +31 |
| conc-005 | mage | 150 | 220 | +70 |
| conc-006 | mage | 78 | 104 | +26 |
| conc-006 | rust | 108 | 96 | -12 |
| conc-007 | mage | 120 | 140 | +20 |
| conc-008 | mage | 90 | 107 | +17 |
| conc-009 | mage | 160 | 201 | +41 |
| conc-009 | rust | 190 | 156 | -34 |
| conc-010 | mage | 110 | 131 | +21 |
| conc-010 | rust | 128 | 111 | -17 |
| ds-001 | mage | 24 | 31 | +7 |
| ds-002 | mage | 36 | 42 | +6 |
| ds-003 | mage | 62 | 92 | +30 |
| ds-003 | rust | 86 | 100 | +14 |
| ds-004 | mage | 32 | 50 | +18 |
| ds-004 | rust | 40 | 50 | +10 |
| ds-005 | mage | 38 | 49 | +11 |
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
| ds-013 | mage | 42 | 64 | +22 |
| ds-013 | rust | 56 | 69 | +13 |
| ds-014 | mage | 40 | 52 | +12 |
| ds-015 | mage | 130 | 185 | +55 |
| err-001 | rust | 55 | 46 | -9 |
| err-002 | mage | 80 | 128 | +48 |
| err-002 | rust | 120 | 135 | +15 |
| err-003 | mage | 140 | 231 | +91 |
| err-004 | mage | 100 | 171 | +71 |
| err-004 | rust | 150 | 167 | +17 |
| err-005 | mage | 100 | 180 | +80 |
| err-005 | rust | 170 | 208 | +38 |
| app-001 | mage | 250 | 338 | +88 |
| app-002 | mage | 320 | 357 | +37 |
| app-002 | rust | 480 | 419 | -61 |
| app-003 | mage | 290 | 392 | +102 |
| app-003 | rust | 450 | 395 | -55 |
| app-004 | mage | 340 | 610 | +270 |
| app-004 | rust | 560 | 685 | +125 |
| app-005 | mage | 390 | 613 | +223 |
| app-006 | mage | 280 | 348 | +68 |
| app-006 | rust | 360 | 282 | -78 |
| app-007 | mage | 250 | 356 | +106 |
| app-008 | mage | 330 | 559 | +229 |
| app-008 | rust | 520 | 573 | +53 |
| app-009 | mage | 260 | 362 | +102 |
| app-009 | rust | 400 | 357 | -43 |
| app-010 | mage | 250 | 330 | +80 |
| app-010 | rust | 390 | 325 | -65 |
| gt-001 | rust | 40 | 34 | -6 |
| gt-002 | rust | 140 | 106 | -34 |
| gt-003 | mage | 170 | 285 | +115 |
| gt-003 | rust | 280 | 309 | +29 |
| gt-004 | mage | 140 | 193 | +53 |
| gt-005 | mage | 200 | 274 | +74 |
| gt-005 | rust | 290 | 256 | -34 |
| sys-001 | mage | 170 | 240 | +70 |
| sys-002 | mage | 170 | 252 | +82 |
| sys-003 | mage | 140 | 188 | +48 |
| sys-003 | rust | 180 | 153 | -27 |
| sys-004 | mage | 160 | 212 | +52 |
| sys-005 | mage | 68 | 94 | +26 |
| sys-005 | rust | 60 | 68 | +8 |
| sys-006 | mage | 200 | 301 | +101 |
| sys-007 | mage | 110 | 149 | +39 |
| sys-008 | mage | 160 | 232 | +72 |
| sys-009 | rust | 72 | 60 | -12 |
| sys-010 | mage | 160 | 183 | +23 |
| sys-010 | rust | 250 | 203 | -47 |
| web-001 | mage | 72 | 85 | +13 |
| web-001 | rust | 108 | 86 | -22 |
| web-002 | mage | 148 | 226 | +78 |
| web-002 | rust | 220 | 261 | +41 |
| web-003 | mage | 140 | 181 | +41 |
| web-004 | rust | 120 | 86 | -34 |
| web-005 | mage | 58 | 51 | -7 |
| web-005 | rust | 100 | 83 | -17 |
| web-006 | mage | 70 | 102 | +32 |
| web-007 | mage | 56 | 77 | +21 |
| web-008 | mage | 62 | 82 | +20 |
| web-009 | mage | 82 | 96 | +14 |
| web-010 | mage | 110 | 129 | +19 |
| web-010 | rust | 160 | 135 | -25 |

---
_Lexer rule used: see `prototype/src/bin/token_bench.rs` docs. Regression threshold: ±10%._
