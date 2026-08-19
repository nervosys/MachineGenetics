# Data Processing

> Recipes for collections, maps and text. Agent-mode syntax; every block was
> verified with `mage-parse --check`, and the ones with a `main` were run —
> the answers in the discussions are what they printed.

The previous version of this page was Rust with sigils: `.iter().filter(…)
.map(…).collect()`, `staff.sort_by(…)`, `groups.entry(k).or_default().push(…)`,
`{s: usize}.new()`, `[T]~.new()`, `K: Eq + Hash` bounds. **None of it exists,
and none of it is needed** — the 31-word standard vocabulary is the collection
library:

```
map filter fold reduce sum len count sort reverse zip freq first last any all
find take range keys values flatten group scan contains split join chars words
lines upper lower
```

They are global functions, not methods, and they nest innermost-first.

---

### Parse and query JSON

**Problem**: Load a JSON file and extract fields.

**Solution**:

```MAGE
// `json` is a capability namespace, and it deliberately attributes no effect
// — no built-in kind names a document store. Parsing is pure; the *reading*
// is what needs `fs`.
+S User { name: s, age: i32 }

+f users_over(path: s, min_age: i32) -> [s]~ / fs {
    v text = fs.read_to_string(path)
    v rows: [s]~ = json.parse(text)
    filter(rows, |row| contains(row, "age"))
}
```

**Discussion**: `json` is a capability namespace that **deliberately attributes no effect** — no built-in kind names a document store, and inventing one would infer an effect no annotation could then declare. Reading the file is what needs `fs`. A capability call returns a type the checker does not know, so annotate the binding.

---

### Sort a collection

**Problem**: Order items by a field.

**Solution**:

```MAGE
+S Employee { name: s, salary: f64 }

// `sort` takes one argument and orders by natural order. To sort *by a
// field*, sort the keys and look the values back up — there is no `sort_by`.
+f names_by_salary(staff: [Employee]~) -> [s]~ {
    v salaries = sort(map(staff, |e| e.salary))
    map(salaries, |wanted| name_at(staff, wanted))
}

f name_at(staff: [Employee]~, wanted: f64) -> s {
    ?= first(filter(staff, |e| e.salary == wanted)) {
        Some(found) => found.name,
        None => "",
    }
}

+f main() -> [s]~ {
    names_by_salary([
        @Employee { name: "Alice", salary: 95000.0 },
        @Employee { name: "Bob", salary: 87000.0 },
        @Employee { name: "Charlie", salary: 102000.0 },
    ])
}
```

**Discussion**: `sort` takes one argument and uses natural order. There is no `sort_by` and no comparator, so sorting by a field means sorting the keys and looking the values back up.

---

### Group items by key

**Problem**: Group a list into buckets by a shared key.

**Solution**:

```MAGE
// `group` is in the standard vocabulary: it takes a list and a key function
// and returns `{K: [T]~}`. There is no `entry(…).or_default()`.
+f by_first_letter(words: [s]~) -> {s: [s]~} {
    group(words, |word| chars(word)[0])
}

+f main() -> i32 / io {
    v groups = by_first_letter(["apple", "banana", "avocado", "blueberry"])
    @ letter in sort(keys(groups)) {
        p"{letter}: {len(groups[letter])} words"
    }
    len(keys(groups)) as i32
}
```

**Discussion**: `group(items, key_fn)` returns `{K: [T]~}` — it is one of the 31 vocabulary words, and it replaces the `entry(…).or_default().push` dance entirely.

---

### Filter and transform

**Problem**: Apply a pipeline of filters and maps.

**Solution**:

```MAGE
// A pipeline is nested vocabulary calls, innermost first. There is no method
// chain, no `.iter()` and no `.collect()`.
+f main() -> [i32]~ / io {
    v numbers = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]

    v result = filter(
        map(
            filter(numbers, |n| n % 2 == 0),   // keep evens
            |n| n * n,                         // square them
        ),
        |n| n > 10,                            // keep > 10
    )

    p"result: {result}"
    result
}
```

**Discussion**: A pipeline is nested calls, innermost first. There is no method chain, no `.iter()`, no `.collect()`. Evaluates to `[16, 36, 64, 100]`.

---

### Count word frequencies

**Problem**: Count how many times each word appears in a text.

**Solution**:

```MAGE
// `freq` counts occurrences directly — `words` splits the text, `freq`
// returns `{s: usize}`.
+f word_freq(text: s) -> {s: usize} {
    freq(words(lower(text)))
}

+f top_word(text: s) -> s {
    v counts = word_freq(text)
    v ordered = sort(keys(counts))
    ?= first(ordered) {
        Some(word) => word,
        None => "",
    }
}
```

**Discussion**: `freq` returns `{T: usize}` directly, over anything `words`, `lines` or `chars` produced.

---

### Deduplicate a list

**Problem**: Remove duplicates while preserving order.

**Solution**:

```MAGE
// Deduplicate by folding: keep an item only when it has not been seen. There
// is no set literal and no `.insert` returning a bool, so the map's keys are
// the set.
+f dedup(items: [i32]~) -> [i32]~ {
    m seen = {0: 0b}
    m result: [i32]~ = []
    @ item in items {
        ? !contains(keys(seen), item) {
            seen[item] = 1b
            result = flatten([result, [item]])
        }
    }
    result
}

+f main() -> [i32]~ {
    dedup([3, 1, 4, 1, 5, 9, 2, 6, 5, 3])
}
```

**Discussion**: There is no set literal and no `.insert` that reports novelty, so a map's keys are the set. Evaluates to `[3, 1, 4, 5, 9, 2, 6]`.

---

### Flatten a nested structure

**Problem**: Convert a tree into a flat list.

**Solution**:

```MAGE
// `flatten` in the vocabulary removes one level of nesting. For a recursive
// structure, recurse — but note a generic recursive sum is more than the
// checker can follow today, so this is the concrete case.
+E Tree {
    Leaf(i32),
    Node([Tree]~),
}

+f flat(tree: Tree) -> [i32]~ {
    ?= tree {
        Leaf(value) => [value],
        Node(children) => flatten(map(children, |child| flat(child))),
    }
}

+f main() -> [i32]~ {
    flat(Node([Leaf(1), Node([Leaf(2), Leaf(3)]), Leaf(4)]))
}
```

**Discussion**: The vocabulary's `flatten` removes one level; a recursive structure recurses. A *generic* recursive sum is more than the checker follows today, so this is the concrete case.

---

### Running statistics

**Problem**: Compute mean and variance over a series of values.

**Solution**:

```MAGE
+S Stats { count: f64, total: f64, total_sq: f64 }

// No `&!self`: a method returns the updated value, so the accumulator is
// threaded rather than mutated.
extend Stats {
    +f push(self, value: f64) -> Stats {
        @Stats {
            count: self.count + 1.0,
            total: self.total + value,
            total_sq: self.total_sq + value * value,
        }
    }

    +f mean(self) -> f64 { self.total / self.count }

    +f variance(self) -> f64 {
        (self.total_sq / self.count) - (self.mean() * self.mean())
    }
}

+f main() -> f64 {
    v stats = fold(
        [2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0],
        @Stats { count: 0.0, total: 0.0, total_sq: 0.0 },
        |acc, x| acc.push(x),
    )
    stats.variance()
}
```

**Discussion**: No `&!self`: a method returns the updated value, so the accumulator is threaded through `fold` rather than mutated in place. The invalid intermediate state never exists.

---
