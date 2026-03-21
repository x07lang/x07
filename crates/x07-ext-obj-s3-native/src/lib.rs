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
const AWS_REGION: &str = "us-east-1";
const AWS_SERVICE: &str = "s3";
const DATE_ONLY_FORMAT: &[FormatItem<'static>] = format_description!("[year][month][day]");
const AMZ_DATE_FORMAT: &[FormatItem<'static>] =
    format_description!("[year][month][day]T[hour][minute][second]Z");

#[derive(Clone, Debug)]
struct Policy {
    enabled: bool,
    s3_enabled: bool,
    endpoint: Url,
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
    Ok(Policy {
        enabled,
        s3_enabled,
        endpoint: Url::parse(&endpoint)
            .map_err(|err| (objcore::OBJ_ERR_BAD_REQ, err.to_string().into_bytes()))?,
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
    let credential_scope = format!("{date_only}/{AWS_REGION}/{AWS_SERVICE}/aws4_request");
    let string_to_sign = format!(
        "AWS4-HMAC-SHA256\n{amz_date}\n{credential_scope}\n{}",
        hex::encode(Sha256::digest(canonical_request.as_bytes()))
    );
    let signature = hex::encode(sign_aws_v4(
        &policy.secret_key,
        &date_only,
        AWS_REGION,
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
