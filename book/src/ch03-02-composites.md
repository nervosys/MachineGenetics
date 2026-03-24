# Composite Types

## Structs

```mg
struct User {
    name: String,
    age: u32,
    email: String,
}

// Creating an instance
let user = User {
    name: "Alice".into(),
    age: 30,
    email: "alice@example.com".into(),
};
```

### Tuple structs

```mg
struct Point(f64, f64);
struct Color(u8, u8, u8);

let origin = Point(0.0, 0.0);
let red = Color(255, 0, 0);
```

### Unit structs

```mg
struct Marker;
```

## Enums

```mg
enum Direction {
    North,
    South,
    East,
    West,
}

// Enum with data
enum Shape {
    Circle(f64),
    Rectangle(f64, f64),
    Triangle { a: f64, b: f64, c: f64 },
}

// Using enums
let d = Direction::North;
let s = Shape::Circle(5.0);
```

## Tuples

```mg
let pair: (i32, String) = (42, "hello".into());
let (x, y) = pair;    // destructure
let first = pair.0;    // field access
```

## Arrays

Fixed-size arrays:

```mg
let data: [i32; 4] = [1, 2, 3, 4];
let zeros = [0; 100];    // 100 zeros
```

## Slices

```mg
let slice: &[i32] = &data[1..3];
```

## Type aliases

```mg
type Id = u64
type Result[T] = R[T, AppError]
type Grid = [[f64]~]~
```
