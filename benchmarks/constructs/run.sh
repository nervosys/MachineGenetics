#!/usr/bin/env bash
# Are MAGE's HIGH-LEVEL constructs (the §8 vocabulary) significantly more
# token-efficient than the explicit low-level equivalent that computes the SAME
# result? Each pair is verified: both forms --check, and both --eval to the same
# value (so the token comparison is fair). Token counts are real cl100k BPE,
# measured separately with agentic-eval's `tokens_of` example (printed below from
# the recorded measurement; reproduce with the command at the end).
set -u
cd "$(dirname "$0")"
MG="${MG:-../../prototype/target/release/mage-parse.exe}"
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
echo "Expressing the SAME architecture (standard layer stack). MAGE declares the"
echo "layers; PyTorch must also spell out the imperative forward (residuals, the"
echo "attention call, norm(x+a)). Token counts real cl100k BPE; ABL bytes measured live."
printf "%-13s %8s %8s  %6s   %s\n" "architecture" "MAGE" "PyTorch" "−tok" "MAGE text → ABL binary"
echo "---------------------------------------------------------------------------"
# Recorded real cl100k BPE (tokens_of): mage pytorch
declare -A NHL=( [mlp]=50 [transformer]=73 ); declare -A NLL=( [mlp]=78 [transformer]=142 )
for n in mlp transformer; do
  "$MG" "${n}_mage.mg" >/dev/null 2>&1 && chk="✓" || chk="check-FAIL"
  "$MG" --target=abl-bytes "${n}_mage.mg" /tmp/_n.abl >/dev/null 2>&1
  tb=$(wc -c <"${n}_mage.mg"); ab=$(wc -c </tmp/_n.abl 2>/dev/null)
  h=${NHL[$n]}; l=${NLL[$n]}; red=$(( ((l-h)*100 + l/2)/l )); abr=$(( ((tb-ab)*100 + tb/2)/tb ))
  printf "%-13s %8s %8s  %5s%%   %4dB → %3dB (−%d%%)  %s\n" "$n" "$h" "$l" "$red" "$tb" "$ab" "$abr" "$chk"
done
echo "---------------------------------------------------------------------------"
echo "The saving GROWS with architecture complexity (MLP 36% → Transformer 49%):"
echo "the more forward-wiring the DSL subsumes, the bigger the win. MAGE then lowers"
echo "the declaration to a byte-level binary IR — a further 34-42% under its own text."
echo "(Honest: this measures tokens to EXPRESS the architecture; the DSL leaves the"
echo " forward graph implicit, where PyTorch makes it explicit — that gap IS the saving.)"
rm -f /tmp/_n.abl 2>/dev/null
echo
echo "Reproduce token counts (real cl100k BPE), from this directory:"
echo "  cargo run -q -p agentic-eval --example tokens_of --features real-tokens -- *_hl.mg *_ll.mg *_mage.mg *_pytorch.py"
echo "  (run in nervosys/cli/AetherShell, passing absolute paths to these files)"
rm -f /tmp/_h.mg /tmp/_l.mg 2>/dev/null

echo
echo "=== Depth scaling — the 'stack N' repeat combinator (O(depth) -> O(1) surface) ==="
echo "A 12-deep transformer, written by hand (12× the block) vs \`stack 12 { block }\`."
echo "Tokens real cl100k (recorded); text + ABL bytes measured live."
# generate the manual 12× form
{ echo "net Manual {"; for i in $(seq 1 12); do
  printf '    layer attn%d: MultiHeadAttention(256, 8);\n    layer n%da: LayerNorm;\n    layer ff%da: Linear(256, 1024);\n    layer act%d: GELU;\n    layer ff%db: Linear(1024, 256);\n    layer n%db: LayerNorm;\n' $i $i $i $i $i $i
done; echo "    forward { attn1 }"; echo "}"; } > /tmp/_manual12.mg
"$MG" /tmp/_manual12.mg >/dev/null 2>&1 && mok=✓ || mok=FAIL
"$MG" deep_transformer_stack.mg >/dev/null 2>&1 && sok=✓ || sok=FAIL
# one-block form, for the binary O(1)-in-depth ratio
sed 's/stack 12/stack 1/' deep_transformer_stack.mg > /tmp/_one.mg
"$MG" --target=abl-bytes /tmp/_manual12.mg /tmp/_m.abl >/dev/null 2>&1
"$MG" --target=abl-bytes deep_transformer_stack.mg /tmp/_s.abl >/dev/null 2>&1
"$MG" --target=abl-bytes /tmp/_one.mg /tmp/_one.abl >/dev/null 2>&1
 b1=$(wc -c </tmp/_one.abl); b12=$(wc -c </tmp/_s.abl)
printf "  %-18s %6s %8s %8s   %s\n" "form" "tokens" "text" "ABL" "check"
printf "  %-18s %6s %7dB %7dB   %s\n" "manual 12×" "839" "$(wc -c </tmp/_manual12.mg)" "$(wc -c </tmp/_m.abl)" "$mok"
printf "  %-18s %6s %7dB %7dB   %s\n" "stack 12 { block }" "82" "$(wc -c <deep_transformer_stack.mg)" "$b12" "$sok"
printf "  %-18s %6s %8s %7dB\n" "(1 block, ref)" "" "" "$b1"
echo "  → surface: 82 vs 839 tokens (10.2× fewer), FLAT in depth (100 layers ≈ 83 tok)."
echo "  → ABL is now O(1) in depth too: the container REPEAT-folds at encode, so 12"
printf  "    blocks = %dB vs %dB for one = %s× (decodes to the full 72 layers; --run\n" \
        "$b12" "$b1" "$(awk "BEGIN{printf \"%.2f\", $b12/$b1}")"
echo "    dispatches all 72). See ARCHITECTURE_DSL.md §4.4."
rm -f /tmp/_manual12.mg /tmp/_m.abl /tmp/_s.abl /tmp/_one.mg /tmp/_one.abl 2>/dev/null
