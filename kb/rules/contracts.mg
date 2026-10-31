// What @req, @ens and @inv mean, and what checks each of them.
//
// A clause can stand in four places: on a function, in a `sp` that shares a
// top-level function's name, in a `sp` with no such function, or on a struct.
// Four components can check it: `types` (it is a bool), `contract` (`result` and
// `old` are in scope), `verify` (it is not refuted) and `eval` (it holds when
// the function runs). `clause_checked` states each cell of that grid that is
// checked. Every other cell is unchecked, and that is a statement too:
// prototype/tests/rules.rs probes all 48 cells against the compiler, both ways.
//
// `verify` refutes very little: the literal `0b`, the text `false` (which is not
// a MAGE literal: it is also an unresolved name), or `path.len() < 0`.

kb Contracts {
    fact instance_of("contract", SymbolicComponent);
    fact instance_of("eval", SymbolicComponent);

    fact instance_of("requires", Contract);
    fact contract_row("requires", "@req", "Must hold on entry; the caller's obligation");
    fact instance_of("ensures", Contract);
    fact contract_row("ensures", "@ens", "Must hold on exit; may name `result` and `old(param)`");
    fact instance_of("invariant", Contract);
    fact contract_row("invariant", "@inv", "Must always hold of a struct's values");

    fact clause_checked("requires", "function", "types");
    fact clause_checked("requires", "function", "contract");
    fact clause_checked("requires", "function", "verify");
    fact clause_checked("requires", "function", "eval");
    fact clause_checked("requires", "spec", "types");
    fact clause_checked("requires", "spec", "contract");
    fact clause_checked("requires", "spec", "verify");
    fact clause_checked("requires", "spec", "eval");
    fact clause_checked("requires", "orphan_spec", "contract");
    fact clause_checked("requires", "orphan_spec", "verify");

    fact clause_checked("ensures", "function", "types");
    fact clause_checked("ensures", "function", "contract");
    fact clause_checked("ensures", "function", "verify");
    fact clause_checked("ensures", "function", "eval");
    fact clause_checked("ensures", "spec", "types");
    fact clause_checked("ensures", "spec", "contract");
    fact clause_checked("ensures", "spec", "verify");
    fact clause_checked("ensures", "spec", "eval");
    fact clause_checked("ensures", "orphan_spec", "verify");

    fact clause_checked("invariant", "function", "types");
    fact clause_checked("invariant", "function", "contract");
    fact clause_checked("invariant", "spec", "types");
    fact clause_checked("invariant", "spec", "contract");
    fact clause_checked("invariant", "orphan_spec", "contract");
    fact clause_checked("invariant", "struct", "verify");

    fact gap("invariant", "no @inv is checked at run time; on a struct it is not even typechecked");
    fact gap("requires", "@req and @ens on a struct are accepted and checked by nothing");
    fact gap("ensures", "in a spec with no function, @ens is not typechecked and its old(x) not scoped: there is no signature");
}
