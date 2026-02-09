pub mod ast;
pub mod builtin_modules;
pub mod c_emit;
pub mod cli_specrows;
pub mod compile;
pub mod contracts_elab;
pub mod diagnostics;
pub mod generics;
pub mod guide;
pub mod json_patch;
pub mod language;
pub mod lint;
pub mod native;
pub mod optimize;
pub mod program;
pub mod project;
pub mod stream_pipe;
pub mod typecheck;
pub mod types;
pub mod validate;
pub mod world_config;
pub mod x07ast;

pub const X07C_VERSION: &str = env!("CARGO_PKG_VERSION");

mod fingerprint;
pub mod unify;
