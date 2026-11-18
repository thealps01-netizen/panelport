//! Test helper: a tiny local HTTP server that answers each connection with the next canned
//! response (then repeats the last one).

use std::io::{Read, Write};

/// Builds a raw HTTP/1.1 response. `content_type` may be empty (header omitted).
pub fn response(status: &str, content_type: &str, extra_headers: &str, body: &str) -> String {
    let ct = if content_type.is_empty() {
        String::new()
    } else {
        format!("Content-Type: {content_type}\r\n")
    };
    format!(
        "HTTP/1.1 {status}\r\n{ct}{extra_headers}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

/// Serves the responses in order on a free loopback port and returns the port.
pub fn serve(responses: Vec<String>) -> u16 {
    serve_n(responses, 50)
}

/// Like `serve`, but answers only `n` connections; then the listener closes and the port
/// refuses connections (a program that stopped).
pub fn serve_n(responses: Vec<String>, n: usize) -> u16 {
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for (i, s) in l.incoming().take(n).enumerate() {
            let Ok(mut s) = s else { continue };
            let mut buf = [0u8; 2048];
            let _ = s.read(&mut buf);
            let r = responses
                .get(i)
                .or(responses.last())
                .cloned()
                .unwrap_or_default();
            let _ = s.write_all(r.as_bytes());
        }
    });
    port
}
