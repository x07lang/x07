use crate::compile::{CompileOptions, CompilerError};
use crate::program::Program;

pub mod features;
pub mod layout;

mod emit_module;

#[derive(Debug, Clone, Copy)]
pub struct WasmMemLimits {
    pub initial_memory_bytes: u64,
    pub max_memory_bytes: u64,
    pub no_growable_memory: bool,
}

#[derive(Debug, Clone)]
pub struct WasmEmitOptions {
    pub mem: WasmMemLimits,
    pub features: features::WasmFeatureSetV1,
}

pub fn emit_solve_pure_wasm_v1(
    program: &Program,
    options: &CompileOptions,
    wasm_opts: &WasmEmitOptions,
) -> Result<Vec<u8>, CompilerError> {
    emit_module::emit_solve_pure_wasm_v1(program, options, wasm_opts)
}
