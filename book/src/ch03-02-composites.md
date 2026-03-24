# Composite Types

## Structs

```rdx
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

```rdx
struct Point(f64, f64);
struct Color(u8, u8, u8);

let origin = Point(0.0, 0.0);
let red = Color(255, 0, 0);
```

### Unit structs

```rdx
struct Marker;
```

## Enums

```rdx
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

```rdx
let pair: (i32, String) = (42, "hello".into());
let (x, y) = pair;    // destructure
let first = pair.0;    // field access
```

## Arrays

Fixed-size arrays:

```rdx
let data: [i32; 4] = [1, 2, 3, 4];
let zeros = [0; 100];    // 100 zeros
```

## Slices

```rdx
let slice: &[i32] = &data[1..3];
```

## Type aliases

```rdx
type Id = u64
type Result[T] = R[T, AppError]
type Grid = [[f64]~]~
```
