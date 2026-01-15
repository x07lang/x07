pub const LANG_ID: &str = "x07-core@0.2.0";

pub mod limits {
    pub const MAX_SOURCE_BYTES: usize = 65_536;
    pub const MAX_AST_NODES: usize = 50_000;
    pub const MAX_LOCALS: usize = 512;
    pub const MAX_C_BYTES: usize = 1_048_576;
}
