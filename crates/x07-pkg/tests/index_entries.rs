use std::path::PathBuf;
use std::sync::Mutex;

static ENV_LOCK: Mutex<()> = Mutex::new(());

fn create_temp_dir(prefix: &str) -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    let base = std::env::temp_dir();
    let pid = std::process::id();
    for _ in 0..10_000 {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = base.join(format!("{prefix}_{pid}_{n}"));
        if std::fs::create_dir(&path).is_ok() {
            return path;
        }
    }
    panic!("failed to create temp dir under {}", base.display());
}

fn rm_rf(path: &std::path::Path) {
    let _ = std::fs::remove_dir_all(path);
}

fn setup_index_dir() -> (PathBuf, String) {
    let _lock = ENV_LOCK.lock().unwrap();

    let dir = create_temp_dir("x07_pkg_index");
    let index_dir = dir.join("index");
    std::fs::create_dir_all(&index_dir).expect("create index dir");

    let config = serde_json::json!({
        "dl": "http://example.invalid/v1/packages/",
        "api": "http://example.invalid/v1/",
        "auth-required": false,
    });
    std::fs::write(
        index_dir.join("config.json"),
        serde_json::to_vec_pretty(&config).expect("encode config"),
    )
    .expect("write config");

    let index_url = format!("file://{}/", index_dir.display());
    (dir, index_url)
}

fn write_index_entry(index_dir: &PathBuf, package: &str, ndjson: &str) {
    let p = match package.len() {
        1 => index_dir.join("1").join(package),
        2 => index_dir.join("2").join(package),
        3 => index_dir.join("3").join(&package[0..1]).join(package),
        _ => index_dir
            .join(&package[0..2])
            .join(&package[2..4])
            .join(package),
    };
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).expect("create shard dir");
    }
    std::fs::write(p, ndjson).expect("write index file");
}

#[test]
fn fetch_entries_requires_yanked_field() {
    let (dir, index_url) = setup_index_dir();
    let index_dir = dir.join("index");

    write_index_entry(
        &index_dir,
        "hello",
        r#"{"schema_version":"x07.index-entry@0.1.0","name":"hello","version":"0.1.0","cksum":"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"}"#,
    );

    let client = x07_pkg::SparseIndexClient::from_index_url(&index_url, None).unwrap();
    let err = client.fetch_entries("hello").unwrap_err();
    let err_text = format!("{err:#}");
    assert!(
        err_text.contains("missing field `yanked`"),
        "expected error mentioning missing yanked field, got: {err_text}"
    );

    rm_rf(&dir);
}

#[test]
fn fetch_entries_enforces_schema_version() {
    let (dir, index_url) = setup_index_dir();
    let index_dir = dir.join("index");

    write_index_entry(
        &index_dir,
        "hello",
        r#"{"schema_version":"x07.index-entry@0.0.0","name":"hello","version":"0.1.0","cksum":"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef","yanked":false}"#,
    );

    let client = x07_pkg::SparseIndexClient::from_index_url(&index_url, None).unwrap();
    let err = client.fetch_entries("hello").unwrap_err();
    let err_text = format!("{err:#}");
    assert!(
        err_text.contains("schema_version mismatch"),
        "expected schema_version mismatch error, got: {err_text}"
    );

    rm_rf(&dir);
}

#[test]
fn fetch_entries_accepts_valid_entries() {
    let (dir, index_url) = setup_index_dir();
    let index_dir = dir.join("index");

    write_index_entry(
        &index_dir,
        "hello",
        r#"{"schema_version":"x07.index-entry@0.1.0","name":"hello","version":"0.1.0","cksum":"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef","yanked":false}"#,
    );

    let client = x07_pkg::SparseIndexClient::from_index_url(&index_url, None).unwrap();
    let entries = client.fetch_entries("hello").unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].name, "hello");
    assert_eq!(entries[0].version, "0.1.0");
    assert_eq!(
        entries[0].cksum,
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
    );
    assert!(!entries[0].yanked);

    rm_rf(&dir);
}
