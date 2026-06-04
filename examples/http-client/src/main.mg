// http-client — Async HTTP client with JSON parsing.
//
// Demonstrates:
//   - Async functions (pub async fn, async fn)
//   - Effect annotations (/ io, / net)
//   - Error handling with T or E (error union)
//   - JSON deserialization
//   - data keyword + extend blocks
//   - val / var bindings
//   - guard for early exit
//   - defer for cleanup
//   - Pipeline operator |>

use std::io;
use std::net;
use std::json;
use std::fmt;

// ── Data types ───────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct User {
    id: u64,
    name: String,
    email: String,
    active: bool,
}

#[derive(Debug, Clone)]
pub struct Post {
    id: u64,
    user_id: u64,
    title: String,
    body: String,
}

#[derive(Debug)]
pub enum ApiError {
    NetworkError(String),
    ParseError(String),
    NotFound,
    RateLimited { retry_after: u64 },
}

extend ApiError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ApiError::NetworkError(msg) => write!(f, "network error: {msg}"),
            ApiError::ParseError(msg) => write!(f, "parse error: {msg}"),
            ApiError::NotFound => write!(f, "resource not found"),
            ApiError::RateLimited(retry_after) =>
                write!(f, "rate limited, retry after {retry_after}s"),
        }
    }
}

// ── HTTP client wrapper ──────────────────────────────────────────────

data ApiClient(base_url: String, timeout_ms: u64 = 5000)

extend ApiClient {
    pub fn new(base_url: String) -> ApiClient =
        ApiClient { base_url: base_url, timeout_ms: 5000 }

    pub fn with_timeout(base_url: String, timeout_ms: u64) -> ApiClient =
        ApiClient { base_url: base_url, timeout_ms: timeout_ms }

    /// Fetch a single resource by path.
    pub async fn get[T: json::Deserialize](&self, path: &str) -> T or ApiError / net, io {
        val url = format!("{self.base_url}{path}");
        val response = net::get(&url)
            .timeout(self.timeout_ms)
            .send()
            .await
            .map_err(|e| ApiError::NetworkError(format!("{e}")))?;

        guard response.status() == 200 else {
            if response.status() == 404 {
                return Err(ApiError::NotFound);
            }
            if response.status() == 429 {
                val retry = response.header("Retry-After")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(60);
                return Err(ApiError::RateLimited(retry));
            }
            return Err(ApiError::NetworkError(format!("HTTP {response.status()}")));
        }

        val body = response.text().await
            .map_err(|e| ApiError::NetworkError(format!("{e}")))?;

        json::from_str(&body)
            .map_err(|e| ApiError::ParseError(format!("{e}")))
    }

    /// Fetch a list of resources.
    pub async fn list[T: json::Deserialize](&self, path: &str) -> [T]~ or ApiError / net, io {
        self.get(path).await
    }
}

// ── Application logic ────────────────────────────────────────────────

async fn fetch_user_posts(client: &ApiClient, user_id: u64)
    -> [Post]~ or ApiError / net, io
{
    val path = format!("/users/{user_id}/posts");
    client.list[Post](&path).await
}

async fn display_user_summary(client: &ApiClient, user_id: u64)
    -> () or ApiError / net, io
{
    // Fetch user and posts concurrently.
    val user_future = client.get[User](&format!("/users/{user_id}"));
    val posts_future = fetch_user_posts(client, user_id);

    val user = user_future.await?;
    val posts = posts_future.await?;

    println!("");
    println!("=== User: {user.name} ===");
    println!("  Email:  {user.email}");
    println!("  Active: {user.active}");
    println!("  Posts:  {posts.len()}");
    println!("");

    for post in &posts {
        println!("  [{post.id}] {post.title}");
    }

    Ok(())
}

// ── Entry point ──────────────────────────────────────────────────────

pub async fn main() -> () or ApiError / net, io {
    val client = ApiClient.new("https://api.example.com".to_string());

    println!("Fetching users...");

    // Fetch and display multiple users.
    val user_ids: [u64]~ = [1, 2, 3];

    for id in &user_ids {
        match display_user_summary(&client, *id).await {
            Ok(()) => {},
            Err(ApiError::NotFound) => eprintln!("User {id} not found, skipping."),
            Err(ApiError::RateLimited(retry_after)) => {
                eprintln!("Rate limited. Waiting {retry_after}s...");
                async_rt::sleep(retry_after * 1000).await;
                // Retry once.
                match display_user_summary(&client, *id).await {
                    Ok(()) => {},
                    Err(e) => eprintln!("Retry failed for user {id}: {e}"),
                }
            },
            Err(e) => eprintln!("Error fetching user {id}: {e}"),
        }
    }

    println!("Done.");
    Ok(())
}
