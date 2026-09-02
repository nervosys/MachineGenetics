# Few-Shot Prompt — Code Generation

Use the following examples to guide MAGE code generation.

---

## Example 1: Simple function

**Task:** Write a function that sums all even numbers in a slice.

**MAGE:**
```MAGE
+f sum_evens(nums: &[i32]) -> i32 {
    nums.iter().filter(|n| *n % 2 == 0).sum()
}
```

---

## Example 2: Struct with methods

**Task:** Write a Counter struct with increment, decrement, and value methods.

**MAGE:**
```MAGE
+S Counter { count: i32 }

I Counter {
    +f new() -> Counter { @Counter { count: 0 } }

    +f increment(self) -> Counter { @Counter { count: self.count + 1 } }

    +f decrement(self) -> Counter { @Counter { count: self.count - 1 } }

    +f value(self) -> i32 { self.count }
}
```

---

## Example 3: Enum with pattern matching

**Task:** Write a Shape enum with Circle and Rectangle variants and an area method.

**MAGE:**
```MAGE
+E Shape { Circle(f64), Rectangle(f64, f64) }

I Shape {
    +f area(self) -> f64 {
        ?= self {
            Shape.Circle(r) => 3.14159 * r * r,
            Shape.Rectangle(w, h) => w * h,
        }
    }
}
```

---

## Example 4: Generic function with trait bound

**Task:** Write a function that finds the maximum element in a non-empty slice.

**MAGE:**
```MAGE
+f find_max(items: [i32]~) -> i32 {
    m best = items[0]
    @ item in items {
        ? item > best { best = item } : { }
    }
    best
}
```

---

## Example 5: A declared effect, performed and guarded

**Task:** Declare an `http` effect, and write a function that performs it after
checking its argument.

**MAGE:**
```MAGE
effect Http {
    f get(url: str) -> str;
}

+f fetch_body(url: str) -> R[str, str] / http {
    guard len(url) > 0 else { ret Err("empty url") }
    Ok(Http.get(url))
}
```

---

## Example 6: An error enum, matched exhaustively

**Task:** Write an `AppError` enum and a function that turns each variant into
a message.

**MAGE:**
```MAGE
+E AppError { NotFound, Invalid(str) }

+f describe(e: AppError) -> str {
    ?= e {
        AppError.NotFound => "not found",
        AppError.Invalid(why) => why,
    }
}
```

---

Now generate MAGE code for the following task:

**Task:** {{task}}

**MAGE:**
```MAGE

```