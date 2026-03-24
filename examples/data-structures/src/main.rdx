// data-structures — Structs, enums, generics, traits, pattern matching.
//
// Demonstrates:
//   - Struct definitions (pub struct, struct)
//   - Enum definitions (pub enum, enum)
//   - Generic types <T>
//   - Trait definitions (pub trait)
//   - Impl blocks (impl Type, impl Trait for Type)
//   - Derive attributes (#[derive])
//   - Pattern matching (match value { ... })
//   - Vec<T>, Option<T>, Result<T, E>
//   - Box<T>, Rc<T>, Arc<T>
//   - HashMap<K, V>
//   - Mutable references (&mut T)

// ── Structs ──────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Point<T> {
    pub x: T,
    pub y: T,
}

#[derive(Debug)]
pub struct Person {
    name: String,
    age: u32,
    email: Option<String>,
}

// ── Enums ────────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum Color {
    Red,
    Green,
    Blue,
    Custom(u8, u8, u8),
}

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

// ── Impl blocks ──────────────────────────────────────────────────────

impl Point<f64> {
    pub fn origin() -> Point<f64> {
        Point { x: 0.0, y: 0.0 }
    }

    pub fn distance(&self, other: &Point<f64>) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy).sqrt()
    }

    pub fn translate(&mut self, dx: f64, dy: f64) {
        self.x = self.x + dx;
        self.y = self.y + dy;
    }
}

impl Area for Shape {
    fn area(&self) -> f64 {
        match self {
            Shape::Circle { radius } => std::f64::consts::PI * radius * radius,
            Shape::Rectangle { width, height } => width * height,
            Shape::Triangle { base, height } => 0.5 * base * height,
        }
    }
}

impl Describe for Shape {
    fn describe(&self) -> String {
        match self {
            Shape::Circle { radius } => format!("Circle with radius {radius}"),
            Shape::Rectangle { width, height } => format!("Rectangle {width}x{height}"),
            Shape::Triangle { base, height } => format!("Triangle base={base} h={height}"),
        }
    }
}

impl Describe for Color {
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
pub struct Stack<T> {
    items: Vec<T>,
}

impl<T> Stack<T> {
    pub fn new() -> Stack<T> {
        Stack { items: Vec::new() }
    }

    pub fn push(&mut self, item: T) {
        self.items.push(item);
    }

    pub fn pop(&mut self) -> Option<T> {
        self.items.pop()
    }

    pub fn peek(&self) -> Option<&T> {
        self.items.last()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

// ── Smart pointer examples ───────────────────────────────────────────

#[derive(Debug)]
struct TreeNode {
    value: i32,
    left: Option<Box<TreeNode>>,
    right: Option<Box<TreeNode>>,
}

impl TreeNode {
    fn leaf(value: i32) -> TreeNode {
        TreeNode { value: value, left: None, right: None }
    }

    fn branch(value: i32, left: TreeNode, right: TreeNode) -> TreeNode {
        TreeNode {
            value: value,
            left: Some(Box::new(left)),
            right: Some(Box::new(right)),
        }
    }

    fn sum(&self) -> i32 {
        let left_sum = match &self.left {
            Some(node) => node.sum(),
            None => 0,
        };
        let right_sum = match &self.right {
            Some(node) => node.sum(),
            None => 0,
        };
        self.value + left_sum + right_sum
    }
}

// ── Collection examples ──────────────────────────────────────────────

pub fn word_count(text: &str) -> HashMap<String, usize> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for word in text.split_whitespace() {
        let counter = counts.entry(word.to_string()).or_insert(0);
        *counter = *counter + 1;
    }
    counts
}

pub fn unique_words(text: &str) -> HashSet<String> {
    let mut set: HashSet<String> = HashSet::new();
    for word in text.split_whitespace() {
        set.insert(word.to_string());
    }
    set
}

// ── Error handling ───────────────────────────────────────────────────

#[derive(Debug)]
pub enum AppError {
    NotFound(String),
    InvalidInput(String),
    IoError(String),
}

pub fn parse_age(input: &str) -> Result<u32, AppError> {
    match input.parse::<u32>() {
        Ok(age) => {
            if age > 150 {
                Err(AppError::InvalidInput(format!("age {age} is unrealistic")))
            } else {
                Ok(age)
            }
        },
        Err(_) => Err(AppError::InvalidInput(format!("'{input}' is not a number"))),
    }
}

// ── Main ─────────────────────────────────────────────────────────────

pub fn main() {
    // Struct construction.
    let mut p = Point { x: 3.0, y: 4.0 };
    let origin = Point::<f64>::origin();
    println!("Distance from origin: {p.distance(&origin)}");

    // Mutation via mutable reference.
    p.translate(1.0, -1.0);
    println!("After translate: ({p.x}, {p.y})");

    // Enum + pattern matching.
    let shapes: Vec<Shape> = vec![
        Shape::Circle { radius: 5.0 },
        Shape::Rectangle { width: 10.0, height: 3.0 },
        Shape::Triangle { base: 6.0, height: 4.0 },
    ];

    for shape in &shapes {
        println!("{shape.describe()}: area = {shape.area()}");
    }

    // Generic stack.
    let mut stack = Stack::<i32>::new();
    stack.push(1);
    stack.push(2);
    stack.push(3);
    println!("Stack top: {stack.peek():?}");
    println!("Popped: {stack.pop():?}");

    // Binary tree.
    let tree = TreeNode::branch(
        1,
        TreeNode::branch(2, TreeNode::leaf(4), TreeNode::leaf(5)),
        TreeNode::leaf(3),
    );
    println!("Tree sum: {tree.sum()}");

    // HashMap.
    let text = "the quick brown fox jumps over the lazy fox";
    let counts = word_count(text);
    println!("Word counts: {counts:?}");

    // Error handling.
    match parse_age("25") {
        Ok(age) => println!("Parsed age: {age}"),
        Err(e) => println!("Error: {e:?}"),
    }
    match parse_age("xyz") {
        Ok(age) => println!("Parsed age: {age}"),
        Err(e) => println!("Error: {e:?}"),
    }
}
