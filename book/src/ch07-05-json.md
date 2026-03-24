# JSON & Serialization

JSON is a **first-class** feature of MechGen — no external crate needed.

## Parsing JSON

```mg
use std::json::{parse, Value};

pub fn main() / io {
    let text = r#"{"name": "Alice", "age": 30}"#;
    let val: Value = parse(text)?;

    println!("Name: {}", val.get("name"));     // "Alice"
    println!("Age: {}", val.get("age"));        // 30
}
```

## Stringify

```mg
use std::json::{stringify, stringify_pretty};

pub fn main() {
    let val = Value::Object({
        "name": Value::Str("Bob".into()),
        "scores": Value::Array(vec![Value::Int(95), Value::Int(87)]),
    });

    let compact = stringify(&val);
    let pretty = stringify_pretty(&val);
}
```

## Serialize / Deserialize

Use `#[derive(Serialize, Deserialize)]` to derive JSON conversion:

```mg
use std::json::{from_str, to_string};

#[derive(Serialize, Deserialize, Debug)]
struct User {
    name: String,
    age: u32,
    email: Option<String>,
}

pub fn main() / io {
    // Deserialize from JSON string
    let json = r#"{"name": "Alice", "age": 30, "email": null}"#;
    let user: User = from_str(json)?;
    println!("User: {}, age {}", user.name, user.age);

    // Serialize to JSON string
    let output = to_string(&user)?;
    println!("JSON: {output}");
}
```

## Working with dynamic JSON

The `Value` enum:

```mg
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    Array(Vec<Value>),
    Object(HashMap<String, Value>),
}
```

Accessing nested values:

```mg
let config = parse(text)?;

// Dot-chained access with get/at
let host = config.get("database").get("host").as_str()?;
let port = config.get("database").get("port").as_int()?;
let first_item = config.get("items").at(0).as_str()?;
```

## JSON in HTTP

HTTP and JSON work together naturally:

```mg
use std::net::Request;
use std::json::{from_str, to_string};

#[derive(Serialize, Deserialize)]
struct ApiResponse { status: String, data: Vec<Item> }

pub async fn fetch_items() -> Result<Vec<Item>, Error> / net {
    let resp = Request::get("https://api.example.com/items").send().await?;
    let body = resp.text().await?;
    let parsed: ApiResponse = from_str(&body)?;
    Ok(parsed.data)
}
```
