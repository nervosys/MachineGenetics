# Chapter 4: Type System Migration

Migrate Rust's type system to MechGen: type sugar, generic syntax, lifetime
removal, trait bounds, and smart pointer conversions.

---

## 4.1 Type Sugar Conversions

MechGen provides compact sugar for the most common standard library types:

### Standard Types

```diff
  // String types
- fn greet(name: &str) -> String {
+ f greet(name: &s) -> s {

  // Collection types
- fn process(items: Vec<i32>) -> Vec<String> {
+ f process(items: [i32]~) -> [s]~ {

  // Option / Result
- fn find(id: u64) -> Option<User> {
+ f find(id: u64) -> ?User {

- fn load(path: &str) -> Result<Config, io::Error> {
+ f load(path: &s) -> R[Config, io.Error] / io {

  // Smart pointers
- fn boxed(x: i32) -> Box<dyn Display> {
+ f boxed(x: i32) -> ^dyn Display {

- fn shared(data: String) -> Rc<String> {
+ f shared(data: s) -> $s {

- fn atomic(data: String) -> Arc<String> {
+ f atomic(data: s) -> @s {

  // Map types
- fn index() -> HashMap<String, Vec<u32>> {
+ f index() -> {s: [u32]~} {

- fn unique() -> HashSet<String> {
+ f unique() -> {s} {

  // Mutable references
- fn mutate(data: &mut Vec<i32>) {
+ f mutate(data: &![i32]~) {
```

### Complete Type Mapping Table

| Rust            | MechGen     | Nesting Example                                   |
| --------------- | --------- | ------------------------------------------------- |
| `String`        | `s`       |                                                   |
| `&str`          | `&s`      |                                                   |
| `Vec<T>`        | `[T]~`    | `Vec<Vec<i32>>` → `[[i32]~]~`                     |
| `Option<T>`     | `?T`      | `Option<Vec<T>>` → `?[T]~`                        |
| `Result<T, E>`  | `R[T, E]` | `Result<Vec<T>, io::Error>` → `R[[T]~, io.Error]` |
| `Box<T>`        | `^T`      | `Box<dyn Trait>` → `^dyn Trait`                   |
| `Rc<T>`         | `$T`      | `Rc<RefCell<T>>` → `$RefCell[T]`                  |
| `Arc<T>`        | `@T`      | `Arc<Mutex<T>>` → `@Mutex[T]`                     |
| `HashMap<K, V>` | `{K: V}`  | `HashMap<String, Vec<i32>>` → `{s: [i32]~}`       |
| `HashSet<K>`    | `{K}`     | `HashSet<String>` → `{s}`                         |
| `&mut T`        | `&!T`     | `&mut Vec<String>` → `&![s]~`                     |

### Deeply Nested Types

```diff
  // Complex nested types
- HashMap<String, Vec<Option<Box<dyn Handler>>>>
+ {s: [?^dyn Handler]~}

- Result<Arc<Mutex<HashMap<String, Vec<u8>>>>, io::Error>
+ R[@Mutex[{s: [u8]~}], io.Error]

  // Function that returns nested optional
- fn find_first(data: &HashMap<String, Vec<User>>) -> Option<&User>
+ f find_first(data: &{s: [User]~}) -> ?&User
```

## 4.2 Removing Lifetime Annotations

MechGen's SKB (Semantic Knowledge Base) infers and proves lifetimes automatically.
Remove all lifetime parameters during migration.

### Simple Cases

```diff
  // Lifetime on references
- fn longest<'a>(a: &'a str, b: &'a str) -> &'a str {
+ f longest(a: &s, b: &s) -> &s {

  // Lifetime on structs
- struct Parser<'a> {
-     input: &'a str,
- }
+ S Parser {
+     input: &s,
+ }

  // Lifetime on impl blocks
- impl<'a> Parser<'a> {
-     fn new(input: &'a str) -> Self {
+ I ~ Parser {
+     f new(input: &s) -> Self {
```

### Complex Cases

```diff
  // Multiple lifetimes
- fn merge<'a, 'b>(a: &'a [u8], b: &'b [u8]) -> Vec<u8> {
+ f merge(a: &[u8], b: &[u8]) -> [u8]~ {

  // Lifetime bounds
- struct Wrapper<'a, T: 'a> {
-     data: &'a T,
- }
+ S Wrapper[T] {
+     data: &T,
+ }

  // Static lifetime
- fn constant() -> &'static str {
+ f constant() -> &s {
      "hello"
  }

  // Lifetime in trait bounds
- fn process<'a, T: AsRef<str> + 'a>(item: &'a T) -> &'a str {
+ f process[T: AsRef[s]](item: &T) -> &s {
```

### What the SKB Does

The SKB automatically:
1. Infers lifetime relationships from data flow
2. Proves that references don't outlive their data
3. Generates compile errors if safety can't be proven
4. Eliminates the need for `PhantomData`, `Pin`, or `ManuallyDrop`

If the SKB can't prove safety, it reports a clear error:

```
error[SKB001]: cannot prove reference safety
  --> src/lib.mg:42:5
   |
42 |     ret &self.data[idx]
   |         ^^^^^^^^^^^^^^^ reference may outlive container
   |
help: consider returning an owned value instead
   |
42 |     ret self.data[idx].clone()
```

## 4.3 Generic Syntax Migration

### Angle Brackets to Square Brackets

```diff
  // Type parameters
- struct Container<T> { item: T }
+ S Container[T] { item: T }

  // Multiple type parameters
- enum Either<A, B> { Left(A), Right(B) }
+ E Either[A, B] { Left(A), Right(B) }

  // Bounded generics
- fn smallest<T: Ord>(list: &[T]) -> &T {
+ f smallest[T: Ord](list: &[T]) -> &T {

  // Where clause
- fn complex<T, U>(a: T, b: U)
- where
-     T: Clone + Debug,
-     U: Into<String>,
- {
+ f complex[T, U](a: T, b: U)
+     ~> T: Clone + Debug,
+        U: Into[s]
+ {
```

### Turbofish Removal

```diff
  // Explicit type parameters on calls
- let parsed = "42".parse::<i32>()?;
+ v parsed = "42".parse[i32]()?

- let collected: Vec<i32> = iter.collect();
+ v collected: [i32]~ = iter.collect()

  // Or with explicit type on collect
- let items = iter.collect::<Vec<String>>();
+ v items = iter.collect[[s]~]()
```

## 4.4 Trait Bound Migration

### Inline Bounds

```diff
- fn print_it<T: Display>(item: &T) {
+ f print_it[T: Display](item: &T) / io {
      p"{item}"
  }

  // Multiple bounds
- fn process<T: Clone + Debug + Send>(item: T) {
+ f process[T: Clone + Debug + Send](item: T) {
```

### Where Clauses

```diff
- fn transform<I, O>(input: I) -> O
- where
-     I: IntoIterator<Item = u8>,
-     O: FromIterator<u8>,
+ f transform[I, O](input: I) -> O
+     ~> I: IntoIterator[Item = u8],
+        O: FromIterator[u8]
```

### Associated Types

```diff
  // In trait definitions
- trait Collection {
-     type Item;
-     fn get(&self, idx: usize) -> Option<&Self::Item>;
- }
+ T Collection {
+     type Item
+     f get(&self, idx: usize) -> ?&Self.Item
+ }

  // In bounds
- fn first<C: Collection<Item = u32>>(c: &C) -> Option<&u32> {
+ f first[C: Collection[Item = u32]](c: &C) -> ?&u32 {
```

## 4.5 Enum Migration Patterns

### Simple Enums

```diff
- pub enum Direction { North, South, East, West }
+ +E Direction { North, South, East, West }
```

### Data-Carrying Enums

```diff
- pub enum Message {
-     Quit,
-     Move { x: i32, y: i32 },
-     Write(String),
-     Color(u8, u8, u8),
- }
+ +E Message {
+     Quit,
+     Move { x: i32, y: i32 },
+     Write(s),
+     Color(u8, u8, u8),
+ }
```

### Pattern Matching on Enums

```diff
- match msg {
-     Message::Quit => println!("quit"),
-     Message::Move { x, y } => println!("move to ({}, {})", x, y),
-     Message::Write(text) => println!("{}", text),
-     Message::Color(r, g, b) => println!("color: {},{},{}", r, g, b),
- }
+ ? msg {
+     Message.Quit => p"quit",
+     Message.Move { x, y } => p"move to ({x}, {y})",
+     Message.Write(text) => p"{text}",
+     Message.Color(r, g, b) => p"color: {r},{g},{b}",
+ }
```

## 4.6 Common Type Migration Pitfalls

### Pitfall 1: Nested Option/Result

```diff
  // WRONG — double ?
- Option<Option<T>>  →  ??T      // Ambiguous!
+ Option<Option<T>>  →  ?(?T)    // Parenthesized for clarity
```

### Pitfall 2: References to Sugar Types

```diff
  // WRONG
- &Vec<T>  →  &[T]~    // This means &(Vec<T>), correct

  // The reference to a slice is different
- &[T]     →  &[T]     // Slice reference, same in both
```

### Pitfall 3: Forgetting `@` in Struct Literals

```diff
  // WRONG
  v point = Point { x: 1.0, y: 2.0 }

  // CORRECT
  v point = Point @{ x: 1.0, y: 2.0 }
```

### Pitfall 4: Path Separator in Types

```diff
  // WRONG — using :: in type paths
  v map: std::collections::HashMap[s, i32]

  // CORRECT
  v map: std.col.HashMap[s, i32]
  // Or just use the sugar:
  v map: {s: i32}
```
