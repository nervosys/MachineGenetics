# Few-Shot Prompt — Code Generation

Use the following examples to guide Redox code generation.

---

## Example 1: Simple function

**Task:** Write a function that sums all even numbers in a slice.

**Redox:**
```redox
+f sum_evens(nums: &[i32]) -> i32 {
    nums.iter().filter(|n| *n % 2 == 0).sum()
}
```

---

## Example 2: Struct with methods

**Task:** Write a Counter struct with increment, decrement, and value methods.

**Redox:**
```redox
+S Counter {
    count: i32,
}

I ~ Counter {
    +f new() -> Self {
        Counter @{ count: 0 }
    }

    +f increment(&!self) {
        self.count += 1;
    }

    +f decrement(&!self) {
        self.count -= 1;
    }

    +f value(&self) -> i32 {
        self.count
    }
}
```

---

## Example 3: Enum with pattern matching

**Task:** Write a Shape enum with Circle and Rectangle variants and an area method.

**Redox:**
```redox
+E Shape {
    Circle(f64),
    Rectangle(f64, f64),
}

I ~ Shape {
    +f area(&self) -> f64 {
        ? self {
            Shape.Circle(r) => std.f64.consts.PI * r * r,
            Shape.Rectangle(w, h) => w * h,
        }
    }
}
```

---

## Example 4: Generic function with trait bound

**Task:** Write a function that finds the maximum element in a non-empty slice.

**Redox:**
```redox
+f find_max[T: Ord](items: &[T]) -> &T {
    m max = &items[0];
    @ item ~ &items[1..] {
        ? item > max {
            max = item;
        }
    }
    max
}
```

---

## Example 5: Async with effects

**Task:** Write an async function that fetches JSON from a URL and parses it.

**Redox:**
```redox
u serde.de.DeserializeOwned

+af fetch_json[T: DeserializeOwned](url: &s) -> R[T, reqwest.Error] / io + net {
    v response = reqwest.get(url).await?;
    v data = response.json::[T]().await?;
    R.Ok(data)
}
```

---

## Example 6: Error type with From impls

**Task:** Write an AppError enum that wraps io::Error and serde_json::Error.

**Redox:**
```redox
u std.io

@d(Debug)
+E AppError {
    Io(io.Error),
    Json(serde_json.Error),
    Custom(s),
}

I std.fmt.Display ~ AppError {
    f fmt(&self, f: &!std.fmt.Formatter) -> std.fmt.Result {
        ? self {
            AppError.Io(e) => e.fmt(f),
            AppError.Json(e) => e.fmt(f),
            AppError.Custom(msg) => f.write_str(msg),
        }
    }
}

I std.error.Error ~ AppError {}

I std.convert.From[io.Error] ~ AppError {
    f from(e: io.Error) -> Self { AppError.Io(e) }
}

I std.convert.From[serde_json.Error] ~ AppError {
    f from(e: serde_json.Error) -> Self { AppError.Json(e) }
}
```

---

Now generate Redox code for the following task:

**Task:** {{task}}

**Redox:**
```redox
