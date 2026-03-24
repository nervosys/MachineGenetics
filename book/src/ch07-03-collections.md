# Collections

Beyond `Vec<T>` and `HashMap<K, V>`, the `std::col` module provides additional
data structures.

## HashMap

```mg
pub fn main() {
    let mut scores: HashMap<String, i32> = HashMap::new();
    scores.insert("Alice", 100);
    scores.insert("Bob", 95);

    // Lookup
    if let Some(score) = scores.get("Alice") {
        println!("Alice: {score}");
    }

    // Iterate
    for (name, score) in &scores {
        println!("{name}: {score}");
    }

    // Check membership
    if scores.contains_key("Charlie") {
        println!("found");
    }
}
```

## HashSet

```mg
pub fn main() {
    let mut fruits: HashSet<String> = HashSet::new();
    fruits.insert("apple");
    fruits.insert("banana");
    fruits.insert("apple");    // no duplicate

    println!("Count: {}", fruits.len());  // 2

    // Set operations
    let mut more: HashSet<String> = HashSet::new();
    more.insert("banana");
    more.insert("cherry");

    let both = fruits.intersection(&more);      // {"banana"}
    let all = fruits.union(&more);              // {"apple", "banana", "cherry"}
    let only_fruits = fruits.difference(&more); // {"apple"}
}
```

## BTreeMap (ordered map)

```mg
use std::col::BTreeMap;

pub fn main() {
    let mut tree: BTreeMap<i32, String> = BTreeMap::new();
    tree.insert(3, "three");
    tree.insert(1, "one");
    tree.insert(2, "two");

    // Iteration is in sorted order
    for (k, v) in &tree {
        println!("{k}: {v}");    // 1: one, 2: two, 3: three
    }

    // Range query
    for (k, v) in tree.range(1..3) {
        println!("{k}: {v}");    // 1: one, 2: two
    }
}
```

## VecDeque (double-ended queue)

```mg
use std::col::VecDeque;

pub fn main() {
    let mut dq: VecDeque<i32> = VecDeque::new();
    dq.push_back(1);
    dq.push_back(2);
    dq.push_front(0);

    let first = dq.pop_front();    // Some(0)
    let last = dq.pop_back();      // Some(2)
}
```

## Choosing the right collection

| Need             | Use        | Type             |
| ---------------- | ---------- | ---------------- |
| Ordered sequence | `Vec`      | `Vec<T>`         |
| Key-value lookup | `HashMap`  | `HashMap<K, V>`  |
| Unique elements  | `HashSet`  | `HashSet<K>`     |
| Sorted key-value | `BTreeMap` | `BTreeMap<K, V>` |
| Queue / deque    | `VecDeque` | `VecDeque<T>`    |
| FIFO queue       | `VecDeque` | `VecDeque<T>`    |
