use std::io::{Read as _, Write as _};
use std::net::TcpListener;
use std::time::Duration;

use url::Url;

fn start_http_server_once(status_line: &str, content_type: &str, body: &str) -> Url {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind");
    let addr = listener.local_addr().expect("local_addr");

    let status_line = status_line.to_string();
    let content_type = content_type.to_string();
    let body = body.to_string();

    std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));

        let mut buf = Vec::new();
        let mut tmp = [0u8; 4096];
        for _ in 0..64 {
            match stream.read(&mut tmp) {
                Ok(0) => break,
                Ok(n) => {
                    buf.extend_from_slice(&tmp[..n]);
                    if buf.windows(4).any(|w| w == b"\r\n\r\n") {
                        break;
                    }
                }
                Err(_) => break,
            }
        }

        let resp = format!(
            "HTTP/1.1 {status_line}\r\n\
Content-Type: {content_type}\r\n\
Content-Length: {}\r\n\
Connection: close\r\n\
\r\n\
{body}",
            body.len()
        );
        stream.write_all(resp.as_bytes()).expect("write response");
        let _ = stream.flush();
    });

    Url::parse(&format!("http://{addr}/")).expect("parse server url")
}

#[test]
fn http_post_bytes_includes_error_body_on_http_status() {
    let body =
        r#"{"code":"X07REG_PUBLISH_LINT_FAILED","message":"bad archive","request_id":"req_123"}"#;
    let url = start_http_server_once("400 Bad Request", "application/json", body);

    let err = x07_pkg::http_post_bytes(&url, None, b"hello")
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("HTTP 400"),
        "expected status in error, got: {err}"
    );
    assert!(
        err.contains("X07REG_PUBLISH_LINT_FAILED"),
        "expected error body in error, got: {err}"
    );
}

#[test]
fn http_get_bytes_includes_error_body_on_http_status() {
    let body = r#"{"code":"X07REG_NOT_FOUND","message":"nope"}"#;
    let url = start_http_server_once("404 Not Found", "application/json", body);

    let err = x07_pkg::http_get_bytes(&url, None).unwrap_err().to_string();
    assert!(
        err.contains("HTTP 404"),
        "expected status in error, got: {err}"
    );
    assert!(
        err.contains("X07REG_NOT_FOUND"),
        "expected error body in error, got: {err}"
    );
}
