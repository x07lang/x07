use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use crate::c_emit;
use crate::diagnostics::{Location, Severity};
use crate::generics;
use crate::guide;
use crate::language;
use crate::module_source;
use crate::native::NativeRequires;
use crate::optimize;
use crate::program::Program;
use crate::stream_pipe;
use crate::types::Ty;
use crate::x07ast;
use x07_contracts::{NATIVE_REQUIRES_SCHEMA_VERSION, X07AST_SCHEMA_VERSION_V0_5_0};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ContractMode {
    /// Runtime contract checks trap with a structured payload (default).
    #[default]
    RuntimeTrap,
    /// Verification mode: contracts lower to `__CPROVER_assume` / `__CPROVER_assert`.
    VerifyBmc,
}

#[derive(Debug, Clone)]
pub struct CompileOptions {
    pub world: x07_worlds::WorldId,
    pub enable_fs: bool,
    pub enable_rr: bool,
    pub enable_kv: bool,
    pub module_roots: Vec<std::path::PathBuf>,
    pub arch_root: Option<std::path::PathBuf>,
    pub emit_main: bool,
    pub freestanding: bool,
    pub contract_mode: ContractMode,
    pub allow_unsafe: Option<bool>,
    pub allow_ffi: Option<bool>,
}

impl Default for CompileOptions {
    fn default() -> Self {
        Self {
            world: x07_worlds::WorldId::default(),
            enable_fs: false,
            enable_rr: false,
            enable_kv: false,
            module_roots: Vec::new(),
            arch_root: None,
            emit_main: true,
            freestanding: false,
            contract_mode: ContractMode::default(),
            allow_unsafe: None,
            allow_ffi: None,
        }
    }
}

impl CompileOptions {
    pub fn allow_unsafe(&self) -> bool {
        self.allow_unsafe
            .unwrap_or_else(|| self.world.caps().allow_unsafe)
    }

    pub fn allow_ffi(&self) -> bool {
        self.allow_ffi
            .unwrap_or_else(|| self.world.caps().allow_ffi)
    }

    pub fn hint_enable_unsafe(&self) -> String {
        if self.world.caps().allow_unsafe {
            "unsafe is disabled (allow_unsafe=false)".to_string()
        } else {
            "compile with --world run-os or --world run-os-sandboxed".to_string()
        }
    }

    pub fn hint_enable_ffi(&self) -> String {
        if self.world.caps().allow_ffi {
            "ffi is disabled (allow_ffi=false)".to_string()
        } else {
            "compile with --world run-os or --world run-os-sandboxed".to_string()
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct CompileStats {
    pub fuel_used: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompileErrorKind {
    Parse,
    Typing,
    Unsupported,
    Budget,
    Internal,
}

#[derive(Debug, Clone)]
pub struct CompilerError {
    pub kind: CompileErrorKind,
    pub message: String,
}

impl CompilerError {
    pub fn new(kind: CompileErrorKind, message: String) -> Self {
        Self { kind, message }
    }
}

pub fn guide_md() -> String {
    guide::guide_md()
}

pub fn compile_program_to_c(
    program: &[u8],
    options: &CompileOptions,
) -> Result<String, CompilerError> {
    compile_program_to_c_with_meta(program, options).map(|out| out.c_src)
}

pub fn compile_program_to_c_with_stats(
    program: &[u8],
    options: &CompileOptions,
) -> Result<(String, CompileStats), CompilerError> {
    let out = compile_program_to_c_with_meta(program, options)?;
    Ok((out.c_src, out.stats))
}

#[derive(Debug, Clone)]
pub struct CompileToCOutput {
    pub c_src: String,
    pub stats: CompileStats,
    pub native_requires: NativeRequires,
    pub mono_map: Option<crate::generics::MonoMapV1>,
}

pub fn compile_program_to_c_with_meta(
    program: &[u8],
    options: &CompileOptions,
) -> Result<CompileToCOutput, CompilerError> {
    let FrontendOutput {
        mut parsed_program,
        mono_map,
        module_infos,
        mut fuel_used,
    } = compile_frontend(program, options)?;

    // Optimize solve expression.
    parsed_program.solve = optimize::optimize_expr(parsed_program.solve);
    fuel_used = fuel_used.saturating_add(parsed_program.solve.node_count() as u64);

    // Optimize function bodies.
    for f in &mut parsed_program.functions {
        f.body = optimize::optimize_expr(f.body.clone());
        fuel_used = fuel_used.saturating_add(f.body.node_count() as u64);
    }

    // Optimize async function bodies.
    for f in &mut parsed_program.async_functions {
        f.body = optimize::optimize_expr(f.body.clone());
        fuel_used = fuel_used.saturating_add(f.body.node_count() as u64);
    }

    validate_program_visibility(&parsed_program, &module_infos)?;
    forbid_internal_only_heads_in_non_builtin_code(&parsed_program, &module_infos)?;

    let contract_nodes = |clauses: &[crate::x07ast::ContractClauseAst]| -> usize {
        clauses
            .iter()
            .map(|c| c.expr.node_count() + c.witness.iter().map(|w| w.node_count()).sum::<usize>())
            .sum()
    };

    let total_nodes: usize = parsed_program.solve.node_count()
        + parsed_program
            .functions
            .iter()
            .map(|f| {
                f.body.node_count()
                    + contract_nodes(&f.requires)
                    + contract_nodes(&f.ensures)
                    + contract_nodes(&f.invariant)
            })
            .sum::<usize>()
        + parsed_program
            .async_functions
            .iter()
            .map(|f| {
                f.body.node_count()
                    + contract_nodes(&f.requires)
                    + contract_nodes(&f.ensures)
                    + contract_nodes(&f.invariant)
            })
            .sum::<usize>();
    let max_ast_nodes = language::limits::max_ast_nodes();
    if total_nodes > max_ast_nodes {
        return Err(CompilerError::new(
            CompileErrorKind::Budget,
            format!(
                "AST too large: max_ast_nodes={} got {} (set X07_MAX_AST_NODES=<n>)",
                max_ast_nodes, total_nodes
            ),
        ));
    }

    let (c_src, native_requires) =
        c_emit::emit_c_program_with_native_requires(&parsed_program, options)?;

    let max_c_bytes = language::limits::max_c_bytes();
    if c_src.len() > max_c_bytes {
        return Err(CompilerError::new(
            CompileErrorKind::Budget,
            format!(
                "C source too large: max_c_bytes={} got {} (set X07_MAX_C_BYTES=<bytes> or pass --max-c-bytes <bytes>)",
                max_c_bytes,
                c_src.len()
            ),
        ));
    }

    Ok(CompileToCOutput {
        c_src,
        stats: CompileStats { fuel_used },
        native_requires: NativeRequires {
            schema_version: NATIVE_REQUIRES_SCHEMA_VERSION.to_string(),
            world: Some(options.world.as_str().to_string()),
            requires: native_requires,
        },
        mono_map: Some(mono_map),
    })
}

pub fn check_program(program: &[u8], options: &CompileOptions) -> Result<(), CompilerError> {
    let _ = compile_frontend(program, options)?;
    Ok(())
}

#[derive(Debug, Clone)]
struct FrontendOutput {
    parsed_program: Program,
    mono_map: crate::generics::MonoMapV1,
    module_infos: BTreeMap<String, ModuleInfo>,
    fuel_used: u64,
}

fn compile_frontend(
    program: &[u8],
    options: &CompileOptions,
) -> Result<FrontendOutput, CompilerError> {
    if options.freestanding {
        if options.emit_main {
            return Err(CompilerError::new(
                CompileErrorKind::Unsupported,
                "freestanding profile cannot emit a C main()".to_string(),
            ));
        }
        if options.world != x07_worlds::WorldId::SolvePure
            || options.enable_fs
            || options.enable_rr
            || options.enable_kv
        {
            return Err(CompilerError::new(
                CompileErrorKind::Unsupported,
                "freestanding profile currently supports only --world solve-pure".to_string(),
            ));
        }
    }

    if program.len() > language::limits::MAX_SOURCE_BYTES {
        return Err(CompilerError::new(
            CompileErrorKind::Budget,
            format!(
                "program too large: max_source_bytes={} got {}",
                language::limits::MAX_SOURCE_BYTES,
                program.len()
            ),
        ));
    }

    let src = std::str::from_utf8(program).map_err(|e| {
        CompilerError::new(
            CompileErrorKind::Parse,
            format!("program must be UTF-8: {e}"),
        )
    })?;

    let mut fuel_used: u64 = 0;

    if !src.trim_start().starts_with('{') {
        return Err(CompilerError::new(
            CompileErrorKind::Parse,
            "program must be x07AST JSON (*.x07.json); legacy S-expr is not supported".to_string(),
        ));
    }

    let file = x07ast::parse_x07ast_json(program)
        .map_err(|e| CompilerError::new(CompileErrorKind::Parse, format!("main: {e}")))?;
    enforce_contract_typecheck("main", &file)?;
    fuel_used = fuel_used.saturating_add(x07ast_node_count(&file));
    let main = parse_main_file_x07ast(file)?;
    forbid_internal_only_heads_in_entry("main", &main)?;
    let mut modules = BTreeMap::new();
    let mut module_infos = BTreeMap::new();
    module_infos.insert(
        "main".to_string(),
        ModuleInfo {
            imports: main.imports.clone(),
            exports: BTreeSet::new(),
            is_builtin: false,
        },
    );
    let mut visiting = BTreeSet::new();
    for module_id in &main.imports {
        load_module_recursive(
            module_id,
            options,
            &mut modules,
            &mut module_infos,
            &mut visiting,
            &mut fuel_used,
        )?;
    }

    inject_implicit_imports_for_ty_intrinsics(
        &main,
        options,
        &mut module_infos,
        &mut modules,
        &mut visiting,
        &mut fuel_used,
    )?;

    let ParsedMain {
        schema_version: main_schema_version,
        imports: _main_imports,
        functions: main_functions,
        async_functions: main_async_functions,
        extern_functions: main_extern_functions,
        solve: main_solve,
        meta: main_meta,
    } = main;

    let mut module_metas: BTreeMap<String, BTreeMap<String, Value>> = BTreeMap::new();
    module_metas.insert("main".to_string(), main_meta);

    let mut generic_program = generics::GenericProgram {
        functions: main_functions,
        async_functions: main_async_functions,
        extern_functions: main_extern_functions,
        solve: main_solve,
    };
    for m in modules.values() {
        generic_program.functions.extend(m.functions.clone());
        generic_program
            .async_functions
            .extend(m.async_functions.clone());
        generic_program
            .extern_functions
            .extend(m.extern_functions.clone());
        module_metas.insert(m.module_id.clone(), m.meta.clone());
    }

    let module_exports: BTreeMap<String, BTreeSet<String>> = module_infos
        .iter()
        .map(|(mid, info)| (mid.clone(), info.exports.clone()))
        .collect();

    let (mut parsed_program, mono_map) =
        generics::monomorphize(generic_program, &module_exports, &main_schema_version)?;
    for item in &mono_map.items {
        let Some(info) = module_infos.get_mut(&item.def_module) else {
            return Err(CompilerError::new(
                CompileErrorKind::Internal,
                format!(
                    "internal error: mono map item references unknown module: {:?}",
                    item.def_module
                ),
            ));
        };
        if info.exports.contains(&item.generic) {
            info.exports.insert(item.specialized.clone());
        }
    }

    forbid_reserved_helper_function_names(&parsed_program)?;
    stream_pipe::elaborate_stream_pipes(&mut parsed_program, options, &module_metas)?;
    parsed_program.functions.sort_by(|a, b| a.name.cmp(&b.name));
    parsed_program
        .async_functions
        .sort_by(|a, b| a.name.cmp(&b.name));
    parsed_program
        .extern_functions
        .sort_by(|a, b| a.name.cmp(&b.name));

    dead_code_eliminate(&mut parsed_program);
    validate_program_world_caps(&parsed_program, options)?;
    c_emit::check_c_program(&parsed_program, options)?;

    Ok(FrontendOutput {
        parsed_program,
        mono_map,
        module_infos,
        fuel_used,
    })
}

fn forbid_reserved_helper_function_names(program: &Program) -> Result<(), CompilerError> {
    const RESERVED: &str = ".__std_stream_pipe_v1_";

    for f in &program.functions {
        if f.name.contains(RESERVED) {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                format!("reserved function name: {:?}", f.name),
            ));
        }
    }
    for f in &program.async_functions {
        if f.name.contains(RESERVED) {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                format!("reserved function name: {:?}", f.name),
            ));
        }
    }
    Ok(())
}

#[derive(Debug, Clone)]
struct ModuleInfo {
    imports: BTreeSet<String>,
    exports: BTreeSet<String>,
    is_builtin: bool,
}

fn inject_implicit_imports_for_ty_intrinsics(
    main: &ParsedMain,
    options: &CompileOptions,
    module_infos: &mut BTreeMap<String, ModuleInfo>,
    modules: &mut BTreeMap<String, ParsedModule>,
    visiting: &mut BTreeSet<String>,
    fuel_used: &mut u64,
) -> Result<(), CompilerError> {
    let imports_by_module: BTreeMap<String, BTreeSet<&'static str>> = {
        let mut imports_by_module: BTreeMap<String, BTreeSet<&'static str>> = BTreeMap::new();

        let mut main_imports: BTreeSet<&'static str> = BTreeSet::new();
        collect_ty_intrinsic_imports_in_expr(&main.solve, &mut main_imports);
        for f in &main.functions {
            collect_ty_intrinsic_imports_in_fn(f, &mut main_imports);
        }
        for f in &main.async_functions {
            collect_ty_intrinsic_imports_in_async_fn(f, &mut main_imports);
        }
        imports_by_module.insert("main".to_string(), main_imports);

        for (module_id, m) in modules.iter() {
            let mut req: BTreeSet<&'static str> = BTreeSet::new();
            for f in &m.functions {
                collect_ty_intrinsic_imports_in_fn(f, &mut req);
            }
            for f in &m.async_functions {
                collect_ty_intrinsic_imports_in_async_fn(f, &mut req);
            }
            imports_by_module.insert(module_id.clone(), req);
        }

        imports_by_module
    };

    let mut to_load: BTreeSet<String> = BTreeSet::new();
    for (module_id, req) in imports_by_module {
        if req.is_empty() {
            continue;
        }
        let Some(info) = module_infos.get_mut(&module_id) else {
            return Err(CompilerError::new(
                CompileErrorKind::Internal,
                format!(
                    "internal error: ty intrinsic scan references unknown module: {module_id:?}"
                ),
            ));
        };
        for imp in req {
            let imp = imp.to_string();
            if info.imports.insert(imp.clone()) {
                to_load.insert(imp);
            }
        }
    }

    for module_id in to_load {
        load_module_recursive(
            &module_id,
            options,
            modules,
            module_infos,
            visiting,
            fuel_used,
        )?;
    }

    Ok(())
}

fn collect_ty_intrinsic_imports_in_fn(
    f: &x07ast::AstFunctionDef,
    out: &mut BTreeSet<&'static str>,
) {
    collect_ty_intrinsic_imports_in_expr(&f.body, out);
    for c in f
        .requires
        .iter()
        .chain(f.ensures.iter())
        .chain(f.invariant.iter())
    {
        collect_ty_intrinsic_imports_in_expr(&c.expr, out);
        for w in &c.witness {
            collect_ty_intrinsic_imports_in_expr(w, out);
        }
    }
}

fn collect_ty_intrinsic_imports_in_async_fn(
    f: &x07ast::AstAsyncFunctionDef,
    out: &mut BTreeSet<&'static str>,
) {
    collect_ty_intrinsic_imports_in_expr(&f.body, out);
    for c in f
        .requires
        .iter()
        .chain(f.ensures.iter())
        .chain(f.invariant.iter())
    {
        collect_ty_intrinsic_imports_in_expr(&c.expr, out);
        for w in &c.witness {
            collect_ty_intrinsic_imports_in_expr(w, out);
        }
    }
}

fn collect_ty_intrinsic_imports_in_expr(expr: &crate::ast::Expr, out: &mut BTreeSet<&'static str>) {
    match expr {
        crate::ast::Expr::Int { .. } | crate::ast::Expr::Ident { .. } => {}
        crate::ast::Expr::List { items, .. } => {
            if let Some(head) = items.first().and_then(crate::ast::Expr::as_ident) {
                match head {
                    "ty.read_le_at" | "ty.write_le_at" | "ty.push_le" => {
                        out.insert("std.u32");
                    }
                    "ty.hash32" => {
                        out.insert("std.hash");
                    }
                    _ => {}
                }
            }
            for item in items {
                collect_ty_intrinsic_imports_in_expr(item, out);
            }
        }
    }
}

#[derive(Debug, Clone)]
struct ParsedModule {
    module_id: String,
    functions: Vec<x07ast::AstFunctionDef>,
    async_functions: Vec<x07ast::AstAsyncFunctionDef>,
    extern_functions: Vec<x07ast::AstExternFunctionDecl>,
    meta: BTreeMap<String, Value>,
}

#[derive(Debug, Clone)]
struct ParsedMain {
    schema_version: String,
    imports: BTreeSet<String>,
    functions: Vec<x07ast::AstFunctionDef>,
    async_functions: Vec<x07ast::AstAsyncFunctionDef>,
    extern_functions: Vec<x07ast::AstExternFunctionDecl>,
    solve: crate::ast::Expr,
    meta: BTreeMap<String, Value>,
}

const INTERNAL_ONLY_HEADS: &[&str] = &[
    "set_u32.dump_u32le",
    "map_u32.dump_kv_u32le_u32le",
    "task.scope.slot_to_i32_v1",
    "task.scope.slot_from_i32_v1",
    "__internal.brand.assume_view_v1",
    "__internal.brand.view_to_bytes_preserve_brand_v1",
    "__internal.result_bytes.unwrap_ok_v1",
    "__internal.bytes.alloc_aligned_v1",
    "__internal.stream_xf.plugin_init_v1",
    "__internal.stream_xf.plugin_step_v1",
    "__internal.stream_xf.plugin_flush_v1",
];

fn find_internal_only_head(expr: &crate::ast::Expr) -> Option<&'static str> {
    match expr {
        crate::ast::Expr::Int { .. } | crate::ast::Expr::Ident { .. } => None,
        crate::ast::Expr::List { items, .. } => {
            let head = items.first().and_then(crate::ast::Expr::as_ident);
            if let Some(head) = head {
                for &forbidden in INTERNAL_ONLY_HEADS {
                    if head == forbidden {
                        return Some(forbidden);
                    }
                }
            }
            for item in items {
                if let Some(hit) = find_internal_only_head(item) {
                    return Some(hit);
                }
            }
            None
        }
    }
}

fn forbid_internal_only_heads_in_entry(
    label: &str,
    main: &ParsedMain,
) -> Result<(), CompilerError> {
    if let Some(head) = find_internal_only_head(&main.solve) {
        return Err(CompilerError::new(
            CompileErrorKind::Unsupported,
            format!("{label}: internal-only builtin is not allowed here: {head}"),
        ));
    }
    for f in &main.functions {
        if let Some(head) = find_internal_only_head(&f.body) {
            return Err(CompilerError::new(
                CompileErrorKind::Unsupported,
                format!("{label}: internal-only builtin is not allowed here: {head}"),
            ));
        }
        for c in f
            .requires
            .iter()
            .chain(f.ensures.iter())
            .chain(f.invariant.iter())
        {
            if let Some(head) = find_internal_only_head(&c.expr) {
                return Err(CompilerError::new(
                    CompileErrorKind::Unsupported,
                    format!("{label}: internal-only builtin is not allowed here: {head}"),
                ));
            }
            for w in &c.witness {
                if let Some(head) = find_internal_only_head(w) {
                    return Err(CompilerError::new(
                        CompileErrorKind::Unsupported,
                        format!("{label}: internal-only builtin is not allowed here: {head}"),
                    ));
                }
            }
        }
    }
    for f in &main.async_functions {
        if let Some(head) = find_internal_only_head(&f.body) {
            return Err(CompilerError::new(
                CompileErrorKind::Unsupported,
                format!("{label}: internal-only builtin is not allowed here: {head}"),
            ));
        }
        for c in f
            .requires
            .iter()
            .chain(f.ensures.iter())
            .chain(f.invariant.iter())
        {
            if let Some(head) = find_internal_only_head(&c.expr) {
                return Err(CompilerError::new(
                    CompileErrorKind::Unsupported,
                    format!("{label}: internal-only builtin is not allowed here: {head}"),
                ));
            }
            for w in &c.witness {
                if let Some(head) = find_internal_only_head(w) {
                    return Err(CompilerError::new(
                        CompileErrorKind::Unsupported,
                        format!("{label}: internal-only builtin is not allowed here: {head}"),
                    ));
                }
            }
        }
    }
    Ok(())
}

fn forbid_internal_only_heads_in_module(
    module_id: &str,
    module: &ParsedModule,
) -> Result<(), CompilerError> {
    for f in &module.functions {
        if let Some(head) = find_internal_only_head(&f.body) {
            return Err(CompilerError::new(
                CompileErrorKind::Unsupported,
                format!("{module_id:?}: internal-only builtin is not allowed here: {head}"),
            ));
        }
        for c in f
            .requires
            .iter()
            .chain(f.ensures.iter())
            .chain(f.invariant.iter())
        {
            if let Some(head) = find_internal_only_head(&c.expr) {
                return Err(CompilerError::new(
                    CompileErrorKind::Unsupported,
                    format!("{module_id:?}: internal-only builtin is not allowed here: {head}"),
                ));
            }
            for w in &c.witness {
                if let Some(head) = find_internal_only_head(w) {
                    return Err(CompilerError::new(
                        CompileErrorKind::Unsupported,
                        format!("{module_id:?}: internal-only builtin is not allowed here: {head}"),
                    ));
                }
            }
        }
    }
    for f in &module.async_functions {
        if let Some(head) = find_internal_only_head(&f.body) {
            return Err(CompilerError::new(
                CompileErrorKind::Unsupported,
                format!("{module_id:?}: internal-only builtin is not allowed here: {head}"),
            ));
        }
        for c in f
            .requires
            .iter()
            .chain(f.ensures.iter())
            .chain(f.invariant.iter())
        {
            if let Some(head) = find_internal_only_head(&c.expr) {
                return Err(CompilerError::new(
                    CompileErrorKind::Unsupported,
                    format!("{module_id:?}: internal-only builtin is not allowed here: {head}"),
                ));
            }
            for w in &c.witness {
                if let Some(head) = find_internal_only_head(w) {
                    return Err(CompilerError::new(
                        CompileErrorKind::Unsupported,
                        format!("{module_id:?}: internal-only builtin is not allowed here: {head}"),
                    ));
                }
            }
        }
    }
    Ok(())
}

fn forbid_internal_only_heads_in_non_builtin_code(
    program: &Program,
    module_infos: &BTreeMap<String, ModuleInfo>,
) -> Result<(), CompilerError> {
    if let Some(head) = find_internal_only_head(&program.solve) {
        return Err(CompilerError::new(
            CompileErrorKind::Unsupported,
            format!("main: internal-only builtin is not allowed here: {head}"),
        ));
    }

    for f in &program.functions {
        let Some((module_id, _name)) = f.name.rsplit_once('.') else {
            return Err(CompilerError::new(
                CompileErrorKind::Internal,
                format!(
                    "internal error: function name missing module prefix: {:?}",
                    f.name
                ),
            ));
        };
        let info = module_infos.get(module_id).ok_or_else(|| {
            CompilerError::new(
                CompileErrorKind::Internal,
                format!(
                    "internal error: function {:?} belongs to unknown module {:?}",
                    f.name, module_id
                ),
            )
        })?;
        if info.is_builtin {
            continue;
        }
        if f.name.contains(".__std_stream_pipe_v1_") {
            continue;
        }
        if let Some(head) = find_internal_only_head(&f.body) {
            return Err(CompilerError::new(
                CompileErrorKind::Unsupported,
                format!(
                    "{:?}: internal-only builtin is not allowed here: {head}",
                    f.name
                ),
            ));
        }
        for c in f
            .requires
            .iter()
            .chain(f.ensures.iter())
            .chain(f.invariant.iter())
        {
            if let Some(head) = find_internal_only_head(&c.expr) {
                return Err(CompilerError::new(
                    CompileErrorKind::Unsupported,
                    format!(
                        "{:?}: internal-only builtin is not allowed here: {head}",
                        f.name
                    ),
                ));
            }
            for w in &c.witness {
                if let Some(head) = find_internal_only_head(w) {
                    return Err(CompilerError::new(
                        CompileErrorKind::Unsupported,
                        format!(
                            "{:?}: internal-only builtin is not allowed here: {head}",
                            f.name
                        ),
                    ));
                }
            }
        }
    }

    for f in &program.async_functions {
        let Some((module_id, _name)) = f.name.rsplit_once('.') else {
            return Err(CompilerError::new(
                CompileErrorKind::Internal,
                format!(
                    "internal error: async function name missing module prefix: {:?}",
                    f.name
                ),
            ));
        };
        let info = module_infos.get(module_id).ok_or_else(|| {
            CompilerError::new(
                CompileErrorKind::Internal,
                format!(
                    "internal error: async function {:?} belongs to unknown module {:?}",
                    f.name, module_id
                ),
            )
        })?;
        if info.is_builtin {
            continue;
        }
        if f.name.contains(".__std_stream_pipe_v1_") {
            continue;
        }
        if let Some(head) = find_internal_only_head(&f.body) {
            return Err(CompilerError::new(
                CompileErrorKind::Unsupported,
                format!(
                    "{:?}: internal-only builtin is not allowed here: {head}",
                    f.name
                ),
            ));
        }
        for c in f
            .requires
            .iter()
            .chain(f.ensures.iter())
            .chain(f.invariant.iter())
        {
            if let Some(head) = find_internal_only_head(&c.expr) {
                return Err(CompilerError::new(
                    CompileErrorKind::Unsupported,
                    format!(
                        "{:?}: internal-only builtin is not allowed here: {head}",
                        f.name
                    ),
                ));
            }
            for w in &c.witness {
                if let Some(head) = find_internal_only_head(w) {
                    return Err(CompilerError::new(
                        CompileErrorKind::Unsupported,
                        format!(
                            "{:?}: internal-only builtin is not allowed here: {head}",
                            f.name
                        ),
                    ));
                }
            }
        }
    }

    Ok(())
}

fn parse_main_file_x07ast(file: x07ast::X07AstFile) -> Result<ParsedMain, CompilerError> {
    if file.kind != x07ast::X07AstKind::Entry {
        return Err(CompilerError::new(
            CompileErrorKind::Parse,
            format!("main: expected kind=\"entry\" got {:?}", file.kind),
        ));
    }
    if file.module_id != "main" {
        return Err(CompilerError::new(
            CompileErrorKind::Parse,
            format!(
                "main: entry module_id must be \"main\" got {:?}",
                file.module_id
            ),
        ));
    }
    let solve = file.solve.ok_or_else(|| {
        CompilerError::new(
            CompileErrorKind::Parse,
            "main: missing solve expression".to_string(),
        )
    })?;

    Ok(ParsedMain {
        schema_version: file.schema_version,
        imports: file.imports,
        functions: file.functions,
        async_functions: file.async_functions,
        extern_functions: file.extern_functions,
        solve,
        meta: file.meta,
    })
}

fn parse_module_file_x07ast(
    module_id: &str,
    file: x07ast::X07AstFile,
) -> Result<(ParsedModule, ModuleInfo), CompilerError> {
    if file.kind != x07ast::X07AstKind::Module {
        return Err(CompilerError::new(
            CompileErrorKind::Parse,
            format!(
                "module {module_id:?}: expected kind=\"module\" got {:?}",
                file.kind
            ),
        ));
    }
    if file.module_id != module_id {
        return Err(CompilerError::new(
            CompileErrorKind::Parse,
            format!(
                "module {module_id:?}: module_id mismatch in file: got {:?}",
                file.module_id
            ),
        ));
    }
    if file.exports.is_empty() {
        return Err(CompilerError::new(
            CompileErrorKind::Parse,
            format!("module {module_id:?} is missing an export declaration"),
        ));
    }

    let mut defined: BTreeSet<String> = file.functions.iter().map(|f| f.name.clone()).collect();
    defined.extend(file.async_functions.iter().map(|f| f.name.clone()));
    defined.extend(file.extern_functions.iter().map(|f| f.name.clone()));
    for e in &file.exports {
        if !e.starts_with(&format!("{module_id}.")) {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                format!("export {e:?} does not belong to module {module_id:?}"),
            ));
        }
        if !defined.contains(e) {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                format!("export {e:?} is not defined in module {module_id:?}"),
            ));
        }
    }

    let functions = file.functions;
    let async_functions = file.async_functions;
    let extern_functions = file.extern_functions;

    Ok((
        ParsedModule {
            module_id: module_id.to_string(),
            functions,
            async_functions,
            extern_functions,
            meta: file.meta,
        },
        ModuleInfo {
            imports: file.imports,
            exports: file.exports,
            is_builtin: false,
        },
    ))
}

fn load_module_recursive(
    module_id: &str,
    options: &CompileOptions,
    modules: &mut BTreeMap<String, ParsedModule>,
    module_infos: &mut BTreeMap<String, ModuleInfo>,
    visiting: &mut BTreeSet<String>,
    fuel_used: &mut u64,
) -> Result<(), CompilerError> {
    if module_infos.contains_key(module_id) {
        return Ok(());
    }
    if !visiting.insert(module_id.to_string()) {
        return Err(CompilerError::new(
            CompileErrorKind::Parse,
            format!("cyclic import detected at module {module_id:?}"),
        ));
    }

    let source =
        module_source::load_module_source(module_id, options.world, &options.module_roots)?;
    let src = source.src;
    let is_builtin = source.is_builtin;

    if !src.trim_start().starts_with('{') {
        return Err(CompilerError::new(
            CompileErrorKind::Parse,
            format!(
                "{module_id:?}: module source must be x07AST JSON (*.x07.json); legacy S-expr is not supported"
            ),
        ));
    }

    let file = x07ast::parse_x07ast_json(src.as_bytes())
        .map_err(|e| CompilerError::new(CompileErrorKind::Parse, format!("{module_id:?}: {e}")))?;
    enforce_contract_typecheck(module_id, &file)?;
    *fuel_used = fuel_used.saturating_add(x07ast_node_count(&file));
    let (m, mut info) = parse_module_file_x07ast(module_id, file)?;
    info.is_builtin = is_builtin;

    if !is_builtin {
        forbid_internal_only_heads_in_module(module_id, &m)?;
    }

    for dep in &info.imports {
        load_module_recursive(dep, options, modules, module_infos, visiting, fuel_used)?;
    }

    modules.insert(module_id.to_string(), m);
    module_infos.insert(module_id.to_string(), info);
    let _ = visiting.remove(module_id);
    Ok(())
}

fn x07ast_node_count(file: &x07ast::X07AstFile) -> u64 {
    let mut n: u64 = 0;
    for f in &file.functions {
        n = n.saturating_add(f.body.node_count() as u64);
        for c in f
            .requires
            .iter()
            .chain(f.ensures.iter())
            .chain(f.invariant.iter())
        {
            n = n.saturating_add(c.expr.node_count() as u64);
            for w in &c.witness {
                n = n.saturating_add(w.node_count() as u64);
            }
        }
    }
    for f in &file.async_functions {
        n = n.saturating_add(f.body.node_count() as u64);
        for c in f
            .requires
            .iter()
            .chain(f.ensures.iter())
            .chain(f.invariant.iter())
        {
            n = n.saturating_add(c.expr.node_count() as u64);
            for w in &c.witness {
                n = n.saturating_add(w.node_count() as u64);
            }
        }
    }
    if let Some(solve) = &file.solve {
        n = n.saturating_add(solve.node_count() as u64);
    }
    n
}

fn file_has_contracts(file: &x07ast::X07AstFile) -> bool {
    file.functions
        .iter()
        .any(|f| !f.requires.is_empty() || !f.ensures.is_empty() || !f.invariant.is_empty())
        || file
            .async_functions
            .iter()
            .any(|f| !f.requires.is_empty() || !f.ensures.is_empty() || !f.invariant.is_empty())
}

fn enforce_contract_typecheck(label: &str, file: &x07ast::X07AstFile) -> Result<(), CompilerError> {
    if file.schema_version != X07AST_SCHEMA_VERSION_V0_5_0 {
        return Ok(());
    }
    if !file_has_contracts(file) {
        return Ok(());
    }

    let tc = crate::typecheck::typecheck_file_local(file, &Default::default());
    let errors = tc
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .collect::<Vec<_>>();
    if errors.is_empty() {
        return Ok(());
    }

    let mut summary: Vec<String> = Vec::new();
    for d in errors.iter().take(8) {
        let ptr = d
            .loc
            .as_ref()
            .and_then(|loc| match loc {
                Location::X07Ast { ptr } => Some(ptr.as_str()),
                _ => None,
            })
            .unwrap_or("");
        if ptr.is_empty() {
            summary.push(format!("{}: {}", d.code, d.message));
        } else {
            summary.push(format!("{}: {} (ptr={})", d.code, d.message, ptr));
        }
    }
    if errors.len() > 8 {
        summary.push(format!("... and {} more", errors.len() - 8));
    }

    Err(CompilerError::new(
        CompileErrorKind::Typing,
        format!("{label}: typecheck failed: {}", summary.join("; ")),
    ))
}

fn validate_program_visibility(
    program: &Program,
    module_infos: &BTreeMap<String, ModuleInfo>,
) -> Result<(), CompilerError> {
    let mut fn_module: BTreeMap<String, String> = BTreeMap::new();
    for f in &program.functions {
        let Some((m, _name)) = f.name.rsplit_once('.') else {
            return Err(CompilerError::new(
                CompileErrorKind::Internal,
                format!(
                    "internal error: function name missing module prefix: {:?}",
                    f.name
                ),
            ));
        };
        if !module_infos.contains_key(m) {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                format!("function {:?} belongs to unknown module {:?}", f.name, m),
            ));
        }
        if fn_module.insert(f.name.clone(), m.to_string()).is_some() {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                format!("duplicate function name: {:?}", f.name),
            ));
        }
    }
    for f in &program.async_functions {
        let Some((m, _name)) = f.name.rsplit_once('.') else {
            return Err(CompilerError::new(
                CompileErrorKind::Internal,
                format!(
                    "internal error: async function name missing module prefix: {:?}",
                    f.name
                ),
            ));
        };
        if !module_infos.contains_key(m) {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                format!(
                    "async function {:?} belongs to unknown module {:?}",
                    f.name, m
                ),
            ));
        }
        if fn_module.insert(f.name.clone(), m.to_string()).is_some() {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                format!("duplicate function name: {:?}", f.name),
            ));
        }
    }
    for f in &program.extern_functions {
        let Some((m, _name)) = f.name.rsplit_once('.') else {
            return Err(CompilerError::new(
                CompileErrorKind::Internal,
                format!(
                    "internal error: extern function name missing module prefix: {:?}",
                    f.name
                ),
            ));
        };
        if !module_infos.contains_key(m) {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                format!(
                    "extern function {:?} belongs to unknown module {:?}",
                    f.name, m
                ),
            ));
        }
        if fn_module.insert(f.name.clone(), m.to_string()).is_some() {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                format!("duplicate function name: {:?}", f.name),
            ));
        }
    }

    validate_expr_visibility(&program.solve, "main", &fn_module, module_infos)?;

    for f in &program.functions {
        let caller_mod = fn_module.get(&f.name).map(|s| s.as_str()).unwrap_or("main");
        validate_expr_visibility(&f.body, caller_mod, &fn_module, module_infos)?;
    }

    for f in &program.async_functions {
        let caller_mod = fn_module.get(&f.name).map(|s| s.as_str()).unwrap_or("main");
        validate_expr_visibility(&f.body, caller_mod, &fn_module, module_infos)?;
    }

    Ok(())
}

fn validate_program_world_caps(
    program: &Program,
    options: &CompileOptions,
) -> Result<(), CompilerError> {
    fn expr_uses_head(expr: &crate::ast::Expr, head: &str) -> bool {
        match expr {
            crate::ast::Expr::Int { .. } | crate::ast::Expr::Ident { .. } => false,
            crate::ast::Expr::List { items, .. } => {
                if let Some(crate::ast::Expr::Ident { name: h, .. }) = items.first() {
                    if h == head {
                        return true;
                    }
                }
                items.iter().any(|e| expr_uses_head(e, head))
            }
        }
    }

    fn program_uses_head(program: &Program, head: &str) -> bool {
        if expr_uses_head(&program.solve, head) {
            return true;
        }
        for f in &program.functions {
            if expr_uses_head(&f.body, head) {
                return true;
            }
            for c in f
                .requires
                .iter()
                .chain(f.ensures.iter())
                .chain(f.invariant.iter())
            {
                if expr_uses_head(&c.expr, head) {
                    return true;
                }
                for w in &c.witness {
                    if expr_uses_head(w, head) {
                        return true;
                    }
                }
            }
        }
        for f in &program.async_functions {
            if expr_uses_head(&f.body, head) {
                return true;
            }
            for c in f
                .requires
                .iter()
                .chain(f.ensures.iter())
                .chain(f.invariant.iter())
            {
                if expr_uses_head(&c.expr, head) {
                    return true;
                }
                for w in &c.witness {
                    if expr_uses_head(w, head) {
                        return true;
                    }
                }
            }
        }
        false
    }

    if !options.enable_fs {
        for head in ["fs.read", "fs.read_async", "fs.open_read", "fs.list_dir"] {
            if program_uses_head(program, head) {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("{head} is disabled in this world"),
                ));
            }
        }
    }

    if !options.enable_rr {
        for head in [
            "rr.current_v1",
            "rr.open_v1",
            "rr.close_v1",
            "rr.stats_v1",
            "rr.next_v1",
            "rr.append_v1",
            "rr.entry_resp_v1",
            "rr.entry_err_v1",
            "std.rr.with_v1",
            "std.rr.with_policy_v1",
        ] {
            if program_uses_head(program, head) {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("{head} is disabled in this world"),
                ));
            }
        }
    }

    if !options.enable_kv {
        for head in ["kv.get", "kv.get_async", "kv.get_stream", "kv.set"] {
            if program_uses_head(program, head) {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("{head} is disabled in this world"),
                ));
            }
        }
    }

    if !options.allow_ffi() {
        if let Some(f) = program.extern_functions.first() {
            return Err(CompilerError::new(
                CompileErrorKind::Unsupported,
                format!(
                    "extern decl {:?} requires ffi capability; {}",
                    f.name,
                    options.hint_enable_ffi()
                ),
            ));
        }
    }

    if !options.allow_unsafe() {
        if let Some((owner, ty)) = first_raw_pointer_signature_type(program) {
            return Err(CompilerError::new(
                CompileErrorKind::Unsupported,
                format!(
                    "raw pointer type {ty:?} in signature of {owner:?} requires unsafe capability; {}",
                    options.hint_enable_unsafe(),
                ),
            ));
        }
    }

    Ok(())
}

fn dead_code_eliminate(program: &mut Program) {
    fn collect_call_heads_contracts(
        clauses: &[crate::x07ast::ContractClauseAst],
        out: &mut Vec<String>,
    ) {
        for c in clauses {
            collect_call_heads(&c.expr, out);
            for w in &c.witness {
                collect_call_heads(w, out);
            }
        }
    }

    let fn_index: BTreeMap<String, usize> = program
        .functions
        .iter()
        .enumerate()
        .map(|(idx, f)| (f.name.clone(), idx))
        .collect();
    let async_index: BTreeMap<String, usize> = program
        .async_functions
        .iter()
        .enumerate()
        .map(|(idx, f)| (f.name.clone(), idx))
        .collect();
    let extern_names: BTreeSet<String> = program
        .extern_functions
        .iter()
        .map(|f| f.name.clone())
        .collect();

    let mut reachable_fns: BTreeSet<String> = BTreeSet::new();
    let mut reachable_async: BTreeSet<String> = BTreeSet::new();
    let mut reachable_extern: BTreeSet<String> = BTreeSet::new();

    let mut worklist: Vec<String> = Vec::new();
    collect_call_heads(&program.solve, &mut worklist);

    while let Some(name) = worklist.pop() {
        if let Some(&idx) = fn_index.get(&name) {
            if reachable_fns.insert(name.clone()) {
                let f = &program.functions[idx];
                collect_call_heads(&f.body, &mut worklist);
                collect_call_heads_contracts(&f.requires, &mut worklist);
                collect_call_heads_contracts(&f.ensures, &mut worklist);
                collect_call_heads_contracts(&f.invariant, &mut worklist);
            }
            continue;
        }
        if let Some(&idx) = async_index.get(&name) {
            if reachable_async.insert(name.clone()) {
                let f = &program.async_functions[idx];
                collect_call_heads(&f.body, &mut worklist);
                collect_call_heads_contracts(&f.requires, &mut worklist);
                collect_call_heads_contracts(&f.ensures, &mut worklist);
                collect_call_heads_contracts(&f.invariant, &mut worklist);
            }
            continue;
        }
        if extern_names.contains(&name) {
            reachable_extern.insert(name);
        }
    }

    program
        .functions
        .retain(|f| reachable_fns.contains(&f.name));
    program
        .async_functions
        .retain(|f| reachable_async.contains(&f.name));
    program
        .extern_functions
        .retain(|f| reachable_extern.contains(&f.name));
}

fn collect_call_heads(expr: &crate::ast::Expr, out: &mut Vec<String>) {
    match expr {
        crate::ast::Expr::Int { .. } | crate::ast::Expr::Ident { .. } => {}
        crate::ast::Expr::List { items, .. } => {
            if let Some(head) = items.first().and_then(crate::ast::Expr::as_ident) {
                out.push(head.to_string());
            }
            for e in items {
                collect_call_heads(e, out);
            }
        }
    }
}

fn first_raw_pointer_signature_type(program: &Program) -> Option<(String, Ty)> {
    for f in &program.functions {
        if let Some(ty) = f
            .params
            .iter()
            .map(|p| p.ty)
            .chain(std::iter::once(f.ret_ty))
            .find(|&ty| ty.is_ptr_ty())
        {
            return Some((f.name.clone(), ty));
        }
    }
    for f in &program.async_functions {
        if let Some(ty) = f
            .params
            .iter()
            .map(|p| p.ty)
            .chain(std::iter::once(f.ret_ty))
            .find(|&ty| ty.is_ptr_ty())
        {
            return Some((f.name.clone(), ty));
        }
    }
    for f in &program.extern_functions {
        if let Some(ty) = f
            .params
            .iter()
            .map(|p| p.ty)
            .chain(std::iter::once(f.ret_ty))
            .find(|&ty| ty.is_ptr_ty())
        {
            return Some((f.name.clone(), ty));
        }
    }
    None
}

fn validate_expr_visibility(
    expr: &crate::ast::Expr,
    caller_module: &str,
    fn_module: &BTreeMap<String, String>,
    module_infos: &BTreeMap<String, ModuleInfo>,
) -> Result<(), CompilerError> {
    match expr {
        crate::ast::Expr::Int { .. } | crate::ast::Expr::Ident { .. } => Ok(()),
        crate::ast::Expr::List { items, .. } => {
            if let Some(head) = items.first().and_then(crate::ast::Expr::as_ident) {
                if let Some(callee_module) = fn_module.get(head) {
                    if callee_module != caller_module {
                        let caller_info = module_infos.get(caller_module).ok_or_else(|| {
                            CompilerError::new(
                                CompileErrorKind::Parse,
                                format!("unknown module: {caller_module:?}"),
                            )
                        })?;
                        if !caller_info.imports.contains(callee_module) {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                format!(
                                    "call to {head:?} requires (import {callee_module}) in module {caller_module:?}"
                                ),
                            ));
                        }
                        let callee_info = module_infos.get(callee_module).ok_or_else(|| {
                            CompilerError::new(
                                CompileErrorKind::Parse,
                                format!("unknown module: {callee_module:?}"),
                            )
                        })?;
                        if !callee_info.exports.contains(head) {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                format!(
                                    "function {head:?} is not exported by module {callee_module:?}"
                                ),
                            ));
                        }
                    }
                }
            }
            for item in items {
                validate_expr_visibility(item, caller_module, fn_module, module_infos)?;
            }
            Ok(())
        }
    }
}
