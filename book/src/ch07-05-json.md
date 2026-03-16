# JSON & Serialization

JSON is a **first-class** feature of Redox — no external crate needed.

## Parsing JSON

```rdx
u std.json.{parse, Value}

+f main() / io {
    v text = r#"{"name": "Alice", "age": 30}"#
    v val: Value = parse(text)?

    p"Name: {val.get("name")}"     // "Alice"
    p"Age: {val.get("age")}"       // 30
}
```

## Stringify

```rdx
u std.json.{stringify, stringify_pretty}

+f main() {
    v val = Value.Object({
        "name": Value.Str("Bob".into()),
        "scores": Value.Array([Value.Int(95), Value.Int(87)]~),
    })

    v compact = stringify(&val)
    v pretty = stringify_pretty(&val)
}
```

## Serialize / Deserialize

Use `@d(Serialize, Deserialize)` to derive JSON conversion:

```rdx
u std.json.{from_str, to_string}

@d(Serialize, Deserialize, Debug)
S User {
    name: s,
    age: u32,
    email: ?s,
}

+f main() / io {
    // Deserialize from JSON string
    v json = r#"{"name": "Alice", "age": 30, "email": null}"#
    v user: User = from_str(json)?
    p"User: {user.name}, age {user.age}"

    // Serialize to JSON string
    v output = to_string(&user)?
    p"JSON: {output}"
}
```

## Working with dynamic JSON

The `Value` enum:

```rdx
+E Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(s),
    Array([Value]~),
    Object({s: Value}),
}
```

Accessing nested values:

```rdx
v config = parse(text)?

// Dot-chained access with get/at
v host = config.get("database").get("host").as_str()?
v port = config.get("database").get("port").as_int()?
v first_item = config.get("items").at(0).as_str()?
```

## JSON in HTTP

HTTP and JSON work together naturally:

```rdx
u std.net.Request
u std.json.{from_str, to_string}

@d(Serialize, Deserialize)
S ApiResponse { status: s, data: [Item]~ }

+af fetch_items() -> R[[Item]~, Error] / net {
    v resp = Request.get("https://api.example.com/items").send().await?
    v body = resp.text().await?
    v parsed: ApiResponse = from_str(&body)?
    Ok(parsed.data)
}
```
