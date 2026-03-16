# Composite Types

## Structs

```rdx
S User {
    name: s,
    age: u32,
    email: s,
}

// Creating an instance (@ is struct-literal syntax)
v user = User @{
    name: "Alice".into(),
    age: 30,
    email: "alice@example.com".into(),
}
```

### Tuple structs

```rdx
S Point(f64, f64);
S Color(u8, u8, u8);

v origin = Point(0.0, 0.0)
v red = Color(255, 0, 0)
```

### Unit structs

```rdx
S Marker;
```

## Enums

```rdx
E Direction {
    North,
    South,
    East,
    West,
}

// Enum with data
E Shape {
    Circle(f64),
    Rectangle(f64, f64),
    Triangle { a: f64, b: f64, c: f64 },
}

// Using enums
v d = Direction.North
v s = Shape.Circle(5.0)
```

## Tuples

```rdx
v pair: (i32, s) = (42, "hello".into())
v (x, y) = pair    // destructure
v first = pair.0    // field access
```

## Arrays

Fixed-size arrays:

```rdx
v data: [i32; 4] = [1, 2, 3, 4]
v zeros = [0; 100]    // 100 zeros
```

## Slices

```rdx
v slice: &[i32] = &data[1..3]
```

## Type aliases

```rdx
type Id = u64
type Result[T] = R[T, AppError]
type Grid = [[f64]~]~
```
