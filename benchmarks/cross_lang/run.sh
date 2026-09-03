#!/usr/bin/env bash
# Cross-language agentic-SWE EXECUTABILITY measurement. Every number below is
# produced by actually compiling+running the program and comparing real stdout
# to a known expected value — no judgments, no curated scores.
#
# Tasks (deterministic integer outputs):
#   fact(12)=479001600  sumto(100)=5050  fib(25)=75025  distinct=5  collatz(27)=111
set -u
cd "$(dirname "$0")"
# MAGE evaluator binary: env override, else the repo-relative release build
# (this script lives in <repo>/benchmarks/cross_lang). Set MG to point elsewhere.
# The default names the Windows binary; everywhere else, strip the suffix.
#
# This *appended* `.exe` instead of stripping it -- the comment said "tolerate
# the .exe suffix on non-Windows checkouts" and the code did the opposite, so on
# Linux it looked for `mage-parse.exe.exe`. With no binary `mage_all` produces
# nothing and `compare` scores it 0/5: this benchmark would have published
# "MAGE fails all five of its own tasks". The identical line was wrong in
# `constructs/run.sh` and right in `capstone/run.sh`; this is the third copy.
MG="${MG:-../../prototype/target/release/mage-parse.exe}"
[ -x "$MG" ] || MG="${MG%.exe}"
if [ ! -x "$MG" ]; then
  echo "missing binary: ${MG##*/} -- build with: cargo build --release --manifest-path prototype/Cargo.toml" >&2
  exit 1
fi
EXPECT=(479001600 5050 75025 5 111)

# have CMD : is this toolchain installed at all?
#
# Absent is not the same as failing, and this script could not tell them apart.
# A missing `node` made the command substitution empty and `compare` scored it
# 0/5 -- reported as five wrong answers from a runtime that never ran. A missing
# `rustc` printed "compile/run FAILED", which reads as a compiler that ran.
# The executability table is the front page's "5/5" claim, so a machine without
# a toolchain silently turned it into a table about the machine.
have() { command -v "$1" >/dev/null 2>&1; }
absent() { printf "%-12s   %-28s (not measured)
" "$1" "$2 not installed"; }

# A language that is present and gets the wrong answer is a failure; a language
# that is absent is not. Counted here, and acted on at the end of the script --
# without that, this prints a table of FAILs and reports success, which is what
# `constructs/run.sh` did until today and the reason none of these three scripts
# could be wired into CI honestly.
measured=0
wrong=0
mage_measured=""

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
  measured=$((measured + 1))
  [ "$pass" -eq 5 ] || wrong=$((wrong + 1))
}

mage_all() {
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
compare MAGE    "$(mage_all 2>/dev/null)"; mage_measured=yes
have node  && compare JavaScript "$(node tasks.js 2>/dev/null)"   || absent JavaScript node
have bun   && compare TypeScript "$(bun tasks.ts 2>/dev/null)"    || absent TypeScript bun
# Compiled as its own step, like Rust and Java. `go run` compiles *and* runs,
# and on a cold build cache the first invocation produced no stdout in time --
# scored 0/5, i.e. "Go failed all five tasks", from a toolchain that works. The
# second run of the same script passed. A benchmark that reports a cold cache as
# a language failure is worse than one that does not run.
if have go; then
  if go build -o tasks_go.exe tasks.go 2>/dev/null; then
    compare Go "$(./tasks_go.exe 2>/dev/null)"
  else
    echo "Go           compile FAILED (go present)"
    measured=$((measured + 1)); wrong=$((wrong + 1))
  fi
else
  absent Go go
fi
if have rustc; then
  if rustc -O tasks.rs -o tasks_rs.exe 2>/dev/null; then
    compare Rust "$(./tasks_rs.exe 2>/dev/null)"
  else
    echo "Rust         compile FAILED (rustc present)"
    measured=$((measured + 1)); wrong=$((wrong + 1))
  fi
else
  absent Rust rustc
fi
if have javac && have java; then
  if javac Tasks.java 2>/dev/null; then
    compare Java "$(java Tasks 2>/dev/null)"
  else
    echo "Java         compile FAILED (javac present)"
    measured=$((measured + 1)); wrong=$((wrong + 1))
  fi
else
  absent Java javac
fi
# There is no `tasks.py`. This suite has never had a Python program, which is
# the real reason Python is absent from the table -- the previous version said
# "runtime not installed on this host", unconditionally, and that was false even
# when written: python3 is present here. Excluded for the honest reason, and it
# will start measuring itself the day someone adds the file.
if [ ! -f tasks.py ]; then
  printf "%-12s   %-28s (not measured)
" "Python" "no tasks.py in this suite"
elif have python3 || have python; then
  compare Python "$( { have python3 && python3 tasks.py; } 2>/dev/null || python tasks.py 2>/dev/null)"
else
  absent Python python3
fi
echo "----------------------------------------------------------------"
echo
echo "SOURCE SIZE (bytes — measured wc -c):"
for f in tasks.mg tasks.js tasks.ts tasks.go tasks.rs Tasks.java; do
  printf "  %-12s %5d\n" "$f" "$(wc -c < "$f")"
done
rm -f tasks_rs.exe tasks_go.exe Tasks.class tasks_rs.pdb 2>/dev/null

# ── Verdict ─────────────────────────────────────────────────────────────
#
# MAGE is the one language whose toolchain lives in this repository, so it is
# the one that must always be measured. If it is missing, this benchmark is not
# measuring the thing it exists to measure, and a "5/5" about the other five
# would be beside the point.
if [ -z "$mage_measured" ]; then
  echo "MAGE was not measured; this benchmark has nothing to say without it" >&2
  exit 1
fi
if [ "$wrong" -ne 0 ]; then
  echo >&2
  echo "$wrong of $measured measured language(s) did not produce the expected output." >&2
  echo "A language absent from this host reads as 'not measured' and is not counted here," >&2
  echo "so this is a real disagreement, not a missing toolchain." >&2
  exit 1
fi
