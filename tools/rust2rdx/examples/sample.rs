use std::collections::HashMap;
use std::io::Read;

#[derive(Debug, Clone)]
pub struct Config {
    name: String,
    values: Vec<i32>,
    cache: HashMap<String, i32>,
    data: Option<Box<Node>>,
}

pub enum Shape {
    Circle(f64),
    Rect { w: f64, h: f64 },
}

pub trait Drawable {
    fn draw(&self);
    fn area(&self) -> f64;
}

impl Drawable for Shape {
    fn draw(&self) {
        match self {
            Shape::Circle(r) => println!("Circle r={}", r),
            Shape::Rect { w, h } => println!("Rect {}x{}", w, h),
        }
    }

    fn area(&self) -> f64 {
        match self {
            Shape::Circle(r) => 3.14159 * r * r,
            Shape::Rect { w, h } => w * h,
        }
    }
}

pub fn process(items: Vec<i32>) -> Result<i32, String> {
    let mut sum = 0;
    for item in &items {
        sum += item;
    }
    if sum > 100 {
        return Ok(sum);
    } else {
        return Err(format!("sum too small: {}", sum));
    }
}

fn helper<'a>(input: &'a str) -> &'a str {
    let trimmed = input.trim();
    trimmed
}

async fn fetch_data(url: &str) -> Result<String, Box<dyn std::error::Error>> {
    let response = reqwest::get(url).await?;
    let body = response.text().await?;
    Ok(body)
}

#[inline]
fn fast_add(a: i32, b: i32) -> i32 {
    a + b
}

fn main() {
    let config = Config {
        name: String::from("test"),
        values: vec![1, 2, 3],
        cache: HashMap::new(),
        data: None,
    };
    println!("Config: {:?}", config);
    let mut counter = 0;
    counter += 1;
}
