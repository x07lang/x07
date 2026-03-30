#![allow(non_camel_case_types)]
#![allow(clippy::missing_safety_doc)]

use hmac::{Hmac, Mac};
use reqwest::{
    blocking::{Client, Response},
    header, Url,
};
use sha2::{Digest, Sha256};
use time::{format_description::FormatItem, macros::format_description, OffsetDateTime};
use x07_ext_obj_native_core as objcore;

type HmacSha256 = Hmac<Sha256>;
type ev_bytes = objcore::ev_bytes;

const REQ_MAGIC: &[u8; 4] = b"X7OR";
const REQ_VERSION: u32 = 1;
const MAX_REQ_BYTES_DEFAULT: u32 = 1024 * 1024;
const MAX_PUT_BYTES_DEFAULT: u32 = 16 * 1024 * 1024;
const MAX_RESP_BYTES_DEFAULT: u32 = 32 * 1024 * 1024;
const AWS_REGION_DEFAULT: &str = "us-east-1";
const AWS_SERVICE: &str = "s3";
const DATE_ONLY_FORMAT: &[FormatItem<'static>] = format_description!("[year][month][day]");
const AMZ_DATE_FORMAT: &[FormatItem<'static>] =
    format_description!("[year][month][day]T[hour][minute][second]Z");

#[derive(Clone, Debug)]
struct Policy {
    enabled: bool,
    s3_enabled: bool,
    endpoint: Url,
    aws_region: String,
    default_bucket: String,
    access_key: String,
    secret_key: String,
    key_prefix: Option<String>,
    max_req_bytes: u32,
    max_put_bytes: u32,
    max_resp_bytes: u32,
}

#[derive(Debug)]
struct ObjRequest<'a> {
    op: u32,
    uri: &'a [u8],
    body: &'a [u8],
}

#[derive(Debug)]
struct Target {
    bucket: String,
    key: String,
}

fn policy() -> Result<Policy, (u32, Vec<u8>)> {
    let sandboxed = objcore::env_bool("X07_OS_SANDBOXED", false);
    let enabled = objcore::env_bool("X07_OS_OBJ", !sandboxed);
    if !enabled {
        return Err((
            objcore::OBJ_ERR_POLICY_DENIED,
            b"object store access disabled".to_vec(),
        ));
    }
    let s3_enabled = objcore::env_bool("X07_OS_OBJ_S3", !sandboxed);
    if !s3_enabled {
        return Err((
            objcore::OBJ_ERR_POLICY_DENIED,
            b"s3 backend disabled".to_vec(),
        ));
    }
    let endpoint = env_required("X07_OS_OBJ_S3_ENDPOINT")?;
    let default_bucket = env_required("X07_OS_OBJ_S3_BUCKET")?;
    let access_key = env_required("X07_OS_OBJ_S3_ACCESS_KEY")?;
    let secret_key = env_required("X07_OS_OBJ_S3_SECRET_KEY")?;
    let endpoint = Url::parse(&endpoint)
        .map_err(|err| (objcore::OBJ_ERR_BAD_REQ, err.to_string().into_bytes()))?;
    let aws_region = std::env::var("X07_OS_OBJ_S3_REGION")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .or_else(|| endpoint.host_str().and_then(infer_aws_region))
        .unwrap_or_else(|| AWS_REGION_DEFAULT.to_string());
    Ok(Policy {
        enabled,
        s3_enabled,
        endpoint,
        aws_region,
        default_bucket,
        access_key,
        secret_key,
        key_prefix: std::env::var("X07_OS_OBJ_S3_PREFIX")
            .ok()
            .map(|value| value.trim_matches('/').to_string())
            .filter(|value| !value.is_empty()),
        max_req_bytes: objcore::env_u32_nonzero("X07_OS_OBJ_MAX_REQ_BYTES", MAX_REQ_BYTES_DEFAULT),
        max_put_bytes: objcore::env_u32_nonzero("X07_OS_OBJ_MAX_PUT_BYTES", MAX_PUT_BYTES_DEFAULT),
        max_resp_bytes: objcore::env_u32_nonzero(
            "X07_OS_OBJ_MAX_RESP_BYTES",
            MAX_RESP_BYTES_DEFAULT,
        ),
    })
}

fn env_required(name: &str) -> Result<String, (u32, Vec<u8>)> {
    let value = std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            (
                objcore::OBJ_ERR_BAD_REQ,
                format!("missing environment variable {name}").into_bytes(),
            )
        })?;
    Ok(value)
}

fn infer_aws_region(host: &str) -> Option<String> {
    let host = host.split(':').next().unwrap_or(host);
    let parts: Vec<&str> = host.split('.').collect();
    for (idx, part) in parts.iter().enumerate() {
        if *part == "s3" {
            match parts.get(idx + 1) {
                Some(region) if *region != "amazonaws" && !region.is_empty() => {
                    return Some((*region).to_string());
                }
                _ => return None,
            }
        }
        if let Some(region) = part.strip_prefix("s3-") {
            if !region.is_empty() {
                return Some(region.to_string());
            }
        }
    }
    None
}

fn read_u32_le(b: &[u8], off: usize) -> Option<u32> {
    let slice = b.get(off..off + 4)?;
    Some(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

fn parse_request(req: &[u8]) -> Result<ObjRequest<'_>, (u32, Vec<u8>)> {
    if req.len() < 20 {
        return Err((objcore::OBJ_ERR_BAD_REQ, b"short object request".to_vec()));
    }
    if &req[0..4] != REQ_MAGIC {
        return Err((
            objcore::OBJ_ERR_BAD_REQ,
            b"bad object request magic".to_vec(),
        ));
    }
    let version = read_u32_le(req, 4).ok_or_else(|| {
        (
            objcore::OBJ_ERR_BAD_REQ,
            b"missing request version".to_vec(),
        )
    })?;
    if version != REQ_VERSION {
        return Err((
            objcore::OBJ_ERR_BAD_REQ,
            b"unsupported object request version".to_vec(),
        ));
    }
    let op = read_u32_le(req, 8).ok_or_else(|| {
        (
            objcore::OBJ_ERR_BAD_REQ,
            b"missing object operation".to_vec(),
        )
    })?;
    let uri_len = read_u32_le(req, 12)
        .ok_or_else(|| (objcore::OBJ_ERR_BAD_REQ, b"missing uri length".to_vec()))?
        as usize;
    let body_len = read_u32_le(req, 16)
        .ok_or_else(|| (objcore::OBJ_ERR_BAD_REQ, b"missing body length".to_vec()))?
        as usize;
    let uri_start = 20usize;
    let uri_end = uri_start
        .checked_add(uri_len)
        .ok_or_else(|| (objcore::OBJ_ERR_BAD_REQ, b"uri length overflow".to_vec()))?;
    let body_end = uri_end
        .checked_add(body_len)
        .ok_or_else(|| (objcore::OBJ_ERR_BAD_REQ, b"body length overflow".to_vec()))?;
    if body_end != req.len() {
        return Err((
            objcore::OBJ_ERR_BAD_REQ,
            b"malformed object request".to_vec(),
        ));
    }
    Ok(ObjRequest {
        op,
        uri: &req[uri_start..uri_end],
        body: &req[uri_end..body_end],
    })
}

fn parse_target(policy: &Policy, uri: &[u8]) -> Result<Target, (u32, Vec<u8>)> {
    let raw = std::str::from_utf8(uri)
        .map_err(|_| (objcore::OBJ_ERR_BAD_REQ, b"uri is not utf-8".to_vec()))?
        .trim();
    if raw.is_empty() {
        return Err((objcore::OBJ_ERR_BAD_REQ, b"uri is required".to_vec()));
    }
    let (bucket, key) = if let Some(rest) = raw.strip_prefix("s3://") {
        let (bucket, key) = rest.split_once('/').ok_or_else(|| {
            (
                objcore::OBJ_ERR_BAD_REQ,
                b"s3 uri must include bucket and key".to_vec(),
            )
        })?;
        (bucket.to_string(), key.to_string())
    } else {
        (
            policy.default_bucket.clone(),
            raw.trim_start_matches('/').to_string(),
        )
    };
    if bucket.trim().is_empty() {
        return Err((objcore::OBJ_ERR_BAD_REQ, b"bucket is required".to_vec()));
    }
    let key = key.trim_matches('/').to_string();
    if key.is_empty() {
        return Err((objcore::OBJ_ERR_BAD_REQ, b"object key is required".to_vec()));
    }
    let key = match &policy.key_prefix {
        Some(prefix) => format!("{prefix}/{key}"),
        None => key,
    };
    Ok(Target { bucket, key })
}

fn object_url(endpoint: &Url, target: &Target) -> Result<Url, (u32, Vec<u8>)> {
    let mut url = endpoint.clone();
    url.set_path(&format!(
        "/{}/{}",
        target.bucket,
        target.key.trim_start_matches('/')
    ));
    Ok(url)
}

fn endpoint_host(endpoint: &Url) -> Result<String, (u32, Vec<u8>)> {
    let host = endpoint.host_str().ok_or_else(|| {
        (
            objcore::OBJ_ERR_BAD_REQ,
            b"object endpoint is missing a host".to_vec(),
        )
    })?;
    Ok(match endpoint.port() {
        Some(port) => format!("{host}:{port}"),
        None => host.to_string(),
    })
}

fn sign_aws_v4(
    secret: &str,
    date: &str,
    region: &str,
    service: &str,
    payload: &[u8],
) -> Result<Vec<u8>, (u32, Vec<u8>)> {
    let k_date = hmac_sha256(format!("AWS4{secret}").as_bytes(), date.as_bytes())?;
    let k_region = hmac_sha256(&k_date, region.as_bytes())?;
    let k_service = hmac_sha256(&k_region, service.as_bytes())?;
    let k_signing = hmac_sha256(&k_service, b"aws4_request")?;
    hmac_sha256(&k_signing, payload)
}

fn hmac_sha256(key: &[u8], data: &[u8]) -> Result<Vec<u8>, (u32, Vec<u8>)> {
    let mut mac = HmacSha256::new_from_slice(key)
        .map_err(|err| (objcore::OBJ_ERR_BAD_REQ, err.to_string().into_bytes()))?;
    mac.update(data);
    Ok(mac.finalize().into_bytes().to_vec())
}

fn signed_request(
    policy: &Policy,
    method: &str,
    target: &Target,
    body: &[u8],
    content_type: Option<&str>,
) -> Result<Response, (u32, Vec<u8>)> {
    let url = object_url(&policy.endpoint, target)?;
    let host = endpoint_host(&policy.endpoint)?;
    let now = OffsetDateTime::now_utc();
    let amz_date = now
        .format(AMZ_DATE_FORMAT)
        .map_err(|err| (objcore::OBJ_ERR_BAD_REQ, err.to_string().into_bytes()))?;
    let date_only = now
        .date()
        .with_hms(0, 0, 0)
        .map_err(|err| (objcore::OBJ_ERR_BAD_REQ, err.to_string().into_bytes()))?
        .format(DATE_ONLY_FORMAT)
        .map_err(|err| (objcore::OBJ_ERR_BAD_REQ, err.to_string().into_bytes()))?;
    let payload_hash = hex::encode(Sha256::digest(body));
    let canonical_headers =
        format!("host:{host}\nx-amz-content-sha256:{payload_hash}\nx-amz-date:{amz_date}\n");
    let signed_headers = "host;x-amz-content-sha256;x-amz-date";
    let canonical_request = format!(
        "{method}\n{}\n\n{canonical_headers}\n{signed_headers}\n{payload_hash}",
        url.path()
    );
    let credential_scope = format!(
        "{date_only}/{}/{AWS_SERVICE}/aws4_request",
        policy.aws_region
    );
    let string_to_sign = format!(
        "AWS4-HMAC-SHA256\n{amz_date}\n{credential_scope}\n{}",
        hex::encode(Sha256::digest(canonical_request.as_bytes()))
    );
    let signature = hex::encode(sign_aws_v4(
        &policy.secret_key,
        &date_only,
        &policy.aws_region,
        AWS_SERVICE,
        string_to_sign.as_bytes(),
    )?);
    let authorization = format!(
        "AWS4-HMAC-SHA256 Credential={}/{}, SignedHeaders={}, Signature={}",
        policy.access_key, credential_scope, signed_headers, signature
    );
    let client = Client::builder()
        .build()
        .map_err(|err| (objcore::OBJ_ERR_IO, err.to_string().into_bytes()))?;
    let mut request = client
        .request(
            method
                .parse::<reqwest::Method>()
                .map_err(|err| (objcore::OBJ_ERR_BAD_REQ, err.to_string().into_bytes()))?,
            url,
        )
        .header("authorization", authorization)
        .header("host", host)
        .header("x-amz-content-sha256", payload_hash)
        .header("x-amz-date", amz_date);
    if let Some(content_type) = content_type {
        request = request.header(header::CONTENT_TYPE, content_type);
    }
    if matches!(method, "PUT") {
        request = request.body(body.to_vec());
    }
    request
        .send()
        .map_err(|err| (objcore::OBJ_ERR_IO, err.to_string().into_bytes()))
}

fn build_head_payload(response: &Response) -> Vec<u8> {
    let content_length = response
        .headers()
        .get(header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("0");
    let etag = response
        .headers()
        .get(header::ETAG)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.replace('"', "\\\""));
    match etag {
        Some(etag) => {
            format!("{{\"exists\":true,\"content_length\":{content_length},\"etag\":\"{etag}\"}}")
                .into_bytes()
        }
        None => format!("{{\"exists\":true,\"content_length\":{content_length}}}").into_bytes(),
    }
}

fn error_bytes(op: u32, code: u32, msg: Vec<u8>) -> ev_bytes {
    objcore::alloc_return_bytes(&objcore::evobj_err(op, code, &msg))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn x07_obj_s3_dispatch_v1(req: ev_bytes, _caps: ev_bytes) -> ev_bytes {
    let req = objcore::bytes_as_slice(req);
    let parsed = match parse_request(req) {
        Ok(parsed) => parsed,
        Err((code, msg)) => return error_bytes(objcore::OP_GET_V1, code, msg),
    };
    let op = parsed.op;
    let policy = match policy() {
        Ok(policy) => policy,
        Err((code, msg)) => return error_bytes(op, code, msg),
    };
    if !policy.enabled || !policy.s3_enabled {
        return error_bytes(
            op,
            objcore::OBJ_ERR_POLICY_DENIED,
            b"s3 backend disabled".to_vec(),
        );
    }
    if req.len() as u32 > policy.max_req_bytes {
        return error_bytes(
            op,
            objcore::OBJ_ERR_TOO_LARGE,
            b"request too large".to_vec(),
        );
    }
    let target = match parse_target(&policy, parsed.uri) {
        Ok(target) => target,
        Err((code, msg)) => return error_bytes(op, code, msg),
    };

    let result = match op {
        objcore::OP_HEAD_V1 => {
            signed_request(&policy, "HEAD", &target, &[], None).and_then(|response| {
                if response.status().is_success() {
                    Ok(build_head_payload(&response))
                } else if response.status().as_u16() == 404 {
                    Err((objcore::OBJ_ERR_NOT_FOUND, b"object not found".to_vec()))
                } else {
                    Err((
                        objcore::OBJ_ERR_IO,
                        format!("object store HEAD failed: {}", response.status()).into_bytes(),
                    ))
                }
            })
        }
        objcore::OP_GET_V1 => {
            signed_request(&policy, "GET", &target, &[], None).and_then(|response| {
                if response.status().as_u16() == 404 {
                    return Err((objcore::OBJ_ERR_NOT_FOUND, b"object not found".to_vec()));
                }
                if !response.status().is_success() {
                    return Err((
                        objcore::OBJ_ERR_IO,
                        format!("object store GET failed: {}", response.status()).into_bytes(),
                    ));
                }
                let body = response
                    .bytes()
                    .map_err(|err| (objcore::OBJ_ERR_IO, err.to_string().into_bytes()))?
                    .to_vec();
                if body.len() as u32 > policy.max_resp_bytes {
                    return Err((objcore::OBJ_ERR_TOO_LARGE, b"response too large".to_vec()));
                }
                Ok(body)
            })
        }
        objcore::OP_PUT_V1 => {
            if parsed.body.len() as u32 > policy.max_put_bytes {
                Err((objcore::OBJ_ERR_TOO_LARGE, b"put body too large".to_vec()))
            } else {
                signed_request(
                    &policy,
                    "PUT",
                    &target,
                    parsed.body,
                    Some("application/octet-stream"),
                )
                .and_then(|response| {
                    if response.status().is_success() {
                        Ok(Vec::new())
                    } else {
                        Err((
                            objcore::OBJ_ERR_IO,
                            format!("object store PUT failed: {}", response.status()).into_bytes(),
                        ))
                    }
                })
            }
        }
        objcore::OP_DELETE_V1 => {
            signed_request(&policy, "DELETE", &target, &[], None).and_then(|response| {
                if response.status().is_success() || response.status().as_u16() == 404 {
                    Ok(Vec::new())
                } else {
                    Err((
                        objcore::OBJ_ERR_IO,
                        format!("object store DELETE failed: {}", response.status()).into_bytes(),
                    ))
                }
            })
        }
        other => Err((
            objcore::OBJ_ERR_BAD_REQ,
            format!("unsupported object op: {other}").into_bytes(),
        )),
    };

    match result {
        Ok(payload) => objcore::alloc_return_bytes(&objcore::evobj_ok(op, &payload)),
        Err((code, msg)) => error_bytes(op, code, msg),
    }
}

#[cfg(test)]
mod region_tests {
    use super::infer_aws_region;

    #[test]
    fn infer_region_service_endpoint() {
        assert_eq!(
            infer_aws_region("s3.us-west-1.amazonaws.com").as_deref(),
            Some("us-west-1")
        );
    }

    #[test]
    fn infer_region_bucket_endpoint() {
        assert_eq!(
            infer_aws_region("my-bucket.s3.us-west-2.amazonaws.com").as_deref(),
            Some("us-west-2")
        );
        assert_eq!(infer_aws_region("my-bucket.s3.amazonaws.com"), None);
    }

    #[test]
    fn infer_region_dash_style() {
        assert_eq!(
            infer_aws_region("s3-eu-central-1.amazonaws.com").as_deref(),
            Some("eu-central-1")
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::sync::mpsc::{self, Receiver};
    use std::thread;
    use std::time::Duration;

    #[derive(Debug)]
    struct CapturedRequest {
        method: String,
        path: String,
        headers: HashMap<String, String>,
        body: Vec<u8>,
    }

    fn test_policy(endpoint: Url) -> Policy {
        Policy {
            enabled: true,
            s3_enabled: true,
            endpoint,
            aws_region: AWS_REGION_DEFAULT.to_string(),
            default_bucket: "demo-bucket".to_string(),
            access_key: "minio".to_string(),
            secret_key: "minio123".to_string(),
            key_prefix: Some("services/demo".to_string()),
            max_req_bytes: MAX_REQ_BYTES_DEFAULT,
            max_put_bytes: MAX_PUT_BYTES_DEFAULT,
            max_resp_bytes: MAX_RESP_BYTES_DEFAULT,
        }
    }

    fn start_http_server(
        responses: Vec<&'static str>,
    ) -> (Url, Receiver<CapturedRequest>, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
        let addr = listener.local_addr().expect("server address");
        let (tx, rx) = mpsc::channel();
        let handle = thread::spawn(move || {
            for response in responses {
                let (mut stream, _) = listener.accept().expect("accept test request");
                let captured = read_request(&mut stream).expect("read test request");
                tx.send(captured).expect("send captured request");
                stream
                    .write_all(response.as_bytes())
                    .expect("write test response");
            }
        });
        let endpoint = Url::parse(&format!("http://{}", addr)).expect("endpoint url");
        (endpoint, rx, handle)
    }

    fn read_request(stream: &mut TcpStream) -> std::io::Result<CapturedRequest> {
        stream.set_read_timeout(Some(Duration::from_secs(2)))?;
        let mut buffer = Vec::new();
        let mut chunk = [0u8; 1024];
        let mut header_end = None;
        let mut content_length = 0usize;
        loop {
            let read = stream.read(&mut chunk)?;
            if read == 0 {
                break;
            }
            buffer.extend_from_slice(&chunk[..read]);
            if header_end.is_none() {
                if let Some(index) = find_header_end(&buffer) {
                    header_end = Some(index + 4);
                    let header_text = String::from_utf8_lossy(&buffer[..index]);
                    for line in header_text.lines().skip(1) {
                        if let Some((name, value)) = line.split_once(':') {
                            if name.eq_ignore_ascii_case("content-length") {
                                content_length = value.trim().parse::<usize>().unwrap_or(0);
                            }
                        }
                    }
                }
            }
            if let Some(header_end) = header_end {
                if buffer.len() >= header_end + content_length {
                    break;
                }
            }
        }

        let header_end = header_end.expect("request headers");
        let header_text = String::from_utf8_lossy(&buffer[..header_end - 4]);
        let mut lines = header_text.lines();
        let request_line = lines.next().expect("request line");
        let mut parts = request_line.split_whitespace();
        let method = parts.next().unwrap_or_default().to_string();
        let path = parts.next().unwrap_or_default().to_string();
        let mut headers = HashMap::new();
        for line in lines {
            if let Some((name, value)) = line.split_once(':') {
                headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_string());
            }
        }
        let body = buffer[header_end..header_end + content_length].to_vec();
        Ok(CapturedRequest {
            method,
            path,
            headers,
            body,
        })
    }

    fn find_header_end(buffer: &[u8]) -> Option<usize> {
        buffer.windows(4).position(|window| window == b"\r\n\r\n")
    }

    #[test]
    fn parse_target_supports_relative_and_s3_uris() {
        let endpoint = Url::parse("http://127.0.0.1:9000").expect("endpoint");
        let policy = test_policy(endpoint);

        let default_target = parse_target(&policy, b"documents/report.json").expect("default");
        assert_eq!(default_target.bucket, "demo-bucket");
        assert_eq!(default_target.key, "services/demo/documents/report.json");

        let explicit_target =
            parse_target(&policy, b"s3://alt-bucket/archive/item.json").expect("explicit");
        assert_eq!(explicit_target.bucket, "alt-bucket");
        assert_eq!(explicit_target.key, "services/demo/archive/item.json");
    }

    #[test]
    fn signed_request_round_trips_against_local_http_server() {
        let (endpoint, requests, handle) = start_http_server(vec![
            "HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n",
            "HTTP/1.1 200 OK\r\nContent-Length: 0\r\nETag: \"etag-demo\"\r\n\r\n",
            "HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\npong",
            "HTTP/1.1 204 No Content\r\nContent-Length: 0\r\n\r\n",
        ]);
        let policy = test_policy(endpoint);
        let target = parse_target(&policy, b"documents/report.json").expect("target");

        let put_response = signed_request(
            &policy,
            "PUT",
            &target,
            b"ping",
            Some("application/octet-stream"),
        )
        .expect("put response");
        assert!(put_response.status().is_success());
        let put_request = requests.recv().expect("captured put request");
        assert_eq!(put_request.method, "PUT");
        assert_eq!(
            put_request.path,
            "/demo-bucket/services/demo/documents/report.json"
        );
        assert_eq!(put_request.body, b"ping");
        assert!(put_request.headers.contains_key("authorization"));
        assert!(put_request.headers.contains_key("x-amz-content-sha256"));
        assert!(put_request.headers.contains_key("x-amz-date"));

        let head_response =
            signed_request(&policy, "HEAD", &target, &[], None).expect("head response");
        assert!(head_response.status().is_success());
        let head_payload = build_head_payload(&head_response);
        let head_text = String::from_utf8(head_payload).expect("head payload utf8");
        assert!(head_text.contains("\"exists\":true"));
        assert!(head_text.contains("etag-demo"));
        let head_request = requests.recv().expect("captured head request");
        assert_eq!(head_request.method, "HEAD");
        assert!(head_request.body.is_empty());

        let get_response = signed_request(&policy, "GET", &target, &[], None).expect("get");
        let get_body = get_response.bytes().expect("get body");
        assert_eq!(get_body.as_ref(), b"pong");
        let get_request = requests.recv().expect("captured get request");
        assert_eq!(get_request.method, "GET");

        let delete_response =
            signed_request(&policy, "DELETE", &target, &[], None).expect("delete");
        assert_eq!(delete_response.status().as_u16(), 204);
        let delete_request = requests.recv().expect("captured delete request");
        assert_eq!(delete_request.method, "DELETE");

        handle.join().expect("join test server");
    }
}
