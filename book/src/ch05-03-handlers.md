# Writing Handlers

Effect handlers let you **intercept** an effect and provide your own
implementation. This is the key to testability, mocking, and effect composition.

## The `handle` function

```mg
use std::effect::{handle, perform};

pub fn main() / io {
    // Run code with a custom IO handler
    let result = handle::<IoEffect, String>(
        || {
            // This code "performs" the io effect
            let data = File::read("config.json")?;
            parse(&data)
        },
        |input| {
            // This handler intercepts the effect
            // Instead of real I/O, return mock data
            Ok(r#"{"key": "value"}"#.to_string())
        },
    );
}
```

## Why handlers matter

### Testing

Replace real I/O with deterministic mocks:

```mg
#[test]
fn test_config_loading() {
    let result = handle::<IoEffect, Config>(
        || load_config("test.json"),
        |_path| Ok(r#"{"debug": true}"#.to_string()),
    );
    assert_eq!(result.debug, true);
}
```

### Logging

Wrap an effect to add logging:

```mg
fn with_logging<T>(f: fn() -> T / io) -> T / io {
    handle::<IoEffect, T>(
        f,
        |input| {
            println!("IO operation: {input}");
            perform::<IoEffect>(input)    // delegate to real handler
        },
    )
}
```

### Dependency injection

Effects replace constructor-injected dependencies:

```mg
// Instead of:
//   struct Service { db: Box<Database>, http: Box<HttpClient> }

// Use effects:
pub fn process_order(order: &Order) -> Result<Receipt, Error> / db, net {
    let inventory = db_check(order.item_id)?;
    let payment = charge_card(order.payment)?;
    Ok(Receipt { order_id: order.id, amount: payment.amount })
}

// In tests, handle db and net effects with mocks
// In production, handle with real implementations
```

## Multi-effect handlers

Handle multiple effects at once:

```mg
use std::effect::handle2;

let result = handle2::<IoEffect, NetEffect, Response>(
    || fetch_and_save("https://api.example.com", "data.json"),
    |io_input| mock_file_system(io_input),
    |net_input| mock_http_client(net_input),
);
```

## Effect discharge

Check whether a type has a particular effect and conditionally handle it:

```mg
use std::effect::{has_effect, discharge};

// Remove the io effect by handling it
fn make_pure<T>(f: fn() -> T / io) -> T {
    handle::<IoEffect, T>(f, |_| default_value())
}
```

The `discharge` operation removes an effect from a function's signature by
providing a handler — transforming an effectful computation into a pure one.
