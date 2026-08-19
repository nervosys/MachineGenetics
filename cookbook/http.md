# HTTP & Networking

> Recipes for the network. Agent-mode syntax; every block was verified with
> `mage-parse --check`.

The previous version of this page was written against an HTTP library that
does not exist — `u std.net.http`, `Request.get(url).send().await?`,
`resp.json[T]()`, `TcpListener.bind(…)?.incoming()`. What exists is the `net`
capability namespace, in scope everywhere, with the operations §11.2 lists:
`connect`, `listen`, `bind`, `send`, `recv`.

The recipes are short because the interesting part is not the API — it is the
**annotation**. A function that fetches and writes declares `/ net, fs`, and
that pair is the audit trail: nothing that touches the network and your disk
can be mistaken for a pure helper.

---

### Simple GET request

**Problem**: Fetch a URL and print the body.

**Solution**:

```MAGE
// `net` is the capability namespace; `net.connect` is the documented
// operation. There is no `Request` type, no `.send()`, and no `.await`.
+f get(url: s) -> s / net {
    net.connect(url)
}

+f main() -> i32 / net, io {
    p"{get("https://api.example.com/status")}"
    0
}
```

**Discussion**: `net` is the capability namespace and `connect` / `send` / `recv` / `listen` / `bind` are its documented operations. There is no `Request` type, no `.send()`, no `.await`, and nothing to import.

---

### POST JSON to an API

**Problem**: Send a JSON body to an endpoint.

**Solution**:

```MAGE
// A body is a string you build. `json` parses and renders but performs no
// effect; the *sending* is what performs `net`.
+f post_json(url: s, name: s, email: s) -> s / net {
    v body = f"{{\"name\": \"{name}\", \"email\": \"{email}\"}}"
    net.send(url, body)
}
```

**Discussion**: A body is a string you build — `{{` escapes a brace inside an f-string. `json` performs no effect; the *sending* is what performs `net`.

---

### Download a file

**Problem**: Fetch a URL and save it to disk.

**Solution**:

```MAGE
// Two capabilities: `net` to fetch, `fs` to write. Neither implies the other,
// so both are declared — that pair *is* the audit trail for a download.
+f download(url: s, path: s) -> i32 / net, fs {
    v body = net.connect(url)
    fs.write(path, body)
    len(body) as i32
}
```

**Discussion**: Two capabilities, two declarations: `net` to fetch and `fs` to write. **That pair is the audit trail** — a function that can read the network and write your disk cannot hide it in its signature.

---

### Fetch and parse JSON

**Problem**: Fetch a URL and turn the response into a record.

**Solution**:

```MAGE
+S Repo { name: s, stars: i32 }

// Fetch, then parse. `json.parse` attributes no effect — see the note in
// `data.md` — so the annotation names `net` alone.
+f fetch_repo(url: s) -> Repo / net {
    v body = net.connect(url)
    v fields: {s: s} = json.parse(body)
    @Repo { name: fields["name"], stars: 0 }
}
```

**Discussion**: A capability call returns a type the checker does not know, so annotate the binding: `v fields: {s: s} = json.parse(body)`.

---

### Parallel requests

**Problem**: Fetch several URLs and combine the results.

**Solution**:

```MAGE
// Parallel requests, without a join handle: `map` over the URLs. The effect
// annotation is what makes the fan-out visible — every caller inherits `net`.
+f fetch_all(urls: [s]~) -> [s]~ / net {
    map(urls, |url| net.connect(url))
}

+f total_bytes(urls: [s]~) -> i32 / net {
    fold(fetch_all(urls), 0, |acc, body| acc + (len(body) as i32))
}
```

**Discussion**: `map` is the fan-out and `fold` the fan-in. There is no join handle and no `Vec<Future>`; the effect annotation is what makes the concurrency visible in the signature.

---

### Simple TCP echo server

**Problem**: Accept connections and echo what arrives.

**Solution**:

```MAGE
// `net.listen` and `net.recv` are the documented operations. There is no
// socket type and no accept loop object — the capability namespace is the
// whole surface.
+f echo_server(address: s, rounds: i32) -> i32 / net {
    net.listen(address)
    m served = 0
    @ _n in range(rounds as usize) {
        v line: s = net.recv(address)
        net.send(address, line)
        served += 1
    }
    served
}
```

**Discussion**: `listen` / `recv` / `send` are all there is. No socket type, no accept loop object — the capability namespace is the whole surface.

---

### Request with a deadline

**Problem**: Give a request a time budget.

**Solution**:

```MAGE
// A timeout is two capabilities: the request, and the clock. `time` is an
// effect kind of its own, so a function that waits says so.
+f get_with_deadline(url: s, budget_ms: i32) -> s / net, time {
    v started = time.now()
    v body = net.connect(url)
    ? time.now() - started > budget_ms { "" } : { body }
}
```

**Discussion**: `time` is a capability of its own, so **a function that waits declares it**. A deadline is not free of consequence, and the signature says so.

---

### Check if a host is reachable

**Problem**: Probe a list of hosts and take the first that answers.

**Solution**:

```MAGE
+f reachable(host: s) -> bool / net {
    len(net.connect(host)) > 0
}

+f first_reachable(hosts: [s]~) -> ?s / net {
    find(hosts, |host| reachable(host))
}
```

**Discussion**: `find` returns `?s` — the optional is the answer to "maybe none of them", and there is no `unwrap` to skip past it.

---

### Build a REST handler

**Problem**: Dispatch a request path to a handler.

**Solution**:

```MAGE
+E Route {
    GetUser(i32),
    ListUsers,
    NotFound(s),
}

f route_of(path: s) -> Route {
    v parts = filter(split(path, "/"), |part| len(part) > 0)
    guard len(parts) > 0 else { ret NotFound(path) }
    ?= parts[0] {
        "users" => ? len(parts) > 1 { GetUser(1) } : { ListUsers },
        _ => NotFound(path),
    }
}

// The dispatch is a `?=` over the route sum; the handler bodies declare what
// they reach. A route that only formats a response stays pure.
+f handle(path: s) -> s / fs {
    ?= route_of(path) {
        GetUser(id) => fs.read_to_string(f"users/{id}.json"),
        ListUsers => fs.read_to_string("users/index.json"),
        NotFound(p) => f"404 {p}",
    }
}
```

**Discussion**: The route is a sum, the dispatch a `?=`. Handlers declare what they reach: a route that only formats a response stays pure, and the checker keeps it that way.

---
