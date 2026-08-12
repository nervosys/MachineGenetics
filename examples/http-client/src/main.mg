// http-client — fetching over an effectful boundary.
//
// `forge run` evaluates `main` and prints its result. Demonstrates:
//   - `/ net` effect annotations and how they propagate: every caller of an
//     effectful function must declare the effect too, all the way up to `main`
//     (delete it and `mage-parse --check` says so)
//   - `pub async` for the request itself
//   - `Result<T, E>` with an error enum, and `match` that handles every case
//   - a struct decoded out of a response body
//
// The evaluator has no sockets, so `transport` answers from a fixed routing
// table instead of a real one. That is the only stub: the effect annotations,
// the error type, and the response handling are what they would be over a real
// socket, and the type checker enforces them either way.
//
// Run:  forge run        (or:  mage-parse --eval src/main.mg main)

// ── Wire types ───────────────────────────────────────────────────────

// A response is a status, a body, and the one header this client acts on.
// That is what the effectful boundary returns; decoding happens above it, in
// pure code.
struct Response {
    status: i32,
    body: String,
    retry_after: i32,
}

// `id` is a String because the standard vocabulary has no string-to-integer
// parse — see `resolve::VOCABULARY`. Storing the raw field is honest; making
// up a number from its length would not be.
struct User {
    id: String,
    name: String,
    active: bool,
}

// Every way a request can fail, as data. `RateLimited` carries the retry delay
// the server asked for, so the caller can act on it rather than just report it.
enum ApiError {
    NotFound,
    RateLimited(i32),
    Transport(String),
    Decode(String),
}

fn describe(error: ApiError) -> String {
    ?= error {
        ApiError.NotFound => "not found",
        ApiError.RateLimited(secs) => f"rate limited, retry in {secs}s",
        ApiError.Transport(why) => f"transport: {why}",
        ApiError.Decode(why) => f"decode: {why}",
    }
}

// ── The effectful boundary ───────────────────────────────────────────

// The one function that would touch a socket. Everything below is pure and
// testable without a network; everything above inherits `/ net`.
pub async transport(path: String) -> Response / net {
    ?= path {
        "/users/1" => @Response { status: 200, body: "1,Ada,yes", retry_after: 0 },
        "/users/2" => @Response { status: 429, body: "", retry_after: 30 },
        "/users/3" => @Response { status: 404, body: "", retry_after: 0 },
        "/users/4" => @Response { status: 200, body: "not-a-user", retry_after: 0 },
        _ => @Response { status: 500, body: "unrouted", retry_after: 0 },
    }
}

// ── Decoding ─────────────────────────────────────────────────────────

// A body is `id,name,active`. Anything else is a decode error rather than a
// partially-filled struct, so a malformed response cannot masquerade as a user.
fn decode_user(body: String) -> Result<User, ApiError> {
    val parts = split(body, ",")
    ? len(parts) == 3 {
        Ok(@User { id: parts[0], name: parts[1], active: parts[2] == "yes" })
    } : {
        Err(ApiError.Decode(f"expected 3 fields, got {len(parts)}"))
    }
}

// Status handling is the whole point of the type: each code becomes a distinct
// error value, not a string the caller has to re-parse.
fn interpret(response: Response) -> Result<User, ApiError> {
    ? response.status == 200 {
        decode_user(response.body)
    } : {
        ? response.status == 404 {
            Err(ApiError.NotFound)
        } : {
            ? response.status == 429 {
                Err(ApiError.RateLimited(response.retry_after))
            } : {
                Err(ApiError.Transport(f"HTTP {response.status}"))
            }
        }
    }
}

// ── Client ───────────────────────────────────────────────────────────

// `/ net` is inherited from `transport`, not declared for decoration.
pub async fetch_user(id: i32) -> Result<User, ApiError> / net {
    interpret(transport(f"/users/{id}"))
}

// A rate-limited request is the one failure worth retrying, and only once —
// the retry is in the client so callers cannot forget it.
pub async fetch_user_retrying(id: i32) -> Result<User, ApiError> / net {
    ?= fetch_user(id) {
        Err(ApiError.RateLimited(secs)) => fetch_user(id),
        other => other,
    }
}

fn render(id: i32, result: Result<User, ApiError>) -> String {
    ?= result {
        Ok(user) => f"{id}: user {user.id} {user.name} (active={user.active})",
        Err(error) => f"{id}: {describe(error)}",
    }
}

// ── Entry point ──────────────────────────────────────────────────────

// `/ net` is what everything below performs. Removing it is an error; adding
// `/ io` on top is not — the checker rejects *under*-declaration, so an
// annotation is an upper bound on effects, not an exact description of them.
pub fn main() -> String / net {
    val ids = [1, 2, 3, 4]
    val report = map(ids, fn(id) => render(id, fetch_user_retrying(id)))
    join(report, "; ")
}
