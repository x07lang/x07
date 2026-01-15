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

#[test]
fn credentials_roundtrip() {
    let _lock = ENV_LOCK.lock().unwrap();

    let dir = create_temp_dir("x07_pkg_creds");
    let old = std::env::var("X07_PKG_HOME").ok();
    std::env::set_var("X07_PKG_HOME", &dir);

    let index = "sparse+file:///tmp/index/";
    x07_pkg::store_token(index, "secret").expect("store token");
    let got = x07_pkg::load_token(index).expect("load token");
    assert_eq!(got.as_deref(), Some("secret"));

    if let Some(old) = old {
        std::env::set_var("X07_PKG_HOME", old);
    } else {
        std::env::remove_var("X07_PKG_HOME");
    }
    rm_rf(&dir);
}

#[test]
fn canonical_index_url_adds_sparse_prefix() {
    let url = x07_pkg::SparseIndexClient::canonical_index_url("file:///tmp/index/").unwrap();
    assert_eq!(url, "sparse+file:///tmp/index/");
}
