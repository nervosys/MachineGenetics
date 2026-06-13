# HTTP & Networking

---

### Simple GET request

**Problem**: Fetch data from a URL.

**Solution**:

```mg
u std.net.Request

+f main() / io, net {
    v resp = Request.get("https://httpbin.org/get").send()?
    p"Status: {resp.status()}"
    p"Body: {resp.text()?}"
}
```

---

### POST JSON to an API

**Problem**: Send a JSON payload to a REST endpoint.

**Solution**:

```mg
u std.net.Request
u std.json.to_string

@d(Serialize)
S CreateItem { name: s, quantity: u32 }

+f main() / io, net {
    v item = CreateItem @{ name: "Widget".into(), quantity: 10 }

    v resp = Request.post("https://api.example.com/items")
        .header("Content-Type", "application/json")
        .json(&item)
        .send()?

    p"Created: {resp.status()}"
}
```

---

### Download a file

**Problem**: Download a large file and save it to disk.

**Solution**:

```mg
u std.net.Request
u std.fs

+f download(url: &s, dest: &s) / io, net {
    v resp = Request.get(url).send()?
    v bytes = resp.bytes()?
    fs.write_bytes(dest, &bytes)?
    p"Downloaded {bytes.len()} bytes to {dest}"
}

+f main() / io, net {
    download(
        "https://example.com/data.zip",
        "data.zip",
    )?
}
```

---

### Fetch and parse JSON

**Problem**: Call a JSON API and deserialize the response.

**Solution**:

```mg
u std.net.Request
u std.json.from_str

@d(Deserialize, Debug)
S User { id: u64, login: s, name: ?s }

+af fetch_user(username: &s) -> R[User, Error] / net {
    v url = f"https://api.github.com/users/{username}"
    v resp = Request.get(&url)
        .header("User-Agent", "MAGE-app")
        .send().await?
    v body = resp.text().await?
    v user: User = from_str(&body)?
    Ok(user)
}
```

---

### Parallel HTTP requests

**Problem**: Fetch multiple URLs concurrently.

**Solution**:

```mg
u std.net.Request
u std.async.{spawn, join_all}

+af fetch_all(urls: &[s]~) -> R[[s]~, Error] / net, async {
    m handles = [_]~.new()

    @ url : urls {
        v url = url.clone()
        handles.push(spawn(|| async {
            v resp = Request.get(&url).send().await?
            resp.text().await
        }))
    }

    v results = join_all(handles).await
    m bodies = [s]~.new()
    @ r : results {
        bodies.push(r??)
    }
    Ok(bodies)
}
```

**Discussion**: `join_all` waits for every handle. Use `select` or `race` if
you only need the first result.

---

### Simple TCP echo server

**Problem**: Build a TCP server that echoes back whatever clients send.

**Solution**:

```mg
u std.net.{TcpListener, TcpStream}
u std.io.{Read, Write}

+f main() / io, net {
    v listener = TcpListener.bind("127.0.0.1:7878")?
    p"Echo server on :7878"

    @ stream : listener.incoming() {
        v stream = stream?
        handle(stream)?
    }
}

f handle(m stream: TcpStream) / io, net {
    m buf = [0u8; 1024]
    loop {
        v n = stream.read(&!buf)?
        ? n == 0 { break }
        stream.write(&buf[..n])?
    }
}
```

---

### HTTP request with timeout

**Problem**: Abort a request if it takes too long.

**Solution**:

```mg
u std.net.Request
u std.time.Duration

+af main() / io, net, async {
    v resp = Request.get("https://slow-api.example.com/data")
        .timeout(Duration.from_secs(5))
        .send().await

    ? resp {
        Ok(r) => p"Got: {r.text().await?}",
        Err(e) => p"Request failed: {e}",
    }
}
```

---

### Check if a host is reachable

**Problem**: Ping a host to see if it's online.

**Solution**:

```mg
u std.net.{TcpStream, dns}
u std.time.Duration

+f is_reachable(host: &s, port: u16) -> bool / net {
    v addr = f"{host}:{port}"
    v result = TcpStream.connect_timeout(&addr, Duration.from_secs(3))
    result.is_ok()
}

+f main() / io, net {
    v hosts = ["google.com", "github.com", "localhost"]~
    @ host : &hosts {
        v up = is_reachable(host, 443)
        p"{host}: {? up { \"reachable\" } : { \"unreachable\" }}"
    }
}
```

---

### Build a REST API handler

**Problem**: Define a handler function for a REST endpoint.

**Solution**:

```mg
u std.net.{Request, Response, StatusCode}
u std.json.{from_str, to_string}

@d(Serialize, Deserialize)
S Item { id: u64, name: s, price: f64 }

+f handle_get_item(id: u64, db: &Database) -> Response / io {
    ? db.find_item(id) => Some(item) {
        Response.json(&item).status(StatusCode.Ok)
    } : {
        Response.text("Not found").status(StatusCode.NotFound)
    }
}

+f handle_create_item(body: &s, db: &!Database) -> Response / io {
    ? from_str[Item](body) => Ok(item) {
        db.insert(&item)?
        Response.json(&item).status(StatusCode.Created)
    } : Err(e) {
        Response.text(f"Bad request: {e}").status(StatusCode.BadRequest)
    }
}
```

**Discussion**: This shows handler functions that can be wired into any HTTP
framework. The effect annotation `/ io` marks database access.
