# Networking & HTTP

Redox includes HTTP in the standard library — no external packages needed.

## HTTP Client

```rdx
use std::net::{Request, Response, Method};

pub fn main() / io, net {
    // Simple GET
    let resp = Request::get("https://api.example.com/data").send()?;
    println!("Status: {}", resp.status());
    println!("Body: {}", resp.text()?);
}
```

### POST with JSON body

```rdx
use std::net::Request;
use std::json::stringify;

#[derive(Serialize)]
struct CreateUser { name: String, email: String }

pub fn create_user() -> Result<Response, NetError> / net {
    let user = CreateUser {
        name: "Alice".into(),
        email: "alice@example.com".into(),
    };

    Request::post("https://api.example.com/users")
        .header("Content-Type", "application/json")
        .json(&user)
        .send()
}
```

### Request builder

```rdx
let resp = Request::new(Method::Put, "https://api.example.com/data")
    .header("Authorization", &format!("Bearer {token}"))
    .header("Accept", "application/json")
    .body(payload)
    .send()?;
```

## TCP

```rdx
use std::net::{TcpStream, TcpListener};

// TCP server
pub fn serve() / io, net {
    let listener = TcpListener::bind("127.0.0.1:8080")?;
    println!("Listening on :8080");

    for stream in listener.incoming() {
        let stream = stream?;
        handle_connection(stream)?;
    }
}

fn handle_connection(mut stream: TcpStream) / io, net {
    let mut buf = [0u8; 1024];
    let n = stream.read(&mut buf)?;
    let request = String::from_utf8(&buf[..n])?;
    stream.write(b"HTTP/1.1 200 OK\r\n\r\nHello!")?;
}
```

## UDP

```rdx
use std::net::UdpSocket;

pub fn main() / io, net {
    let socket = UdpSocket::bind("0.0.0.0:9000")?;
    let mut buf = [0u8; 1024];
    let (n, addr) = socket.recv_from(&mut buf)?;
    println!("Received {n} bytes from {addr}");
    socket.send_to(b"ACK", addr)?;
}
```

## DNS

```rdx
use std::net::dns;

pub fn main() / net {
    let addrs = dns::resolve("example.com")?;
    for addr in addrs {
        println!("IP: {addr}");
    }
}
```
