use std::collections::{BTreeMap, BTreeSet};

use crate::builtin_modules;
use crate::c_emit;
use crate::guide;
use crate::language;
use crate::native::NativeRequires;
use crate::optimize;
use crate::program::{AsyncFunctionDef, FunctionDef, Program};
use crate::types::Ty;
use crate::validate;
use crate::x07ast;
use x07_contracts::NATIVE_REQUIRES_SCHEMA_VERSION;

#[derive(Debug, Clone)]
pub struct CompileOptions {
    pub world: x07_worlds::WorldId,
    pub enable_fs: bool,
    pub enable_rr: bool,
    pub enable_kv: bool,
    pub module_roots: Vec<std::path::PathBuf>,
    pub emit_main: bool,
    pub freestanding: bool,
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
            emit_main: true,
            freestanding: false,
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
}

pub fn compile_program_to_c_with_meta(
    program: &[u8],
    options: &CompileOptions,
) -> Result<CompileToCOutput, CompilerError> {
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
    fuel_used = fuel_used.saturating_add(x07ast_node_count(&file));
    let main = parse_main_file_x07ast(file)?;
    forbid_internal_only_heads_in_program("main", &main.program)?;
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

    let mut parsed_program = Program {
        functions: main.program.functions,
        async_functions: main.program.async_functions,
        extern_functions: main.program.extern_functions,
        solve: main.program.solve,
    };
    for m in modules.values() {
        parsed_program.functions.extend(m.functions.clone());
        parsed_program
            .async_functions
            .extend(m.async_functions.clone());
        parsed_program
            .extern_functions
            .extend(m.extern_functions.clone());
    }
    parsed_program.functions.sort_by(|a, b| a.name.cmp(&b.name));
    parsed_program
        .async_functions
        .sort_by(|a, b| a.name.cmp(&b.name));
    parsed_program
        .extern_functions
        .sort_by(|a, b| a.name.cmp(&b.name));

    validate_program_world_caps(&parsed_program, options)?;
    c_emit::check_c_program(&parsed_program, options)?;
    dead_code_eliminate(&mut parsed_program);

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

    let total_nodes: usize = parsed_program.solve.node_count()
        + parsed_program
            .functions
            .iter()
            .map(|f| f.body.node_count())
            .sum::<usize>()
        + parsed_program
            .async_functions
            .iter()
            .map(|f| f.body.node_count())
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
    })
}

#[derive(Debug, Clone)]
struct ModuleInfo {
    imports: BTreeSet<String>,
    exports: BTreeSet<String>,
    is_builtin: bool,
}

#[derive(Debug, Clone)]
struct ParsedModule {
    functions: Vec<FunctionDef>,
    async_functions: Vec<AsyncFunctionDef>,
    extern_functions: Vec<crate::program::ExternFunctionDecl>,
}

#[derive(Debug, Clone)]
struct ParsedMain {
    imports: BTreeSet<String>,
    program: Program,
}

const INTERNAL_ONLY_HEADS: &[&str] = &["set_u32.dump_u32le", "map_u32.dump_kv_u32le_u32le"];

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

fn forbid_internal_only_heads_in_program(
    label: &str,
    program: &Program,
) -> Result<(), CompilerError> {
    if let Some(head) = find_internal_only_head(&program.solve) {
        return Err(CompilerError::new(
            CompileErrorKind::Unsupported,
            format!("{label}: internal-only builtin is not allowed here: {head}"),
        ));
    }
    for f in &program.functions {
        if let Some(head) = find_internal_only_head(&f.body) {
            return Err(CompilerError::new(
                CompileErrorKind::Unsupported,
                format!("{label}: internal-only builtin is not allowed here: {head}"),
            ));
        }
    }
    for f in &program.async_functions {
        if let Some(head) = find_internal_only_head(&f.body) {
            return Err(CompilerError::new(
                CompileErrorKind::Unsupported,
                format!("{label}: internal-only builtin is not allowed here: {head}"),
            ));
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
    }
    for f in &module.async_functions {
        if let Some(head) = find_internal_only_head(&f.body) {
            return Err(CompilerError::new(
                CompileErrorKind::Unsupported,
                format!("{module_id:?}: internal-only builtin is not allowed here: {head}"),
            ));
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
        if let Some(head) = find_internal_only_head(&f.body) {
            return Err(CompilerError::new(
                CompileErrorKind::Unsupported,
                format!(
                    "{:?}: internal-only builtin is not allowed here: {head}",
                    f.name
                ),
            ));
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
        if let Some(head) = find_internal_only_head(&f.body) {
            return Err(CompilerError::new(
                CompileErrorKind::Unsupported,
                format!(
                    "{:?}: internal-only builtin is not allowed here: {head}",
                    f.name
                ),
            ));
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
        imports: file.imports,
        program: Program {
            functions: file.functions,
            async_functions: file.async_functions,
            extern_functions: file.extern_functions,
            solve,
        },
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

    Ok((
        ParsedModule {
            functions: file.functions,
            async_functions: file.async_functions,
            extern_functions: file.extern_functions,
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

    let (src, is_builtin) =
        if options.world.is_standalone_only() && module_id.starts_with("std.world.") {
            (
                read_module_from_roots(module_id, &options.module_roots)?,
                false,
            )
        } else if let Some(src) = builtin_modules::builtin_module_source(module_id) {
            (src.to_string(), true)
        } else {
            (
                read_module_from_roots(module_id, &options.module_roots)?,
                false,
            )
        };

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

fn read_module_from_roots(
    module_id: &str,
    module_roots: &[std::path::PathBuf],
) -> Result<String, CompilerError> {
    if module_roots.is_empty() {
        return Err(CompilerError::new(
            CompileErrorKind::Parse,
            format!("unknown module: {module_id:?}"),
        ));
    }

    validate::validate_module_id(module_id)
        .map_err(|message| CompilerError::new(CompileErrorKind::Parse, message))?;

    let mut rel_path_base = std::path::PathBuf::new();
    for seg in module_id.split('.') {
        rel_path_base.push(seg);
    }

    let mut json_rel = rel_path_base.clone();
    json_rel.set_extension("x07.json");
    let json_rel_display = json_rel.display().to_string();

    let mut json_hits: Vec<std::path::PathBuf> = Vec::new();
    for root in module_roots {
        let path = root.join(&json_rel);
        if path.exists() {
            json_hits.push(path);
        }
    }
    if !json_hits.is_empty() {
        return match json_hits.len() {
            1 => {
                let path = &json_hits[0];
                std::fs::read_to_string(path).map_err(|e| {
                    CompilerError::new(
                        CompileErrorKind::Parse,
                        format!("read module {module_id:?} at {}: {e}", path.display()),
                    )
                })
            }
            _ => Err(CompilerError::new(
                CompileErrorKind::Parse,
                format!("module {module_id:?} is ambiguous across roots: {json_hits:?}"),
            )),
        };
    }
    Err(CompilerError::new(
        CompileErrorKind::Parse,
        format!("unknown module: {module_id:?} (searched: {json_rel_display})"),
    ))
}

fn x07ast_node_count(file: &x07ast::X07AstFile) -> u64 {
    let mut n: u64 = 0;
    for f in &file.functions {
        n = n.saturating_add(f.body.node_count() as u64);
    }
    for f in &file.async_functions {
        n = n.saturating_add(f.body.node_count() as u64);
    }
    if let Some(solve) = &file.solve {
        n = n.saturating_add(solve.node_count() as u64);
    }
    n
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
        }
        for f in &program.async_functions {
            if expr_uses_head(&f.body, head) {
                return true;
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
        for head in ["rr.send_request", "rr.fetch", "rr.send"] {
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
                collect_call_heads(&program.functions[idx].body, &mut worklist);
            }
            continue;
        }
        if let Some(&idx) = async_index.get(&name) {
            if reachable_async.insert(name.clone()) {
                collect_call_heads(&program.async_functions[idx].body, &mut worklist);
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
