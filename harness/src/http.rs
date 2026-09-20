//! The loopback HTTP client — the family pattern (hand-rolled, http://
//! only, short timeouts), as the demo crate carries it; the tenant
//! isolation leg needs the bearer-token form against the registries.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

#[allow(dead_code)] // parity with the demo client's response type
pub struct HttpReply {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

impl HttpReply {
    pub fn healthy(&self) -> bool {
        (200..300).contains(&self.status)
    }
}

pub fn request(
    host: &str,
    port: u16,
    method: &str,
    path: &str,
    body: Option<&str>,
    timeout: Duration,
    bearer: Option<&str>,
) -> Option<HttpReply> {
    let mut stream = TcpStream::connect((host, port)).ok()?;
    stream.set_read_timeout(Some(timeout)).ok()?;
    stream.set_write_timeout(Some(timeout)).ok()?;
    let auth = bearer
        .map(|token| format!("authorization: Bearer {token}\r\n"))
        .unwrap_or_default();
    let request = format!(
        "{} {} HTTP/1.1\r\nHost: {}:{}\r\nConnection: close\r\ncontent-type: application/json\r\ncontent-length: {}\r\n{}\r\n{}",
        method,
        path,
        host,
        port,
        body.map(str::len).unwrap_or(0),
        auth,
        body.unwrap_or(""),
    );
    stream.write_all(request.as_bytes()).ok()?;
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).ok()?;
    let text = String::from_utf8_lossy(&raw).into_owned();
    let status = text.split_whitespace().nth(1)?.parse().ok()?;
    let (head, body) = text.split_once("\r\n\r\n")?;
    let headers = head
        .split("\r\n")
        .skip(1)
        .filter_map(|line| {
            let (name, value) = line.split_once(':')?;
            Some((name.trim().to_ascii_lowercase(), value.trim().to_string()))
        })
        .collect();
    Some(HttpReply {
        status,
        headers,
        body: body.to_string(),
    })
}

pub fn get(host: &str, port: u16, path: &str, timeout: Duration) -> Option<HttpReply> {
    request(host, port, "GET", path, None, timeout, None)
}

pub fn post_bearer(
    host: &str,
    port: u16,
    path: &str,
    body: &str,
    timeout: Duration,
    token: &str,
) -> Option<HttpReply> {
    request(host, port, "POST", path, Some(body), timeout, Some(token))
}
