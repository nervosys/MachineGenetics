# Networking & HTTP

Redox includes HTTP in the standard library — no external packages needed.

## HTTP Client

```rdx
u std.net.{Request, Response, Method}

+f main() / io, net {
    // Simple GET
    v resp = Request.get("https://api.example.com/data").send()?
    p"Status: {resp.status()}"
    p"Body: {resp.text()?}"
}
```

### POST with JSON body

```rdx
u std.net.Request
u std.json.stringify

@d(Serialize)
S CreateUser { name: s, email: s }

+f create_user() -> R[Response, NetError] / net {
    v user = CreateUser @{
        name: "Alice".into(),
        email: "alice@example.com".into(),
    }

    Request.post("https://api.example.com/users")
        .header("Content-Type", "application/json")
        .json(&user)
        .send()
}
```

### Request builder

```rdx
v resp = Request.new(Method.Put, "https://api.example.com/data")
    .header("Authorization", f"Bearer {token}")
    .header("Accept", "application/json")
    .body(payload)
    .send()?
```

## TCP

```rdx
u std.net.{TcpStream, TcpListener}

// TCP server
+f serve() / io, net {
    v listener = TcpListener.bind("127.0.0.1:8080")?
    p"Listening on :8080"

    @ stream : listener.incoming() {
        v stream = stream?
        handle_connection(stream)?
    }
}

f handle_connection(m stream: TcpStream) / io, net {
    m buf = [0u8; 1024]
    v n = stream.read(&!buf)?
    v request = s.from_utf8(&buf[..n])?
    stream.write(b"HTTP/1.1 200 OK\r\n\r\nHello!")?
}
```

## UDP

```rdx
u std.net.UdpSocket

+f main() / io, net {
    v socket = UdpSocket.bind("0.0.0.0:9000")?
    m buf = [0u8; 1024]
    v (n, addr) = socket.recv_from(&!buf)?
    p"Received {n} bytes from {addr}"
    socket.send_to(b"ACK", addr)?
}
```

## DNS

```rdx
u std.net.dns

+f main() / net {
    v addrs = dns.resolve("example.com")?
    @ addr : addrs {
        p"IP: {addr}"
    }
}
```
