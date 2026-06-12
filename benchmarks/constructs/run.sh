#!/usr/bin/env bash
# Are MechGen's HIGH-LEVEL constructs (the §8 vocabulary) significantly more
# token-efficient than the explicit low-level equivalent that computes the SAME
# result? Each pair is verified: both forms --check, and both --eval to the same
# value (so the token comparison is fair). Token counts are real cl100k BPE,
# measured separately with agentic-eval's `tokens_of` example (printed below from
# the recorded measurement; reproduce with the command at the end).
set -u
cd "$(dirname "$0")"
MG="${MG:-../../prototype/target/release/MechGen-parse.exe}"
[ -x "$MG" ] || MG="${MG}.exe"

# pair  driver-call(input)               expected   high-level construct
PAIRS=(
  "even_squares|s([1,2,3,4,5])|20|sum(map(filter(…)))   vs  for+if accumulator"
  "distinct_words|s(\"a b a c\")|3|len(keys(freq(words…))) vs  for-build map"
  "sum|s([1,2,3,4,5])|15|sum(xs)               vs  for accumulator"
  "scan|s([1,2,3,4,5])|[0, 1, 3, 6, 10, 15]|scan(xs,0,+)        vs  for-build list"
)
# Recorded real cl100k BPE (agentic-eval tokens_of, 2026-06-12): hl ll
declare -A HL=( [even_squares]=30 [distinct_words]=11 [sum]=8  [scan]=21 )
declare -A LL=( [even_squares]=35 [distinct_words]=28 [sum]=23 [scan]=37 )

echo "=== High-level vs explicit constructs — token efficiency (verified pairs) ==="
printf "%-16s %5s %5s  %6s   %s\n" "construct" "HL" "LL" "−tok" "equiv/check"
echo "---------------------------------------------------------------------------"
thl=0; tll=0
for row in "${PAIRS[@]}"; do
  IFS='|' read -r name call expect desc <<< "$row"
  # check both
  ok="ok"; for v in hl ll; do "$MG" "${name}_${v}.mg" >/dev/null 2>&1 || ok="CHECK-FAIL($v)"; done
  # equivalence: same eval result
  a=$(printf '%s\nf main(){ %s }' "$(cat ${name}_hl.mg)" "$call" | { cat > /tmp/_h.mg; "$MG" --eval /tmp/_h.mg main 2>&1; })
  b=$(printf '%s\nf main(){ %s }' "$(cat ${name}_ll.mg)" "$call" | { cat > /tmp/_l.mg; "$MG" --eval /tmp/_l.mg main 2>&1; })
  eq="≠"; [ "$a" = "$b" ] && [ "$a" = "$expect" ] && eq="= ($a)"
  h=${HL[$name]}; l=${LL[$name]}; red=$(( (l-h)*100/l ))
  printf "%-16s %5s %5s  %5s%%   %s %s\n" "$name" "$h" "$l" "$red" "$ok" "$eq"
  thl=$((thl+h)); tll=$((tll+l))
done
echo "---------------------------------------------------------------------------"
printf "%-16s %5s %5s  %5s%%   (high-level total vs explicit total)\n" "TOTAL" "$thl" "$tll" "$(( (tll-thl)*100/tll ))"
echo
echo "Reading: pure vocabulary calls (sum/freq/scan) cut 43–65% — a named combinator"
echo "replaces the whole \`var t; for … { } t\` scaffold. When the per-element logic is a"
echo "custom closure (even_squares) the saving shrinks to 14%: the closure body is the"
echo "irreducible payload, present in both forms. So: significant in aggregate (1.76×),"
echo "concentrated where a vocabulary op subsumes control-flow boilerplate."
echo
echo
echo "=== Neural-net architectures — declarative \`net\` DSL vs PyTorch nn.Module ==="
echo "Expressing the SAME architecture (standard layer stack). MechGen declares the"
echo "layers; PyTorch must also spell out the imperative forward (residuals, the"
echo "attention call, norm(x+a)). Token counts real cl100k BPE; ABL bytes measured live."
printf "%-13s %8s %8s  %6s   %s\n" "architecture" "MechGen" "PyTorch" "−tok" "MechGen text → ABL binary"
echo "---------------------------------------------------------------------------"
# Recorded real cl100k BPE (tokens_of): mechgen pytorch
declare -A NHL=( [mlp]=50 [transformer]=73 ); declare -A NLL=( [mlp]=78 [transformer]=142 )
for n in mlp transformer; do
  "$MG" "${n}_mechgen.mg" >/dev/null 2>&1 && chk="✓" || chk="check-FAIL"
  "$MG" --target=abl-bytes "${n}_mechgen.mg" /tmp/_n.abl >/dev/null 2>&1
  tb=$(wc -c <"${n}_mechgen.mg"); ab=$(wc -c </tmp/_n.abl 2>/dev/null)
  h=${NHL[$n]}; l=${NLL[$n]}; red=$(( ((l-h)*100 + l/2)/l )); abr=$(( ((tb-ab)*100 + tb/2)/tb ))
  printf "%-13s %8s %8s  %5s%%   %4dB → %3dB (−%d%%)  %s\n" "$n" "$h" "$l" "$red" "$tb" "$ab" "$abr" "$chk"
done
echo "---------------------------------------------------------------------------"
echo "The saving GROWS with architecture complexity (MLP 36% → Transformer 49%):"
echo "the more forward-wiring the DSL subsumes, the bigger the win. MechGen then lowers"
echo "the declaration to a byte-level binary IR — a further 34-42% under its own text."
echo "(Honest: this measures tokens to EXPRESS the architecture; the DSL leaves the"
echo " forward graph implicit, where PyTorch makes it explicit — that gap IS the saving.)"
rm -f /tmp/_n.abl 2>/dev/null
echo
echo "Reproduce token counts (real cl100k BPE), from this directory:"
echo "  cargo run -q -p agentic-eval --example tokens_of --features real-tokens -- *_hl.mg *_ll.mg *_mechgen.mg *_pytorch.py"
echo "  (run in nervosys/cli/AetherShell, passing absolute paths to these files)"
rm -f /tmp/_h.mg /tmp/_l.mg 2>/dev/null
