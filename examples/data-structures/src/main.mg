// data-structures — Data types, extensions, generics, pattern matching.
//
// Demonstrates:
//   - data keyword for records and sums (replaces struct/enum for simple types)
//   - struct/enum still available for complex cases (derive, named fields)
//   - extend keyword (replaces impl)
//   - Trait definitions (pub trait)
//   - val / var (replaces let / let mut)
//   - guard for early exit
//   - is pattern (pattern test expression)
//   - T or E (error union)
//   - Expression-body functions

// ── Data types (concise form) ────────────────────────────────────────

// Records: positional fields with named access.
data Point[T](pub x: T, pub y: T)

// Sums: algebraic data types.
data Color = Red | Green | Blue | Custom(u8, u8, u8)

// ── Structs (when you need derive or named enum fields) ──────────────

#[derive(Debug)]
pub struct Person {
    name: String,
    age: u32,
    email: ?String,
}

// Named enum fields still use struct-style syntax.
#[derive(Debug)]
pub enum Shape {
    Circle { radius: f64 },
    Rectangle { width: f64, height: f64 },
    Triangle { base: f64, height: f64 },
}

// ── Traits ───────────────────────────────────────────────────────────

pub trait Area {
    fn area(&self) -> f64;
}

pub trait Describe {
    fn describe(&self) -> String;

    // Default method.
    fn summary(&self) -> String {
        format!("Object: {self.describe()}")
    }
}

// ── Extend blocks (replace impl) ────────────────────────────────────

extend Point[f64] {
    pub fn origin() -> Point[f64] = Point { x: 0.0, y: 0.0 }

    pub fn distance(&self, other: &Point[f64]) -> f64 {
        val dx = self.x - other.x;
        val dy = self.y - other.y;
        (dx * dx + dy * dy).sqrt()
    }

    pub fn translate(&mut self, dx: f64, dy: f64) {
        self.x = self.x + dx;
        self.y = self.y + dy;
    }
}

extend Shape {
    fn area(&self) -> f64 {
        match self {
            Shape::Circle { radius } => std::f64::consts::PI * radius * radius,
            Shape::Rectangle { width, height } => width * height,
            Shape::Triangle { base, height } => 0.5 * base * height,
        }
    }

    fn describe(&self) -> String {
        match self {
            Shape::Circle { radius } => format!("Circle with radius {radius}"),
            Shape::Rectangle { width, height } => format!("Rectangle {width}x{height}"),
            Shape::Triangle { base, height } => format!("Triangle base={base} h={height}"),
        }
    }
}

extend Color {
    fn describe(&self) -> String {
        match self {
            Color::Red => "Red".to_string(),
            Color::Green => "Green".to_string(),
            Color::Blue => "Blue".to_string(),
            Color::Custom(r, g, b) => format!("RGB({r}, {g}, {b})"),
        }
    }
}

// ── Generic container ────────────────────────────────────────────────

#[derive(Debug)]
pub struct Stack[T] {
    items: [T]~,
}

extend Stack[T] {
    pub fn new() -> Stack[T] = Stack { items: [T]~.new() }

    pub fn push(&mut self, item: T) {
        self.items.push(item);
    }

    pub fn pop(&mut self) -> ?T = self.items.pop()

    pub fn peek(&self) -> ?&T = self.items.last()

    pub fn len(&self) -> usize = self.items.len()

    pub fn is_empty(&self) -> bool = self.items.is_empty()
}

// ── Smart pointer examples ───────────────────────────────────────────

#[derive(Debug)]
struct TreeNode {
    value: i32,
    left: ?^TreeNode,
    right: ?^TreeNode,
}

extend TreeNode {
    fn leaf(value: i32) -> TreeNode {
        TreeNode { value: value, left: None, right: None }
    }

    fn branch(value: i32, left: TreeNode, right: TreeNode) -> TreeNode {
        TreeNode {
            value: value,
            left: Some(Box.new(left)),
            right: Some(Box.new(right)),
        }
    }

    fn sum(&self) -> i32 {
        val left_sum = match &self.left {
            Some(node) => node.sum(),
            None => 0,
        };
        val right_sum = match &self.right {
            Some(node) => node.sum(),
            None => 0,
        };
        self.value + left_sum + right_sum
    }
}

// ── Collection examples ──────────────────────────────────────────────

pub fn word_count(text: &str) -> {String: usize} {
    var counts: {String: usize} = HashMap.new();
    for word in text.split_whitespace() {
        val counter = counts.entry(word.to_string()).or_insert(0);
        *counter = *counter + 1;
    }
    counts
}

pub fn unique_words(text: &str) -> {String} {
    var set: {String} = HashSet.new();
    for word in text.split_whitespace() {
        set.insert(word.to_string());
    }
    set
}

// ── Error handling with error unions ─────────────────────────────────

data AppError = NotFound(String) | InvalidInput(String) | IoError(String)

// T or E replaces Result<T, E>.
pub fn parse_age(input: &str) -> u32 or AppError {
    match input.parse() {
        Ok(age) => {
            guard age <= 150 else {
                return Err(AppError.InvalidInput(format!("age {age} is unrealistic")));
            }
            Ok(age)
        },
        Err(_) => Err(AppError.InvalidInput(format!("'{input}' is not a number"))),
    }
}

// ── Main ─────────────────────────────────────────────────────────────

pub fn main() {
    // Data construction.
    var p = Point { x: 3.0, y: 4.0 };
    val origin = Point[f64].origin();
    println!("Distance from origin: {p.distance(&origin)}");

    // Mutation via mutable reference.
    p.translate(1.0, -1.0);
    println!("After translate: ({p.x}, {p.y})");

    // Enum + pattern matching.
    val shapes: [Shape]~ = [
        Shape::Circle { radius: 5.0 },
        Shape::Rectangle { width: 10.0, height: 3.0 },
        Shape::Triangle { base: 6.0, height: 4.0 },
    ];

    for shape in &shapes {
        println!("{shape.describe()}: area = {shape.area()}");
    }

    // Generic stack.
    var stack = Stack[i32].new();
    stack.push(1);
    stack.push(2);
    stack.push(3);
    println!("Stack top: {stack.peek():?}");
    println!("Popped: {stack.pop():?}");

    // Binary tree.
    val tree = TreeNode.branch(
        1,
        TreeNode.branch(2, TreeNode.leaf(4), TreeNode.leaf(5)),
        TreeNode.leaf(3),
    );
    println!("Tree sum: {tree.sum()}");

    // HashMap using map type syntax.
    val text = "the quick brown fox jumps over the lazy fox";
    val counts = word_count(text);
    println!("Word counts: {counts:?}");

    // Error handling with pattern test (is).
    val result = parse_age("25");
    if result is Ok(_) {
        println!("Valid age");
    }

    match parse_age("xyz") {
        Ok(age) => println!("Parsed age: {age}"),
        Err(e) => println!("Error: {e:?}"),
    }
}
