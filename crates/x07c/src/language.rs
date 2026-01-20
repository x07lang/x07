pub const LANG_ID: &str = "x07-core@0.2.0";

pub mod limits {
    pub const MAX_SOURCE_BYTES: usize = 65_536;
    pub const MAX_AST_NODES: usize = 50_000;
    pub const MAX_LOCALS: usize = 512;
    pub const MAX_C_BYTES: usize = 5 * 1024 * 1024;

    pub fn max_c_bytes() -> usize {
        match std::env::var("X07_MAX_C_BYTES") {
            Ok(v) => v
                .parse::<usize>()
                .ok()
                .filter(|v| *v > 0)
                .unwrap_or(MAX_C_BYTES),
            Err(_) => MAX_C_BYTES,
        }
    }
}
