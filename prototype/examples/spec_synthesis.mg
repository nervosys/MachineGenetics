// ── Example 3: Spec-Driven Synthesis ────────────────────────────────
//
// Demonstrates contract-directed code generation:
// 1. Developer writes a function specification (contracts only, no body)
// 2. The synthesis oracle generates a correct implementation
// 3. Verification certificates confirm the implementation meets the spec

// ── Step 1: Write Specifications ───────────────────────────────────

// Specification for binary search — contracts define the interface.
// The body is missing; the synthesis oracle fills it in.

f binary_search(arr: &[i32], target: i32) -> usize?
    @req arr.windows(2).all(|w| w[0] <= w[1]) "input must be sorted"
    @ens result.map_or(true, |i| arr[i] == target)
    @fx pure;

// Specification for insertion sort.

f insertion_sort(arr: &mut [i32])
    @req arr.len() > 0
    @ens arr.windows(2).all(|w| w[0] <= w[1])
    @ens arr.len() == old(arr.len())
    @fx pure;

// Specification for merging two sorted arrays.

f merge_sorted(a: &[i32], b: &[i32]) -> [i32]~
    @req a.windows(2).all(|w| w[0] <= w[1])
    @req b.windows(2).all(|w| w[0] <= w[1])
    @ens result.len() == a.len() + b.len()
    @ens result.windows(2).all(|w| w[0] <= w[1])
    @fx pure;

// Specification for deduplication preserving order.

f dedup(items: [i32]~) -> [i32]~
    @ens result.len() <= items.len()
    @fx pure;

// ── Step 2: Synthesis Oracle Generates Implementations ─────────────

// The synthesis oracle reads the contracts and produces correct code.
// Below are the synthesised implementations.

// Synthesised: binary_search
//
// f binary_search(arr: &[i32], target: i32) -> usize?
//     @req arr.windows(2).all(|w| w[0] <= w[1]) "input must be sorted"
//     @ens result.map_or(true, |i| arr[i] == target)
//     @fx pure
// {
//     var lo: usize = 0;
//     var hi: usize = arr.len();
//     @w lo < hi {
//         val mid = lo + (hi - lo) / 2;
//         ?: arr[mid] == target { ret Some(mid) }
//         ?: arr[mid] < target { lo = mid + 1 } _ { hi = mid }
//     }
//     None
// }

// Synthesised: insertion_sort
//
// f insertion_sort(arr: &mut [i32])
//     @req arr.len() > 0
//     @ens arr.windows(2).all(|w| w[0] <= w[1])
//     @ens arr.len() == old(arr.len())
//     @fx pure
// {
//     @ i in 1..arr.len() {
//         val key = arr[i];
//         var j = i;
//         @w j > 0 && arr[j - 1] > key {
//             arr[j] = arr[j - 1];
//             j -= 1;
//         }
//         arr[j] = key;
//     }
// }

// Synthesised: merge_sorted
//
// f merge_sorted(a: &[i32], b: &[i32]) -> [i32]~
//     @req a.windows(2).all(|w| w[0] <= w[1])
//     @req b.windows(2).all(|w| w[0] <= w[1])
//     @ens result.len() == a.len() + b.len()
//     @ens result.windows(2).all(|w| w[0] <= w[1])
//     @fx pure
// {
//     var result = Vec.with_capacity(a.len() + b.len());
//     var i = 0;
//     var j = 0;
//     @w i < a.len() && j < b.len() {
//         ?: a[i] <= b[j] {
//             result.push(a[i]);
//             i += 1;
//         } _ {
//             result.push(b[j]);
//             j += 1;
//         }
//     }
//     @w i < a.len() { result.push(a[i]); i += 1; }
//     @w j < b.len() { result.push(b[j]); j += 1; }
//     result
// }

// Synthesised: dedup
//
// f dedup(items: [i32]~) -> [i32]~
//     @ens result.len() <= items.len()
//     @fx pure
// {
//     var seen = HashSet.new();
//     var result = Vec.new();
//     @ item in items {
//         ?: seen.insert(item) {
//             result.push(item);
//         }
//     }
//     result
// }

// ── Step 3: Verification Certificates ──────────────────────────────

// After synthesis, the verifier checks each implementation against its
// contracts and produces a verification certificate.
//
// Certificate for binary_search:
//   strategy: bounded_model_checking
//   status: verified
//   preconditions_checked: ["arr sorted"]
//   postconditions_checked: ["result points to target"]
//   counterexamples: none
//
// Certificate for insertion_sort:
//   strategy: inductive_invariant
//   status: verified
//   preconditions_checked: ["arr non-empty"]
//   postconditions_checked: ["arr sorted", "length preserved"]
//   counterexamples: none

// ── Step 4: Integration ────────────────────────────────────────────

// The developer writes specs, the synthesis oracle generates code,
// the verifier issues certificates, and the result is production-ready.
//
// Workflow:
//   1. `mechgen synth src/specs.rx`         — synthesise implementations
//   2. `mechgen verify src/specs.rx`        — verify against contracts
//   3. `mechgen build src/specs.rx`         — compile to target
//
// The synthesis oracle can also be invoked programmatically:
//
// val oracle = SynthesisOracle.new();
// val spec = parse_spec("f sort(arr: &mut [i32]) @ens sorted(arr) @fx pure;");
// val result = oracle.synthesise(&spec);
// ?: result.is_success() {
//     val cert = verify(result.implementation(), &spec);
//     p"Verified: {cert.status}";
// }
