# Data Processing

---

### Parse and query JSON

**Problem**: Load a JSON file and extract specific fields.

**Solution**:

```rdx
u std.fs
u std.json.{parse, Value}

+f main() / io {
    v text = fs.read("users.json")?
    v data = parse(&text)?

    // data is a Value.Array
    v users = data.as_array()?
    @ user : users {
        v name = user.get("name").as_str()?
        v age = user.get("age").as_int()?
        ? age > 30 {
            p"{name} (age {age})"
        }
    }
}
```

---

### Sort a collection

**Problem**: Sort items by a specific field.

**Solution**:

```rdx
@d(Debug, Clone)
S Employee { name: s, salary: f64 }

+f main() {
    m staff = [
        Employee @{ name: "Alice".into(), salary: 95_000.0 },
        Employee @{ name: "Bob".into(), salary: 87_000.0 },
        Employee @{ name: "Charlie".into(), salary: 102_000.0 },
    ]~

    // Sort by salary descending
    staff.sort_by(|a, b| b.salary.partial_cmp(&a.salary).unwrap())

    @ e : &staff {
        p"{e.name}: ${e.salary}"
    }
}
```

---

### Group items by key

**Problem**: Group a list of items into buckets by a shared key.

**Solution**:

```rdx
+f group_by[T, K: Eq + Hash](items: &[T]~, key_fn: f(&T) -> K) -> {K: [&T]~} {
    m groups: {K: [&T]~} = {K: [&T]~}.new()
    @ item : items {
        v k = key_fn(item)
        groups.entry(k).or_default().push(item)
    }
    groups
}

// Usage
+f main() / io {
    v words = ["apple", "banana", "avocado", "blueberry", "cherry"]~
    v by_first = group_by(&words, |w| w.chars().next().unwrap())

    @ (letter, group) : &by_first {
        p"{letter}: {group.len()} words"
    }
}
```

---

### Filter and transform

**Problem**: Apply a pipeline of filter and map operations.

**Solution**:

```rdx
+f main() / io {
    v numbers = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]~

    v result: [i32]~ = numbers.iter()
        .filter(|n| *n % 2 == 0)    // keep evens
        .map(|n| n * n)             // square them
        .filter(|n| *n > 10)        // keep > 10
        .collect()

    p"Result: {result}"  // [16, 36, 64, 100]
}
```

---

### Count word frequencies

**Problem**: Count how many times each word appears in text.

**Solution**:

```rdx
+f word_freq(text: &s) -> {s: usize} {
    m counts: {s: usize} = {s: usize}.new()
    @ word : text.split_whitespace() {
        v w = word.to_lowercase()
        *counts.entry(w).or_insert(0) += 1
    }
    counts
}

+f main() / io {
    v text = "the quick brown fox jumps over the lazy dog the fox"
    v freq = word_freq(text)

    // Sort by frequency descending
    m pairs: [(&s, &usize)]~ = freq.iter().collect()
    pairs.sort_by(|a, b| b.1.cmp(a.1))

    @ (word, count) : &pairs {
        p"{word}: {count}"
    }
}
```

---

### Deduplicate a list

**Problem**: Remove duplicate items while preserving order.

**Solution**:

```rdx
+f dedup[T: Eq + Hash + Clone](items: &[T]~) -> [T]~ {
    m seen: {&T} = {&T}.new()
    m result = [T]~.new()
    @ item : items {
        ? seen.insert(item) {
            result.push(item.clone())
        }
    }
    result
}

+f main() / io {
    v data = [3, 1, 4, 1, 5, 9, 2, 6, 5, 3]~
    v unique = dedup(&data)
    p"Unique: {unique}"  // [3, 1, 4, 5, 9, 2, 6]
}
```

---

### Flatten nested structures

**Problem**: Convert a nested tree into a flat list.

**Solution**:

```rdx
E Tree[T] {
    Leaf(T),
    Node([Tree[T]]~),
}

+f flatten[T: Clone](tree: &Tree[T]) -> [T]~ {
    ? tree {
        Tree.Leaf(v) => [v.clone()]~,
        Tree.Node(children) => {
            m result = [T]~.new()
            @ child : children {
                result.extend(flatten(child))
            }
            result
        },
    }
}

+f main() / io {
    v tree = Tree.Node([
        Tree.Leaf(1),
        Tree.Node([Tree.Leaf(2), Tree.Leaf(3)]~),
        Tree.Leaf(4),
    ]~)

    v flat = flatten(&tree)
    p"Flat: {flat}"  // [1, 2, 3, 4]
}
```

---

### Running statistics

**Problem**: Compute mean and standard deviation over a stream of values.

**Solution**:

```rdx
S Stats {
    count: u64,
    sum: f64,
    sum_sq: f64,
}

I ~ Stats {
    +f new() -> Self {
        Stats @{ count: 0, sum: 0.0, sum_sq: 0.0 }
    }

    +f push(&!self, value: f64) {
        self.count += 1
        self.sum += value
        self.sum_sq += value * value
    }

    +f mean(&self) -> f64 {
        self.sum / self.count as f64
    }

    +f std_dev(&self) -> f64 {
        v n = self.count as f64
        v variance = (self.sum_sq / n) - (self.mean() * self.mean())
        variance.sqrt()
    }
}

+f main() / io {
    m stats = Stats.new()
    v data = [2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0]~
    @ v : &data { stats.push(*v) }
    p"Mean: {stats.mean():.2}"       // 5.00
    p"Std dev: {stats.std_dev():.2}" // 2.00
}
```
