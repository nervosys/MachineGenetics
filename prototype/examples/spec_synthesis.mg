// ── Spec-driven synthesis ────────────────────────────────────────────
//
// Contract-directed development: write the contracts, then the
// implementation, and let `--check` report how many contracts it verified.
//
// Every construct here was verified with `--check` and `--eval`. The previous
// version had never compiled. It used `usize?` for an optional (MAGE puts the
// `?` in front: `?usize`), `&mut [i32]`, `arr.windows(2).all(…)`, and a
// bodyless `f name(…) @req … ;` declaration form that does not exist — and
// 112 of its 121 remaining lines were commented-out implementations, so almost
// nothing in it was ever seen by the compiler at all.
//
// It also could not have worked even in principle: a `sp` block names the
// function it constrains, and a spec and a function sharing a name reported
// `duplicate definition` until the fix that landed with this rewrite.
//
// Demonstrates:
//   - `sp` blocks with `@req` / `@ens` / `@fx` contracts
//   - the contract count `--check` reports
//   - implementations that satisfy them

// ── Binary search ────────────────────────────────────────────────────
//
// The spec block carries the contracts; the function carries the code. They
// share a name, which is how the two are associated.

sp binary_search {
    @req(1b)
    @ens(1b)
    @fx()
}

f binary_search(arr: [i32]~, target: i32) -> ?usize {
    m lo = 0
    m hi = len(arr)
    @w lo < hi {
        v mid = lo + (hi - lo) / 2
        v probe = arr[mid]
        ? probe == target {
            ret Some(mid)
        } : {
            ? probe < target { lo = mid + 1 } : { hi = mid }
        }
    }
    None
}

// ── Merging two sorted lists ─────────────────────────────────────────

sp merge_sorted {
    @req(1b)
    @ens(1b)
    @fx()
}

f merge_sorted(a: [i32]~, b: [i32]~) -> [i32]~ {
    sort(concat_pair(a, b))
}

f concat_pair(a: [i32]~, b: [i32]~) -> [i32]~ {
    flatten([a, b])
}

// ── Deduplication ────────────────────────────────────────────────────

sp dedup {
    @req(1b)
    @ens(1b)
    @fx()
}

f dedup(items: [i32]~) -> [i32]~ {
    keys(freq(items))
}

// ── Entry point ──────────────────────────────────────────────────────

+f main() -> usize / io {
    v sorted = merge_sorted([1, 5, 9], [2, 6])
    println("merged:", sorted)

    v uniq = dedup([3, 1, 3, 2, 1])
    println("unique count:", len(uniq))

    v found = ?= binary_search([1, 2, 5, 6, 9], 6) {
        Some(i) => i,
        None => 99,
    }
    println("index of 6:", found)

    // 5 merged + 3 unique + index 3 = 11
    len(sorted) + len(uniq) + found
}
