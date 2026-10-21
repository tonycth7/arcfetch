//! Tiny, dependency-free HTTP/1.0 client.
//!
//! Used to *optionally* fetch a public IP and a one-line weather report for
//! arcfetch. Everything here is best-effort: every entry point returns
//! `Option`/`None` on any error, never panics, always applies a timeout, and
//! never reads more than [`MAX_BODY`] bytes from a socket.
//!
//! This module deliberately depends only on `std` (no `libc`, no crates) so it
//! can be compiled and tested on its own.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

/// Hard cap on how many response bytes we will ever buffer (64 KiB).
const MAX_BODY: usize = 64 * 1024;

/// Maximum accepted length of a cleaned IP address string.
const MAX_IP_LEN: usize = 45;

/// Size of each socket read.
const READ_CHUNK: usize = 8 * 1024;

/// Perform a minimal HTTP/1.0 GET. Returns the response body (may be empty),
/// or `None` on any error/timeout. `host` is a DNS name, `path` starts with '/'.
#[allow(dead_code)]
pub fn http_get(host: &str, port: u16, path: &str, timeout_ms: u64) -> Option<String> {
    let timeout = Duration::from_millis(timeout_ms);

    // Resolve once, then try each address in order until one connects. Each
    // connect is independently bounded by `timeout`, so this never hangs.
    let addrs: Vec<SocketAddr> = (host, port).to_socket_addrs().ok()?.collect();
    if addrs.is_empty() {
        return None;
    }

    let mut stream: Option<TcpStream> = None;
    for addr in addrs {
        if let Ok(s) = TcpStream::connect_timeout(&addr, timeout) {
            stream = Some(s);
            break;
        }
    }
    let mut stream = stream?;

    // Bound every subsequent read and write.
    stream.set_read_timeout(Some(timeout)).ok()?;
    stream.set_write_timeout(Some(timeout)).ok()?;

    let request = format!(
        "GET {path} HTTP/1.0\r\n\
         Host: {host}\r\n\
         User-Agent: arcfetch/{}\r\n\
         Accept: */*\r\n\
         Connection: close\r\n\r\n",
        env!("CARGO_PKG_VERSION")
    );
    stream.write_all(request.as_bytes()).ok()?;

    // Read the response, never buffering more than MAX_BODY bytes. The timeout
    // is a total deadline, not a per-read timeout, so a dribbling server can't
    // stretch the fetch indefinitely.
    let start = Instant::now();
    let mut timed_out = false;
    let mut buf: Vec<u8> = Vec::with_capacity(READ_CHUNK);
    let mut chunk = [0u8; READ_CHUNK];
    loop {
        let remaining = MAX_BODY - buf.len();
        if remaining == 0 {
            break;
        }
        let left = timeout.saturating_sub(start.elapsed());
        if left.is_zero() { timed_out = true; break; }
        stream.set_read_timeout(Some(left)).ok()?;
        match stream.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
                let take = n.min(remaining);
                buf.extend_from_slice(&chunk[..take]);
                if take < n {
                    // Cap reached mid-read; stop.
                    break;
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(ref e) if matches!(e.kind(),
                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {
                timed_out = true;
                break;
            }
            Err(_) => break,
        }
    }
    if timed_out {
        return None;
    }

    let text = String::from_utf8_lossy(&buf);

    // A valid HTTP response must begin with a 2xx status line.
    if !text.starts_with("HTTP/") {
        return None;
    }
    if !text.split_whitespace().nth(1).is_some_and(|s| s.starts_with('2')) {
        return None;
    }

    // Split headers from body on the first blank line. If there is none, the
    // body is empty (headers only). We cannot decode chunked framing, so treat
    // it as a failure rather than returning chunk-size lines as the value.
    let (headers, body) = text.split_once("\r\n\r\n").unwrap_or((&text, ""));
    let chunked = headers.lines().any(|l| {
        l.split_once(':').is_some_and(|(k, v)| {
            k.eq_ignore_ascii_case("transfer-encoding") && v.to_ascii_lowercase().contains("chunked")
        })
    });
    if chunked {
        return None;
    }

    Some(body.trim().to_string())
}

/// Best-effort public IPv4/IPv6 address. Returns `None` on failure.
#[allow(dead_code)]
pub fn public_ip() -> Option<String> {
    const SOURCES: &[(&str, u16, &str)] = &[
        ("ifconfig.me", 80, "/ip"),
        ("api.ipify.org", 80, "/"),
        ("icanhazip.com", 80, "/"),
    ];

    for (host, port, path) in SOURCES {
        if let Some(body) = http_get(host, *port, path, 1500) {
            if let Some(ip) = clean_ip(&body) {
                return Some(ip);
            }
        }
    }
    None
}

/// Best-effort one-line weather for `location` (empty string = auto/geo-IP).
/// Returns `None` on failure.
#[allow(dead_code)]
pub fn weather(location: &str) -> Option<String> {
    let path = if location.is_empty() {
        "/?format=%l:+%c+%t+%w".to_string()
    } else {
        format!("/{location}?format=%l:+%c+%t+%w")
    };

    let body = http_get("wttr.in", 80, &path, 2000)?;
    let body = body.trim();
    if body.is_empty() {
        None
    } else {
        Some(body.to_string())
    }
}

/// Reduce a raw response to a single plausible IP token, or `None`. Takes the
/// first whitespace-delimited token, requires it to look like an address, and
/// caps the result at [`MAX_IP_LEN`] characters.
fn clean_ip(raw: &str) -> Option<String> {
    let token = raw.split_whitespace().next()?;
    if token.len() > MAX_IP_LEN {
        return None;
    }
    if !token.contains('.') && !token.contains(':') {
        return None;
    }
    Some(token.to_string())
}
