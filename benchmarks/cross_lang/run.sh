#!/usr/bin/env bash
# Cross-language agentic-SWE EXECUTABILITY measurement. Every number below is
# produced by actually compiling+running the program and comparing real stdout
# to a known expected value — no judgments, no curated scores.
#
# Tasks (deterministic integer outputs):
#   fact(12)=479001600  sumto(100)=5050  fib(25)=75025  distinct=5  collatz(27)=111
set -u
cd "$(dirname "$0")"
MG=/c/Users/adamm/dev/nervosys/ai/MechGen/prototype/target/release/MechGen-parse.exe
EXPECT=(479001600 5050 75025 5 111)

# compare NAME <multiline-output> : compares 5 lines to EXPECT, prints the row.
compare() {
  local name="$1" out="$2"
  local -a lines; mapfile -t lines <<< "$out"
  local pass=0 row=""
  for i in 0 1 2 3 4; do
    local got="${lines[$i]:-<none>}"; got="${got//[$'\r']/}"
    if [ "$got" = "${EXPECT[$i]}" ]; then row+="  PASS"; pass=$((pass+1)); else row+="  FAIL"; fi
  done
  printf "%-12s %s    %d/5\n" "$name" "$row" "$pass"
}

mechgen_all() {
  "$MG" --eval tasks.mg fact 12
  "$MG" --eval tasks.mg sumto 100
  "$MG" --eval tasks.mg fib 25
  "$MG" --eval tasks.mg distinct
  "$MG" --eval tasks.mg collatz 27
}

echo "=== Cross-language agentic-SWE executability (MEASURED: real compile+run) ==="
echo "tasks: fact sumto fib distinct collatz   expected: ${EXPECT[*]}"
printf "%-12s %s    %s\n" "language" " f1    f2    f3    f4    f5" "pass"
echo "----------------------------------------------------------------"
compare MechGen    "$(mechgen_all 2>/dev/null)"
compare JavaScript "$(node tasks.js 2>/dev/null)"
compare TypeScript "$(bun tasks.ts 2>/dev/null)"
compare Go         "$(go run tasks.go 2>/dev/null)"
rustc -O tasks.rs -o tasks_rs.exe 2>/dev/null && compare Rust "$(./tasks_rs.exe 2>/dev/null)" || echo "Rust         compile/run FAILED"
javac Tasks.java 2>/dev/null && compare Java "$(java Tasks 2>/dev/null)" || echo "Java         compile/run FAILED"
echo "----------------------------------------------------------------"
echo "(Python: runtime not installed on this host — excluded, not estimated.)"
echo
echo "SOURCE SIZE (bytes — measured wc -c):"
for f in tasks.mg tasks.js tasks.ts tasks.go tasks.rs Tasks.java; do
  printf "  %-12s %5d\n" "$f" "$(wc -c < "$f")"
done
rm -f tasks_rs.exe Tasks.class 2>/dev/null
