# Few-Shot Prompt — Error Repair

Use the following examples to diagnose and fix Redox syntax errors.

---

## Example 1: Wrong function keyword

**Broken Redox:**
```redox
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}
```

**Error:** `unexpected token 'pub'`

**Fixed Redox:**
```redox
+f add(a: i32, b: i32) -> i32 {
    a + b
}
```

**Fix:** Replace `pub fn` with `+f`.

---

## Example 2: Wrong generic syntax

**Broken Redox:**
```redox
+f first<T>(items: &[T]) -> &T {
    &items[0]
}
```

**Error:** `unexpected '<' in generic parameter list`

**Fixed Redox:**
```redox
+f first[T](items: &[T]) -> &T {
    &items[0]
}
```

**Fix:** Replace `<T>` with `[T]` for generics.

---

## Example 3: Wrong path separator

**Broken Redox:**
```redox
u std::collections::HashMap

+f count_words(text: &s) -> {s: usize} {
    m map = HashMap::new();
    @ word ~ text.split_whitespace() {
        *map.entry(word.to_string()).or_insert(0) += 1;
    }
    map
}
```

**Error:** `unexpected '::' — use '.' for path access`

**Fixed Redox:**
```redox
u std.collections.HashMap

+f count_words(text: &s) -> {s: usize} {
    m map = HashMap.new();
    @ word ~ text.split_whitespace() {
        *map.entry(word.to_string()).or_insert(0) += 1;
    }
    map
}
```

**Fix:** Replace all `::` with `.` for path separators.

---

## Example 4: Wrong conditional and boolean syntax

**Broken Redox:**
```redox
+f is_valid(name: &s, age: u32) -> bool {
    if name.is_empty() {
        return false;
    }
    if age > 0 && age < 150 {
        true
    } else {
        false
    }
}
```

**Errors:** `unexpected 'if'`, `unexpected 'true'`, `unexpected 'else'`

**Fixed Redox:**
```redox
+f is_valid(name: &s, age: u32) -> bool {
    ? name.is_empty() {
        ret 0b;
    }
    ? age > 0 && age < 150 {
        1b
    } : {
        0b
    }
}
```

**Fix:** Replace `if`/`else` with `?`/`:`, `true`/`false` with `1b`/`0b`, `return` with `ret`.

---

## Example 5: Wrong binding and mut syntax

**Broken Redox:**
```redox
+f process(data: &mut Vec<i32>) {
    let total: i32 = data.iter().sum();
    let mut avg = total / data.len() as i32;
    avg += 1;
    data.push(avg);
}
```

**Errors:** `unexpected 'let'`, `unexpected '&mut'`, `unexpected 'Vec<'`

**Fixed Redox:**
```redox
+f process(data: &![i32]~) {
    v total: i32 = data.iter().sum();
    m avg = total / data.len() as i32;
    avg += 1;
    data.push(avg);
}
```

**Fix:** `let` → `v`, `let mut` → `m`, `&mut` → `&!`, `Vec<i32>` → `[i32]~`.

---

## Example 6: Missing effect annotation

**Broken Redox:**
```redox
+f save(path: &s, content: &s) -> R[(), std.io.Error] {
    std.fs.write(path, content)?;
    R.Ok(())
}
```

**Error:** `function performs I/O but missing effect annotation`

**Fixed Redox:**
```redox
+f save(path: &s, content: &s) -> R[(), std.io.Error] / io {
    std.fs.write(path, content)?;
    R.Ok(())
}
```

**Fix:** Add `/ io` to the function signature to declare the I/O effect.

---

Now fix the following Redox code:

**Broken Redox:**
```redox
{{broken_code}}
```

**Error:** {{error_message}}

**Fixed Redox:**
```redox
