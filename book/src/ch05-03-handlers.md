# Writing Handlers

Effect handlers let you **intercept** an effect and provide your own
implementation. This is the key to testability, mocking, and effect composition.

## The `handle` function

```rdx
u std.effect.{handle, perform}

+f main() / io {
    // Run code with a custom IO handler
    v result = handle[IoEffect, s](
        || {
            // This code "performs" the io effect
            v data = File.read("config.json")?
            parse(&data)
        },
        |input| {
            // This handler intercepts the effect
            // Instead of real I/O, return mock data
            Ok(r#"{"key": "value"}"#.to_string())
        },
    )
}
```

## Why handlers matter

### Testing

Replace real I/O with deterministic mocks:

```rdx
@test
f test_config_loading() {
    v result = handle[IoEffect, Config](
        || load_config("test.json"),
        |_path| Ok(r#"{"debug": true}"#.to_string()),
    )
    assert_eq(result.debug, 1b)
}
```

### Logging

Wrap an effect to add logging:

```rdx
f with_logging[T](f: f() -> T / io) -> T / io {
    handle[IoEffect, T](
        f,
        |input| {
            p"IO operation: {input}"
            perform[IoEffect](input)    // delegate to real handler
        },
    )
}
```

### Dependency injection

Effects replace constructor-injected dependencies:

```rdx
// Instead of:
//   S Service { db: ^Database, http: ^HttpClient }

// Use effects:
+f process_order(order: &Order) -> R[Receipt, Error] / db, net {
    v inventory = db_check(order.item_id)?
    v payment = charge_card(order.payment)?
    Ok(Receipt @{ order_id: order.id, amount: payment.amount })
}

// In tests, handle db and net effects with mocks
// In production, handle with real implementations
```

## Multi-effect handlers

Handle multiple effects at once:

```rdx
u std.effect.handle2

v result = handle2[IoEffect, NetEffect, Response](
    || fetch_and_save("https://api.example.com", "data.json"),
    |io_input| mock_file_system(io_input),
    |net_input| mock_http_client(net_input),
)
```

## Effect discharge

Check whether a type has a particular effect and conditionally handle it:

```rdx
u std.effect.{has_effect, discharge}

// Remove the io effect by handling it
f make_pure[T](f: f() -> T / io) -> T {
    handle[IoEffect, T](f, |_| default_value())
}
```

The `discharge` operation removes an effect from a function's signature by
providing a handler — transforming an effectful computation into a pure one.
