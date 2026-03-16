# Collections

Beyond the built-in `[T]~` (Vec) and `{K:V}` (Map), the `std.col` module
provides additional data structures.

## Map (`{K: V}`)

```rdx
+f main() {
    m scores: {s: i32} = {s: i32}.new()
    scores.insert("Alice", 100)
    scores.insert("Bob", 95)

    // Lookup
    ? scores.get("Alice") => Some(score) {
        p"Alice: {score}"
    }

    // Iterate
    @ (name, score) : &scores {
        p"{name}: {score}"
    }

    // Check membership
    ? scores.contains_key("Charlie") {
        p"found"
    }
}
```

## Set (`{K}`)

```rdx
+f main() {
    m fruits: {s} = {s}.new()
    fruits.insert("apple")
    fruits.insert("banana")
    fruits.insert("apple")    // no duplicate

    p"Count: {fruits.len()}"  // 2

    // Set operations
    m more: {s} = {s}.new()
    more.insert("banana")
    more.insert("cherry")

    v both = fruits.intersection(&more)      // {"banana"}
    v all = fruits.union(&more)              // {"apple", "banana", "cherry"}
    v only_fruits = fruits.difference(&more) // {"apple"}
}
```

## BTree (ordered map)

```rdx
u std.col.BTree

+f main() {
    m tree: BTree[i32, s] = BTree.new()
    tree.insert(3, "three")
    tree.insert(1, "one")
    tree.insert(2, "two")

    // Iteration is in sorted order
    @ (k, v) : &tree {
        p"{k}: {v}"    // 1: one, 2: two, 3: three
    }

    // Range query
    @ (k, v) : tree.range(1..3) {
        p"{k}: {v}"    // 1: one, 2: two
    }
}
```

## VecDeque (double-ended queue)

```rdx
u std.col.VecDeque

+f main() {
    m dq: VecDeque[i32] = VecDeque.new()
    dq.push_back(1)
    dq.push_back(2)
    dq.push_front(0)

    v first = dq.pop_front()    // Some(0)
    v last = dq.pop_back()      // Some(2)
}
```

## Choosing the right collection

| Need             | Use        | Sugar    |
| ---------------- | ---------- | -------- |
| Ordered sequence | `Vec`      | `[T]~`   |
| Key-value lookup | `Map`      | `{K: V}` |
| Unique elements  | `Set`      | `{K}`    |
| Sorted key-value | `BTree`    | —        |
| Queue / deque    | `VecDeque` | —        |
| FIFO queue       | `VecDeque` | —        |
