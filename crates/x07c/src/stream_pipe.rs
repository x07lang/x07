use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use crate::ast::Expr;
use crate::compile::{CompileErrorKind, CompileOptions, CompilerError};
use crate::program::{FunctionDef, FunctionParam, Program};
use crate::types::Ty;
use crate::x07ast;

pub fn elaborate_stream_pipes(
    program: &mut Program,
    options: &CompileOptions,
) -> Result<(), CompilerError> {
    let mut existing_names: BTreeSet<String> = BTreeSet::new();
    for f in &program.functions {
        existing_names.insert(f.name.clone());
    }
    for f in &program.async_functions {
        existing_names.insert(f.name.clone());
    }
    for f in &program.extern_functions {
        existing_names.insert(f.name.clone());
    }

    let mut elab = Elaborator {
        options,
        existing_names,
        helpers: BTreeMap::new(),
        new_helpers: Vec::new(),
    };

    program.solve = elab.rewrite_expr(program.solve.clone(), "main")?;
    for f in &mut program.functions {
        let module_id = function_module_id(&f.name)?;
        f.body = elab.rewrite_expr(f.body.clone(), module_id)?;
    }
    for f in &mut program.async_functions {
        let module_id = function_module_id(&f.name)?;
        f.body = elab.rewrite_expr(f.body.clone(), module_id)?;
    }

    program.functions.extend(elab.new_helpers);
    Ok(())
}

struct Elaborator<'a> {
    options: &'a CompileOptions,
    existing_names: BTreeSet<String>,
    helpers: BTreeMap<(String, String), String>,
    new_helpers: Vec<FunctionDef>,
}

impl Elaborator<'_> {
    fn rewrite_expr(&mut self, expr: Expr, module_id: &str) -> Result<Expr, CompilerError> {
        match expr {
            Expr::Int { .. } | Expr::Ident { .. } => Ok(expr),
            Expr::List { items, ptr } => {
                if items.first().and_then(Expr::as_ident) == Some("std.stream.pipe_v1") {
                    return self.rewrite_pipe(Expr::List { items, ptr }, module_id);
                }

                let mut new_items = Vec::with_capacity(items.len());
                for item in items {
                    new_items.push(self.rewrite_expr(item, module_id)?);
                }
                Ok(Expr::List {
                    items: new_items,
                    ptr,
                })
            }
        }
    }

    fn rewrite_pipe(&mut self, expr: Expr, module_id: &str) -> Result<Expr, CompilerError> {
        let h8 = hash_pipe_without_expr_bodies(&expr)?;
        let parsed = parse_pipe_v1(&expr)?;

        let helper_full = format!("{module_id}.__std_stream_pipe_v1_{h8}");
        let helper_key = (module_id.to_string(), h8.clone());

        let helper_name = if let Some(name) = self.helpers.get(&helper_key) {
            name.clone()
        } else {
            if self.existing_names.contains(&helper_full) {
                return Err(CompilerError::new(
                    CompileErrorKind::Parse,
                    format!("pipe helper name collision: {helper_full:?}"),
                ));
            }
            validate_pipe_world_caps(&parsed, self.options)?;
            let body = gen_pipe_helper_body(&parsed, self.options)?;
            let helper = FunctionDef {
                name: helper_full.clone(),
                params: parsed
                    .params
                    .iter()
                    .enumerate()
                    .map(|(idx, p)| FunctionParam {
                        name: format!("p{idx}"),
                        ty: p.ty,
                    })
                    .collect(),
                ret_ty: Ty::Bytes,
                body,
            };
            self.existing_names.insert(helper_full.clone());
            self.helpers.insert(helper_key, helper_full.clone());
            self.new_helpers.push(helper);
            helper_full
        };

        let mut begin_items: Vec<Expr> = vec![expr_ident("begin")];
        let mut arg_names: Vec<String> = Vec::with_capacity(parsed.params.len());
        for (idx, param) in parsed.params.iter().enumerate() {
            let arg_name = format!("__std_stream_pipe_v1_{h8}_arg{idx}");
            let arg_expr = self.rewrite_expr(param.expr.clone(), module_id)?;
            begin_items.push(expr_list(vec![
                expr_ident("let"),
                expr_ident(arg_name.clone()),
                arg_expr,
            ]));
            arg_names.push(arg_name);
        }

        let mut call_items: Vec<Expr> = Vec::with_capacity(1 + arg_names.len());
        call_items.push(expr_ident(helper_name));
        for name in arg_names {
            call_items.push(expr_ident(name));
        }
        begin_items.push(expr_list(call_items));
        Ok(expr_list(begin_items))
    }
}

#[derive(Debug, Clone)]
struct PipeParam {
    ty: Ty,
    expr: Expr,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct PipeParsed {
    cfg: PipeCfgV1,
    src: PipeSrcV1,
    chain: Vec<PipeXfV1>,
    sink: PipeSinkV1,
    params: Vec<PipeParam>,
}

fn parse_pipe_v1(expr: &Expr) -> Result<PipeParsed, CompilerError> {
    let Expr::List { items, .. } = expr else {
        return Err(CompilerError::new(
            CompileErrorKind::Typing,
            "std.stream.pipe_v1 must be a list".to_string(),
        ));
    };
    if items.len() != 5 {
        return Err(CompilerError::new(
            CompileErrorKind::Typing,
            "std.stream.pipe_v1 expects 4 arguments".to_string(),
        ));
    }

    let cfg = &items[1];
    let src = &items[2];
    let chain = &items[3];
    let sink = &items[4];

    let mut params: Vec<PipeParam> = Vec::new();

    let cfg = parse_cfg_v1(cfg)?;
    let src = parse_src_v1(src, &mut params)?;
    let mut chain = parse_chain_v1(chain, &mut params)?;

    // Desugaring: src.net_tcp_read_u32frames_v1 := src.net_tcp_read_stream_handle_v1 + xf.deframe_u32le_v1
    let (src, chain) = match src {
        PipeSrcV1::NetTcpReadU32Frames {
            stream_handle_param,
            caps_param,
            max_frame_bytes,
            allow_empty,
            on_timeout,
            on_eof,
        } => {
            chain.insert(
                0,
                PipeXfV1::DeframeU32LeV1 {
                    cfg: DeframeU32LeCfgV1 {
                        max_frame_bytes,
                        max_frames: 0,
                        allow_empty,
                        on_truncated: DeframeOnTruncatedV1::Err,
                    },
                },
            );
            (
                PipeSrcV1::NetTcpReadStreamHandle {
                    stream_handle_param,
                    caps_param,
                    on_timeout,
                    on_eof,
                },
                chain,
            )
        }
        src => (src, chain),
    };

    let sink = parse_sink_v1(sink, &mut params)?;

    Ok(PipeParsed {
        cfg,
        src,
        chain,
        sink,
        params,
    })
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct PipeCfgV1 {
    chunk_max_bytes: i32,
    bufread_cap_bytes: i32,
    max_in_bytes: i32,
    max_out_bytes: i32,
    max_items: i32,
    max_steps: Option<i32>,
    emit_payload: Option<i32>,
    emit_stats: Option<i32>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
enum PipeSrcV1 {
    FsOpenRead {
        path_param: usize,
    },
    RrSend {
        key_param: usize,
    },
    Bytes {
        bytes_param: usize,
    },
    DbRowsDoc {
        conn_param: usize,
        sql_param: usize,
        params_doc_param: usize,
        qcaps_doc_param: usize,
    },
    NetTcpReadStreamHandle {
        stream_handle_param: usize,
        caps_param: usize,
        on_timeout: NetOnTimeoutV1,
        on_eof: NetOnEofV1,
    },
    NetTcpReadU32Frames {
        stream_handle_param: usize,
        caps_param: usize,
        max_frame_bytes: i32,
        allow_empty: i32,
        on_timeout: NetOnTimeoutV1,
        on_eof: NetOnEofV1,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NetOnTimeoutV1 {
    Err,
    Stop,
    StopIfClean,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NetOnEofV1 {
    LeaveOpen,
    ShutdownRead,
    Close,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
enum PipeXfV1 {
    MapBytes {
        fn_id: String,
    },
    Filter {
        fn_id: String,
    },
    Take {
        n_param: usize,
    },
    SplitLines {
        delim_param: usize,
        max_line_bytes_param: usize,
    },
    FrameU32Le,
    MapInPlaceBufV1 {
        scratch_cap_bytes: i32,
        clear_before_each: i32,
        fn_id: String,
    },
    JsonCanonStreamV1 {
        cfg: JsonCanonStreamCfgV1,
    },
    DeframeU32LeV1 {
        cfg: DeframeU32LeCfgV1,
    },
}

#[derive(Debug, Clone, Copy)]
enum DeframeOnTruncatedV1 {
    Err,
    Drop,
}

#[derive(Debug, Clone, Copy)]
struct JsonCanonStreamCfgV1 {
    max_depth: i32,
    max_total_json_bytes: i32,
    max_object_members: i32,
    max_object_total_bytes: i32,
    emit_chunk_max_bytes: i32,
}

#[derive(Debug, Clone)]
struct DeframeU32LeCfgV1 {
    max_frame_bytes: i32,
    max_frames: i32,
    allow_empty: i32,
    on_truncated: DeframeOnTruncatedV1,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
enum PipeSinkV1 {
    CollectBytes,
    HashFnv1a32,
    Null,
    WorldFsWriteFile {
        path_param: usize,
    },
    U32Frames {
        inner: Box<PipeSinkV1>,
    },
    WorldFsWriteStream {
        path_param: usize,
        caps_param: usize,
        cfg: WorldFsWriteStreamCfgV1,
    },
    WorldFsWriteStreamHashFnv1a32 {
        path_param: usize,
        caps_param: usize,
        cfg: WorldFsWriteStreamCfgV1,
    },
    NetTcpWriteStreamHandle {
        stream_handle_param: usize,
        caps_param: usize,
        cfg: NetTcpWriteStreamHandleCfgV1,
    },
    NetTcpConnectWrite {
        addr_param: usize,
        caps_param: usize,
        cfg: NetTcpWriteStreamHandleCfgV1,
    },
}

#[derive(Debug, Clone, Copy)]
struct WorldFsWriteStreamCfgV1 {
    buf_cap_bytes: i32,
    flush_min_bytes: i32,
    max_flushes: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NetSinkOnFinishV1 {
    LeaveOpen,
    ShutdownWrite,
    Close,
}

#[derive(Debug, Clone, Copy)]
struct NetTcpWriteStreamHandleCfgV1 {
    buf_cap_bytes: i32,
    flush_min_bytes: i32,
    max_flushes: i32,
    max_write_calls: i32,
    on_finish: NetSinkOnFinishV1,
}

fn parse_cfg_v1(expr: &Expr) -> Result<PipeCfgV1, CompilerError> {
    let Expr::List { items, .. } = expr else {
        return Err(CompilerError::new(
            CompileErrorKind::Typing,
            "std.stream.cfg_v1 must be a list".to_string(),
        ));
    };
    if items.first().and_then(Expr::as_ident) != Some("std.stream.cfg_v1") {
        return Err(CompilerError::new(
            CompileErrorKind::Typing,
            "pipe cfg must be std.stream.cfg_v1".to_string(),
        ));
    }

    let mut chunk_max_bytes = None;
    let mut bufread_cap_bytes = None;
    let mut max_in_bytes = None;
    let mut max_out_bytes = None;
    let mut max_items = None;

    let mut max_steps = None;
    let mut emit_payload = None;
    let mut emit_stats = None;

    for field in items.iter().skip(1) {
        let Expr::List { items: kv, .. } = field else {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "cfg field must be a pair".to_string(),
            ));
        };
        if kv.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "cfg field must be a pair".to_string(),
            ));
        }
        let Some(key) = kv[0].as_ident() else {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "cfg key must be an identifier".to_string(),
            ));
        };
        let value = expect_i32(&kv[1], "cfg value must be an integer")?;
        match key {
            "chunk_max_bytes" => chunk_max_bytes = Some(value),
            "bufread_cap_bytes" => bufread_cap_bytes = Some(value),
            "max_in_bytes" => max_in_bytes = Some(value),
            "max_out_bytes" => max_out_bytes = Some(value),
            "max_items" => max_items = Some(value),
            "max_steps" => max_steps = Some(value),
            "emit_payload" => emit_payload = Some(value),
            "emit_stats" => emit_stats = Some(value),
            _ => {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("unknown cfg field: {key}"),
                ));
            }
        }
    }

    Ok(PipeCfgV1 {
        chunk_max_bytes: chunk_max_bytes.ok_or_else(|| {
            CompilerError::new(
                CompileErrorKind::Typing,
                "cfg missing chunk_max_bytes".to_string(),
            )
        })?,
        bufread_cap_bytes: bufread_cap_bytes.ok_or_else(|| {
            CompilerError::new(
                CompileErrorKind::Typing,
                "cfg missing bufread_cap_bytes".to_string(),
            )
        })?,
        max_in_bytes: max_in_bytes.ok_or_else(|| {
            CompilerError::new(
                CompileErrorKind::Typing,
                "cfg missing max_in_bytes".to_string(),
            )
        })?,
        max_out_bytes: max_out_bytes.ok_or_else(|| {
            CompilerError::new(
                CompileErrorKind::Typing,
                "cfg missing max_out_bytes".to_string(),
            )
        })?,
        max_items: max_items.ok_or_else(|| {
            CompilerError::new(
                CompileErrorKind::Typing,
                "cfg missing max_items".to_string(),
            )
        })?,
        max_steps,
        emit_payload,
        emit_stats,
    })
}

fn parse_src_v1(expr: &Expr, params: &mut Vec<PipeParam>) -> Result<PipeSrcV1, CompilerError> {
    let Expr::List { items, .. } = expr else {
        return Err(CompilerError::new(
            CompileErrorKind::Typing,
            "pipe src must be a list".to_string(),
        ));
    };
    let Some(head) = items.first().and_then(Expr::as_ident) else {
        return Err(CompilerError::new(
            CompileErrorKind::Typing,
            "pipe src must start with an identifier".to_string(),
        ));
    };

    match head {
        "std.stream.src.fs_open_read_v1" => {
            if items.len() != 2 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("{head} expects 1 argument"),
                ));
            }
            let path_param = parse_expr_v1(params, Ty::BytesView, &items[1])?;
            Ok(PipeSrcV1::FsOpenRead { path_param })
        }
        "std.stream.src.rr_send_v1" => {
            if items.len() != 2 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("{head} expects 1 argument"),
                ));
            }
            let key_param = parse_expr_v1(params, Ty::BytesView, &items[1])?;
            Ok(PipeSrcV1::RrSend { key_param })
        }
        "std.stream.src.bytes_v1" => {
            if items.len() != 2 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("{head} expects 1 argument"),
                ));
            }
            let bytes_param = parse_expr_v1(params, Ty::Bytes, &items[1])?;
            Ok(PipeSrcV1::Bytes { bytes_param })
        }
        "std.stream.src.db_rows_doc_v1" => {
            if items.len() != 5 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("{head} expects 4 arguments"),
                ));
            }
            let conn_param = parse_expr_v1(params, Ty::I32, &items[1])?;
            let sql_param = parse_expr_v1(params, Ty::BytesView, &items[2])?;
            let params_doc_param = parse_expr_v1(params, Ty::Bytes, &items[3])?;
            let qcaps_doc_param = parse_expr_v1(params, Ty::Bytes, &items[4])?;
            Ok(PipeSrcV1::DbRowsDoc {
                conn_param,
                sql_param,
                params_doc_param,
                qcaps_doc_param,
            })
        }
        "std.stream.src.net_tcp_read_stream_handle_v1" => {
            let fields = parse_kv_fields(head, &items[1..])?;
            let stream_handle = fields.get("stream_handle").ok_or_else(|| {
                CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("{head} missing stream_handle"),
                )
            })?;
            let caps = fields.get("caps").ok_or_else(|| {
                CompilerError::new(CompileErrorKind::Typing, format!("{head} missing caps"))
            })?;

            // Canonical evaluation order (spec): stream_handle then caps.
            let stream_handle_param = parse_expr_v1(params, Ty::I32, stream_handle)?;
            let caps_param = parse_expr_v1(params, Ty::Bytes, caps)?;

            let on_timeout = match fields.get("on_timeout") {
                None => NetOnTimeoutV1::Err,
                Some(v) => match v.as_ident() {
                    Some("err") => NetOnTimeoutV1::Err,
                    Some("stop") => NetOnTimeoutV1::Stop,
                    _ => {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            format!("{head} on_timeout must be \"err\" or \"stop\""),
                        ));
                    }
                },
            };

            let on_eof = match fields.get("on_eof") {
                None => NetOnEofV1::LeaveOpen,
                Some(v) => match v.as_ident() {
                    Some("leave_open") => NetOnEofV1::LeaveOpen,
                    Some("shutdown_read") => NetOnEofV1::ShutdownRead,
                    Some("close") => NetOnEofV1::Close,
                    _ => {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            format!("{head} on_eof must be \"leave_open\", \"shutdown_read\", or \"close\""),
                        ));
                    }
                },
            };
            Ok(PipeSrcV1::NetTcpReadStreamHandle {
                stream_handle_param,
                caps_param,
                on_timeout,
                on_eof,
            })
        }
        "std.stream.src.net_tcp_read_u32frames_v1" => {
            let fields = parse_kv_fields(head, &items[1..])?;
            let stream_handle = fields.get("stream_handle").ok_or_else(|| {
                CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("{head} missing stream_handle"),
                )
            })?;
            let caps = fields.get("caps").ok_or_else(|| {
                CompilerError::new(CompileErrorKind::Typing, format!("{head} missing caps"))
            })?;
            let max_frame_bytes = fields.get("max_frame_bytes").ok_or_else(|| {
                CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("{head} missing max_frame_bytes"),
                )
            })?;

            // Canonical evaluation order: stream_handle then caps.
            let stream_handle_param = parse_expr_v1(params, Ty::I32, stream_handle)?;
            let caps_param = parse_expr_v1(params, Ty::Bytes, caps)?;

            let max_frame_bytes =
                expect_i32(max_frame_bytes, "max_frame_bytes must be an integer")?;
            let allow_empty = fields
                .get("allow_empty")
                .map(|v| expect_i32(v, "allow_empty must be an integer"))
                .transpose()?
                .unwrap_or(1);

            let on_timeout = match fields.get("on_timeout") {
                None => NetOnTimeoutV1::Err,
                Some(v) => match v.as_ident() {
                    Some("err") => NetOnTimeoutV1::Err,
                    Some("stop_if_clean") => NetOnTimeoutV1::StopIfClean,
                    _ => {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            format!("{head} on_timeout must be \"err\" or \"stop_if_clean\""),
                        ));
                    }
                },
            };

            let on_eof = match fields.get("on_eof") {
                None => NetOnEofV1::LeaveOpen,
                Some(v) => match v.as_ident() {
                    Some("leave_open") => NetOnEofV1::LeaveOpen,
                    Some("shutdown_read") => NetOnEofV1::ShutdownRead,
                    Some("close") => NetOnEofV1::Close,
                    _ => {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            format!("{head} on_eof must be \"leave_open\", \"shutdown_read\", or \"close\""),
                        ));
                    }
                },
            };

            Ok(PipeSrcV1::NetTcpReadU32Frames {
                stream_handle_param,
                caps_param,
                max_frame_bytes,
                allow_empty,
                on_timeout,
                on_eof,
            })
        }
        _ => Err(CompilerError::new(
            CompileErrorKind::Typing,
            format!("unsupported pipe src: {head}"),
        )),
    }
}

fn parse_chain_v1(
    expr: &Expr,
    params: &mut Vec<PipeParam>,
) -> Result<Vec<PipeXfV1>, CompilerError> {
    let Expr::List { items, .. } = expr else {
        return Err(CompilerError::new(
            CompileErrorKind::Typing,
            "pipe chain must be a list".to_string(),
        ));
    };
    if items.first().and_then(Expr::as_ident) != Some("std.stream.chain_v1") {
        return Err(CompilerError::new(
            CompileErrorKind::Typing,
            "pipe chain must be std.stream.chain_v1".to_string(),
        ));
    }
    let mut xfs = Vec::with_capacity(items.len().saturating_sub(1));
    for xf in items.iter().skip(1) {
        xfs.push(parse_xf_v1(xf, params)?);
    }
    Ok(xfs)
}

fn parse_xf_v1(expr: &Expr, params: &mut Vec<PipeParam>) -> Result<PipeXfV1, CompilerError> {
    let Expr::List { items, .. } = expr else {
        return Err(CompilerError::new(
            CompileErrorKind::Typing,
            "pipe xf must be a list".to_string(),
        ));
    };
    let Some(head) = items.first().and_then(Expr::as_ident) else {
        return Err(CompilerError::new(
            CompileErrorKind::Typing,
            "pipe xf must start with an identifier".to_string(),
        ));
    };
    match head {
        "std.stream.xf.map_bytes_v1" => {
            if items.len() != 2 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("{head} expects 1 argument"),
                ));
            }
            let fn_id = parse_fn_v1(&items[1])?;
            Ok(PipeXfV1::MapBytes { fn_id })
        }
        "std.stream.xf.filter_v1" => {
            if items.len() != 2 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("{head} expects 1 argument"),
                ));
            }
            let fn_id = parse_fn_v1(&items[1])?;
            Ok(PipeXfV1::Filter { fn_id })
        }
        "std.stream.xf.take_v1" => {
            if items.len() != 2 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("{head} expects 1 argument"),
                ));
            }
            let n_param = parse_expr_v1(params, Ty::I32, &items[1])?;
            Ok(PipeXfV1::Take { n_param })
        }
        "std.stream.xf.split_lines_v1" => {
            if items.len() != 3 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("{head} expects 2 arguments"),
                ));
            }
            let delim_param = parse_expr_v1(params, Ty::I32, &items[1])?;
            let max_line_bytes_param = parse_expr_v1(params, Ty::I32, &items[2])?;
            Ok(PipeXfV1::SplitLines {
                delim_param,
                max_line_bytes_param,
            })
        }
        "std.stream.xf.frame_u32le_v1" => {
            if items.len() != 1 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("{head} expects 0 arguments"),
                ));
            }
            Ok(PipeXfV1::FrameU32Le)
        }
        "std.stream.xf.map_in_place_buf_v1" => {
            // v1.1; no expr_v1 params.
            let mut scratch_cap_bytes: Option<i32> = None;
            let mut clear_before_each: Option<i32> = None;
            let mut fn_id: Option<String> = None;

            for item in items.iter().skip(1) {
                let Expr::List { items: inner, .. } = item else {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        format!("{head} items must be lists"),
                    ));
                };

                if inner.first().and_then(Expr::as_ident) == Some("std.stream.fn_v1") {
                    let f = parse_fn_v1(item)?;
                    if fn_id.replace(f).is_some() {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            format!("{head} has duplicate fn"),
                        ));
                    }
                    continue;
                }

                if inner.len() != 2 {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        format!("{head} fields must be pairs"),
                    ));
                }
                let Some(key) = inner[0].as_ident() else {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        format!("{head} field key must be an identifier"),
                    ));
                };
                match key {
                    "scratch_cap_bytes" => {
                        let x = expect_i32(&inner[1], "scratch_cap_bytes must be an integer")?;
                        if x <= 0 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} scratch_cap_bytes must be >= 1"),
                            ));
                        }
                        if scratch_cap_bytes.replace(x).is_some() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} has duplicate scratch_cap_bytes"),
                            ));
                        }
                    }
                    "clear_before_each" => {
                        let x = expect_i32(&inner[1], "clear_before_each must be an integer")?;
                        if clear_before_each.replace(x).is_some() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} has duplicate clear_before_each"),
                            ));
                        }
                    }
                    "fn" => {
                        let f = parse_fn_v1(&inner[1])?;
                        if fn_id.replace(f).is_some() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} has duplicate fn"),
                            ));
                        }
                    }
                    _ => {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            format!("{head} unknown field: {key}"),
                        ));
                    }
                }
            }

            let scratch_cap_bytes = scratch_cap_bytes.ok_or_else(|| {
                CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("{head} missing scratch_cap_bytes"),
                )
            })?;
            let fn_id = fn_id.ok_or_else(|| {
                CompilerError::new(CompileErrorKind::Typing, format!("{head} missing fn"))
            })?;

            Ok(PipeXfV1::MapInPlaceBufV1 {
                scratch_cap_bytes,
                clear_before_each: clear_before_each.unwrap_or(1),
                fn_id,
            })
        }
        "std.stream.xf.json_canon_stream_v1" => {
            // v1.1; no expr_v1 params.
            let fields = parse_kv_fields(head, &items[1..])?;
            for k in fields.keys() {
                match k.as_str() {
                    "max_depth"
                    | "max_total_json_bytes"
                    | "max_object_members"
                    | "max_object_total_bytes"
                    | "emit_chunk_max_bytes" => {}
                    _ => {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            format!("{head} unknown field: {k}"),
                        ));
                    }
                }
            }

            fn parse_opt_pos_i32(
                fields: &BTreeMap<String, Expr>,
                key: &str,
                msg: &str,
                head: &str,
            ) -> Result<i32, CompilerError> {
                match fields.get(key) {
                    None => Ok(0),
                    Some(v) => {
                        let x = expect_i32(v, msg)?;
                        if x <= 0 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} {key} must be >= 1"),
                            ));
                        }
                        Ok(x)
                    }
                }
            }

            let max_depth =
                parse_opt_pos_i32(&fields, "max_depth", "max_depth must be an integer", head)?;
            let max_total_json_bytes = parse_opt_pos_i32(
                &fields,
                "max_total_json_bytes",
                "max_total_json_bytes must be an integer",
                head,
            )?;
            let max_object_members = parse_opt_pos_i32(
                &fields,
                "max_object_members",
                "max_object_members must be an integer",
                head,
            )?;
            let max_object_total_bytes = parse_opt_pos_i32(
                &fields,
                "max_object_total_bytes",
                "max_object_total_bytes must be an integer",
                head,
            )?;
            let emit_chunk_max_bytes = parse_opt_pos_i32(
                &fields,
                "emit_chunk_max_bytes",
                "emit_chunk_max_bytes must be an integer",
                head,
            )?;

            Ok(PipeXfV1::JsonCanonStreamV1 {
                cfg: JsonCanonStreamCfgV1 {
                    max_depth,
                    max_total_json_bytes,
                    max_object_members,
                    max_object_total_bytes,
                    emit_chunk_max_bytes,
                },
            })
        }
        "std.stream.xf.deframe_u32le_v1" => {
            // v1.1 read-side; no expr_v1 params.
            let fields = parse_kv_fields(head, &items[1..])?;

            let max_frame_bytes = fields
                .get("max_frame_bytes")
                .ok_or_else(|| {
                    CompilerError::new(
                        CompileErrorKind::Typing,
                        format!("{head} missing max_frame_bytes"),
                    )
                })
                .and_then(|v| expect_i32(v, "max_frame_bytes must be an integer"))?;

            let max_frames = fields
                .get("max_frames")
                .map(|v| expect_i32(v, "max_frames must be an integer"))
                .transpose()?
                .unwrap_or(0);

            let allow_empty = fields
                .get("allow_empty")
                .map(|v| expect_i32(v, "allow_empty must be an integer"))
                .transpose()?
                .unwrap_or(1);

            let on_truncated = match fields.get("on_truncated") {
                None => DeframeOnTruncatedV1::Err,
                Some(v) => match v.as_ident() {
                    Some("err") => DeframeOnTruncatedV1::Err,
                    Some("drop") => DeframeOnTruncatedV1::Drop,
                    _ => {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            format!("{head} on_truncated must be \"err\" or \"drop\""),
                        ));
                    }
                },
            };

            Ok(PipeXfV1::DeframeU32LeV1 {
                cfg: DeframeU32LeCfgV1 {
                    max_frame_bytes,
                    max_frames,
                    allow_empty,
                    on_truncated,
                },
            })
        }
        _ => Err(CompilerError::new(
            CompileErrorKind::Typing,
            format!("unsupported pipe xf: {head}"),
        )),
    }
}

fn parse_sink_v1(expr: &Expr, params: &mut Vec<PipeParam>) -> Result<PipeSinkV1, CompilerError> {
    let Expr::List { items, .. } = expr else {
        return Err(CompilerError::new(
            CompileErrorKind::Typing,
            "pipe sink must be a list".to_string(),
        ));
    };
    let Some(head) = items.first().and_then(Expr::as_ident) else {
        return Err(CompilerError::new(
            CompileErrorKind::Typing,
            "pipe sink must start with an identifier".to_string(),
        ));
    };

    match head {
        "std.stream.sink.collect_bytes_v1" => {
            if items.len() != 1 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("{head} expects 0 arguments"),
                ));
            }
            Ok(PipeSinkV1::CollectBytes)
        }
        "std.stream.sink.hash_fnv1a32_v1" => {
            if items.len() != 1 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("{head} expects 0 arguments"),
                ));
            }
            Ok(PipeSinkV1::HashFnv1a32)
        }
        "std.stream.sink.null_v1" => {
            if items.len() != 1 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("{head} expects 0 arguments"),
                ));
            }
            Ok(PipeSinkV1::Null)
        }
        "std.stream.sink.world_fs_write_file_v1" => {
            if items.len() != 2 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("{head} expects 1 argument"),
                ));
            }
            let path_param = parse_expr_v1(params, Ty::Bytes, &items[1])?;
            Ok(PipeSinkV1::WorldFsWriteFile { path_param })
        }
        "std.stream.sink.u32frames_v1" => {
            if items.len() != 2 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("{head} expects 1 argument"),
                ));
            }
            let inner = parse_sink_v1(&items[1], params)?;
            Ok(PipeSinkV1::U32Frames {
                inner: Box::new(inner),
            })
        }
        "std.stream.sink.world_fs_write_stream_v1"
        | "std.stream.sink.world_fs_write_stream_hash_fnv1a32_v1" => {
            if items.len() < 3 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("{head} expects at least 2 arguments"),
                ));
            }
            let path_param = parse_expr_v1(params, Ty::Bytes, &items[1])?;
            let caps_param = parse_expr_v1(params, Ty::Bytes, &items[2])?;
            let fields = parse_kv_fields(head, &items[3..])?;
            for k in fields.keys() {
                match k.as_str() {
                    "buf_cap_bytes" | "flush_min_bytes" | "max_flushes" => {}
                    _ => {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            format!("{head} unknown field: {k}"),
                        ));
                    }
                }
            }

            let buf_cap_bytes = match fields.get("buf_cap_bytes") {
                None => 0,
                Some(v) => {
                    let x = expect_i32(v, "buf_cap_bytes must be an integer")?;
                    if x <= 0 {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            format!("{head} buf_cap_bytes must be >= 1"),
                        ));
                    }
                    x
                }
            };

            let flush_min_bytes = match fields.get("flush_min_bytes") {
                None => 0,
                Some(v) => {
                    let x = expect_i32(v, "flush_min_bytes must be an integer")?;
                    if x <= 0 {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            format!("{head} flush_min_bytes must be >= 1"),
                        ));
                    }
                    x
                }
            };

            let max_flushes = match fields.get("max_flushes") {
                None => 0,
                Some(v) => {
                    let x = expect_i32(v, "max_flushes must be an integer")?;
                    if x < 0 {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            format!("{head} max_flushes must be >= 0"),
                        ));
                    }
                    x
                }
            };

            let cfg = WorldFsWriteStreamCfgV1 {
                buf_cap_bytes,
                flush_min_bytes,
                max_flushes,
            };
            if head == "std.stream.sink.world_fs_write_stream_v1" {
                Ok(PipeSinkV1::WorldFsWriteStream {
                    path_param,
                    caps_param,
                    cfg,
                })
            } else {
                Ok(PipeSinkV1::WorldFsWriteStreamHashFnv1a32 {
                    path_param,
                    caps_param,
                    cfg,
                })
            }
        }
        "std.stream.sink.net_tcp_write_stream_handle_v1" => {
            let fields = parse_kv_fields(head, &items[1..])?;
            for k in fields.keys() {
                match k.as_str() {
                    "stream_handle" | "caps" | "buf_cap_bytes" | "flush_min_bytes"
                    | "on_finish" | "max_flushes" | "max_write_calls" => {}
                    _ => {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            format!("{head} unknown field: {k}"),
                        ));
                    }
                }
            }

            let stream_handle = fields.get("stream_handle").ok_or_else(|| {
                CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("{head} missing stream_handle"),
                )
            })?;
            let caps = fields.get("caps").ok_or_else(|| {
                CompilerError::new(CompileErrorKind::Typing, format!("{head} missing caps"))
            })?;

            // Canonical evaluation order (spec): stream_handle then caps.
            let stream_handle_param = parse_expr_v1(params, Ty::I32, stream_handle)?;
            let caps_param = parse_expr_v1(params, Ty::Bytes, caps)?;

            let buf_cap_bytes = fields
                .get("buf_cap_bytes")
                .map(|v| expect_i32(v, "buf_cap_bytes must be an integer"))
                .transpose()?
                .unwrap_or(65536);
            if buf_cap_bytes <= 0 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("{head} buf_cap_bytes must be >= 1"),
                ));
            }

            let flush_min_bytes = fields
                .get("flush_min_bytes")
                .map(|v| expect_i32(v, "flush_min_bytes must be an integer"))
                .transpose()?
                .unwrap_or(buf_cap_bytes);
            if flush_min_bytes <= 0 || flush_min_bytes > buf_cap_bytes {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("{head} flush_min_bytes must be in [1..buf_cap_bytes]"),
                ));
            }

            let on_finish = match fields.get("on_finish") {
                None => NetSinkOnFinishV1::LeaveOpen,
                Some(v) => match v.as_ident() {
                    Some("leave_open") => NetSinkOnFinishV1::LeaveOpen,
                    Some("shutdown_write") => NetSinkOnFinishV1::ShutdownWrite,
                    Some("close") => NetSinkOnFinishV1::Close,
                    _ => {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            format!("{head} on_finish must be \"leave_open\", \"shutdown_write\", or \"close\""),
                        ));
                    }
                },
            };

            let max_flushes = fields
                .get("max_flushes")
                .map(|v| expect_i32(v, "max_flushes must be an integer"))
                .transpose()?
                .unwrap_or(0);
            if max_flushes < 0 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("{head} max_flushes must be >= 0"),
                ));
            }

            let max_write_calls = fields
                .get("max_write_calls")
                .map(|v| expect_i32(v, "max_write_calls must be an integer"))
                .transpose()?
                .unwrap_or(0);
            if max_write_calls < 0 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("{head} max_write_calls must be >= 0"),
                ));
            }

            Ok(PipeSinkV1::NetTcpWriteStreamHandle {
                stream_handle_param,
                caps_param,
                cfg: NetTcpWriteStreamHandleCfgV1 {
                    buf_cap_bytes,
                    flush_min_bytes,
                    max_flushes,
                    max_write_calls,
                    on_finish,
                },
            })
        }
        "std.stream.sink.net_tcp_write_u32frames_v1" => {
            // Convenience wrapper: u32frames(net_tcp_write_stream_handle_v1(...)).
            let fields = parse_kv_fields(head, &items[1..])?;
            for k in fields.keys() {
                match k.as_str() {
                    "stream_handle" | "caps" | "buf_cap_bytes" | "flush_min_bytes"
                    | "on_finish" | "max_flushes" | "max_write_calls" => {}
                    _ => {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            format!("{head} unknown field: {k}"),
                        ));
                    }
                }
            }

            let stream_handle = fields.get("stream_handle").ok_or_else(|| {
                CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("{head} missing stream_handle"),
                )
            })?;
            let caps = fields.get("caps").ok_or_else(|| {
                CompilerError::new(CompileErrorKind::Typing, format!("{head} missing caps"))
            })?;

            // Canonical evaluation order (spec): stream_handle then caps.
            let stream_handle_param = parse_expr_v1(params, Ty::I32, stream_handle)?;
            let caps_param = parse_expr_v1(params, Ty::Bytes, caps)?;

            let buf_cap_bytes = fields
                .get("buf_cap_bytes")
                .map(|v| expect_i32(v, "buf_cap_bytes must be an integer"))
                .transpose()?
                .unwrap_or(65536);
            if buf_cap_bytes <= 0 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("{head} buf_cap_bytes must be >= 1"),
                ));
            }

            let flush_min_bytes = fields
                .get("flush_min_bytes")
                .map(|v| expect_i32(v, "flush_min_bytes must be an integer"))
                .transpose()?
                .unwrap_or(buf_cap_bytes);
            if flush_min_bytes <= 0 || flush_min_bytes > buf_cap_bytes {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("{head} flush_min_bytes must be in [1..buf_cap_bytes]"),
                ));
            }

            let on_finish = match fields.get("on_finish") {
                None => NetSinkOnFinishV1::LeaveOpen,
                Some(v) => match v.as_ident() {
                    Some("leave_open") => NetSinkOnFinishV1::LeaveOpen,
                    Some("shutdown_write") => NetSinkOnFinishV1::ShutdownWrite,
                    Some("close") => NetSinkOnFinishV1::Close,
                    _ => {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            format!("{head} on_finish must be \"leave_open\", \"shutdown_write\", or \"close\""),
                        ));
                    }
                },
            };

            let max_flushes = fields
                .get("max_flushes")
                .map(|v| expect_i32(v, "max_flushes must be an integer"))
                .transpose()?
                .unwrap_or(0);
            if max_flushes < 0 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("{head} max_flushes must be >= 0"),
                ));
            }

            let max_write_calls = fields
                .get("max_write_calls")
                .map(|v| expect_i32(v, "max_write_calls must be an integer"))
                .transpose()?
                .unwrap_or(0);
            if max_write_calls < 0 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("{head} max_write_calls must be >= 0"),
                ));
            }

            Ok(PipeSinkV1::U32Frames {
                inner: Box::new(PipeSinkV1::NetTcpWriteStreamHandle {
                    stream_handle_param,
                    caps_param,
                    cfg: NetTcpWriteStreamHandleCfgV1 {
                        buf_cap_bytes,
                        flush_min_bytes,
                        max_flushes,
                        max_write_calls,
                        on_finish,
                    },
                }),
            })
        }
        "std.stream.sink.net_tcp_connect_write_v1" => {
            let fields = parse_kv_fields(head, &items[1..])?;
            for k in fields.keys() {
                match k.as_str() {
                    "addr" | "caps" | "buf_cap_bytes" | "flush_min_bytes" | "on_finish"
                    | "max_flushes" | "max_write_calls" => {}
                    _ => {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            format!("{head} unknown field: {k}"),
                        ));
                    }
                }
            }

            let addr = fields.get("addr").ok_or_else(|| {
                CompilerError::new(CompileErrorKind::Typing, format!("{head} missing addr"))
            })?;
            let caps = fields.get("caps").ok_or_else(|| {
                CompilerError::new(CompileErrorKind::Typing, format!("{head} missing caps"))
            })?;

            // Canonical evaluation order (spec): addr then caps.
            let addr_param = parse_expr_v1(params, Ty::Bytes, addr)?;
            let caps_param = parse_expr_v1(params, Ty::Bytes, caps)?;

            let buf_cap_bytes = fields
                .get("buf_cap_bytes")
                .map(|v| expect_i32(v, "buf_cap_bytes must be an integer"))
                .transpose()?
                .unwrap_or(65536);
            if buf_cap_bytes <= 0 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("{head} buf_cap_bytes must be >= 1"),
                ));
            }

            let flush_min_bytes = fields
                .get("flush_min_bytes")
                .map(|v| expect_i32(v, "flush_min_bytes must be an integer"))
                .transpose()?
                .unwrap_or(buf_cap_bytes);
            if flush_min_bytes <= 0 || flush_min_bytes > buf_cap_bytes {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("{head} flush_min_bytes must be in [1..buf_cap_bytes]"),
                ));
            }

            let on_finish = match fields.get("on_finish") {
                None => NetSinkOnFinishV1::Close,
                Some(v) => match v.as_ident() {
                    Some("shutdown_write") => NetSinkOnFinishV1::ShutdownWrite,
                    Some("close") => NetSinkOnFinishV1::Close,
                    _ => {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            format!("{head} on_finish must be \"shutdown_write\" or \"close\""),
                        ));
                    }
                },
            };

            let max_flushes = fields
                .get("max_flushes")
                .map(|v| expect_i32(v, "max_flushes must be an integer"))
                .transpose()?
                .unwrap_or(0);
            if max_flushes < 0 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("{head} max_flushes must be >= 0"),
                ));
            }

            let max_write_calls = fields
                .get("max_write_calls")
                .map(|v| expect_i32(v, "max_write_calls must be an integer"))
                .transpose()?
                .unwrap_or(0);
            if max_write_calls < 0 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("{head} max_write_calls must be >= 0"),
                ));
            }

            Ok(PipeSinkV1::NetTcpConnectWrite {
                addr_param,
                caps_param,
                cfg: NetTcpWriteStreamHandleCfgV1 {
                    buf_cap_bytes,
                    flush_min_bytes,
                    max_flushes,
                    max_write_calls,
                    on_finish,
                },
            })
        }
        _ => Err(CompilerError::new(
            CompileErrorKind::Typing,
            format!("unsupported pipe sink: {head}"),
        )),
    }
}

fn validate_pipe_world_caps(
    pipe: &PipeParsed,
    options: &CompileOptions,
) -> Result<(), CompilerError> {
    fn sink_needs_os(sink: &PipeSinkV1) -> bool {
        match sink {
            PipeSinkV1::CollectBytes | PipeSinkV1::HashFnv1a32 | PipeSinkV1::Null => false,
            PipeSinkV1::WorldFsWriteFile { .. }
            | PipeSinkV1::WorldFsWriteStream { .. }
            | PipeSinkV1::WorldFsWriteStreamHashFnv1a32 { .. }
            | PipeSinkV1::NetTcpWriteStreamHandle { .. }
            | PipeSinkV1::NetTcpConnectWrite { .. } => true,
            PipeSinkV1::U32Frames { inner } => sink_needs_os(inner),
        }
    }

    let src_needs_os = matches!(
        pipe.src,
        PipeSrcV1::DbRowsDoc { .. } | PipeSrcV1::NetTcpReadStreamHandle { .. }
    );

    let needs_os = src_needs_os || sink_needs_os(&pipe.sink);
    if needs_os && !options.world.is_standalone_only() {
        return Err(CompilerError::new(
            CompileErrorKind::Unsupported,
            format!(
                "std.stream.pipe_v1 requires an OS world (run-os, run-os-sandboxed); got {}",
                options.world.as_str()
            ),
        ));
    }
    Ok(())
}

fn gen_pipe_helper_body(
    pipe: &PipeParsed,
    _options: &CompileOptions,
) -> Result<Expr, CompilerError> {
    let mut cfg = pipe.cfg.clone();

    let emit_payload = cfg.emit_payload.unwrap_or(1) != 0;
    let emit_stats = cfg.emit_stats.unwrap_or(1) != 0;

    if cfg.chunk_max_bytes <= 0
        || cfg.bufread_cap_bytes <= 0
        || cfg.max_in_bytes <= 0
        || cfg.max_out_bytes <= 0
        || cfg.max_items <= 0
        || cfg.bufread_cap_bytes < cfg.chunk_max_bytes
    {
        return Ok(err_doc_const(E_CFG_INVALID, "stream:cfg_invalid"));
    }

    // Resolve cfg.max_in_bytes to include json_canon_stream_v1's optional max_total_json_bytes.
    {
        let mut json_stage_count = 0;
        let mut max_total_json_bytes = 0;
        for xf in &pipe.chain {
            if let PipeXfV1::JsonCanonStreamV1 { cfg: jcfg } = xf {
                json_stage_count += 1;
                if json_stage_count > 1 {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        "std.stream.pipe_v1 supports at most one std.stream.xf.json_canon_stream_v1".to_string(),
                    ));
                }
                max_total_json_bytes = jcfg.max_total_json_bytes;
            }
        }
        if max_total_json_bytes > 0 && max_total_json_bytes < cfg.max_in_bytes {
            cfg.max_in_bytes = max_total_json_bytes;
        }
    }

    let max_steps = if let Some(ms) = cfg.max_steps.filter(|&v| v > 0) {
        ms
    } else {
        // Conservative bound: ceil(max_in_bytes / chunk_max_bytes) + slack.
        let chunk = cfg.chunk_max_bytes.max(1) as i64;
        let max_in = cfg.max_in_bytes as i64;
        let steps = (max_in + chunk - 1) / chunk + 8;
        i32::try_from(steps.min(i32::MAX as i64)).unwrap_or(i32::MAX)
    };

    let mut sink_shape = sink_shape_v1(&pipe.sink)?;
    match &mut sink_shape.base {
        SinkBaseV1::WorldFsWriteStream { cfg: fs_cfg, .. }
        | SinkBaseV1::WorldFsWriteStreamHashFnv1a32 { cfg: fs_cfg, .. } => {
            if fs_cfg.buf_cap_bytes == 0 {
                fs_cfg.buf_cap_bytes = cfg.chunk_max_bytes;
            }
            if fs_cfg.flush_min_bytes == 0 {
                fs_cfg.flush_min_bytes = fs_cfg.buf_cap_bytes;
            }
            if fs_cfg.buf_cap_bytes <= 0 || fs_cfg.flush_min_bytes <= 0 {
                return Ok(err_doc_const(E_CFG_INVALID, "stream:fs_sink_cfg_invalid"));
            }
            if fs_cfg.flush_min_bytes > fs_cfg.buf_cap_bytes {
                return Ok(err_doc_const(E_CFG_INVALID, "stream:fs_sink_cfg_invalid"));
            }
            if fs_cfg.max_flushes == 0 {
                // max_flushes_default = ceil(max_out_bytes / max(1, flush_min_bytes)) + 4
                let out = i64::from(cfg.max_out_bytes.max(1));
                let fmin = i64::from(fs_cfg.flush_min_bytes.max(1));
                let derived = (out + fmin - 1) / fmin + 4;
                fs_cfg.max_flushes =
                    i32::try_from(derived.min(i64::from(i32::MAX))).unwrap_or(i32::MAX);
            }
            if fs_cfg.max_flushes <= 0 {
                return Ok(err_doc_const(E_CFG_INVALID, "stream:fs_sink_cfg_invalid"));
            }
        }
        _ => {}
    }
    let mut take_states: Vec<TakeState> = Vec::new();
    let mut map_in_place_states: Vec<MapInPlaceState> = Vec::new();
    let mut split_states: Vec<SplitLinesState> = Vec::new();
    let mut deframe_states: Vec<DeframeState> = Vec::new();
    let mut json_canon_states: Vec<JsonCanonState> = Vec::new();
    for (idx, xf) in pipe.chain.iter().enumerate() {
        match xf {
            PipeXfV1::Take { n_param } => {
                take_states.push(TakeState {
                    stage_idx: idx,
                    n_param: *n_param,
                    rem_var: format!("take_rem_{idx}"),
                });
            }
            PipeXfV1::MapInPlaceBufV1 {
                scratch_cap_bytes,
                clear_before_each,
                fn_id,
            } => {
                map_in_place_states.push(MapInPlaceState {
                    stage_idx: idx,
                    scratch_cap_bytes: *scratch_cap_bytes,
                    clear_before_each: *clear_before_each,
                    fn_id: fn_id.clone(),
                    scratch_var: format!("scratch_{idx}"),
                });
            }
            PipeXfV1::SplitLines {
                delim_param,
                max_line_bytes_param,
            } => {
                split_states.push(SplitLinesState {
                    stage_idx: idx,
                    delim_param: *delim_param,
                    max_line_bytes_param: *max_line_bytes_param,
                    carry_var: format!("split_carry_{idx}"),
                });
            }
            PipeXfV1::DeframeU32LeV1 { cfg } => {
                deframe_states.push(DeframeState {
                    stage_idx: idx,
                    cfg: cfg.clone(),
                    hdr_var: format!("deframe_hdr_{idx}"),
                    hdr_fill_var: format!("deframe_hdr_fill_{idx}"),
                    need_var: format!("deframe_need_{idx}"),
                    buf_var: format!("deframe_buf_{idx}"),
                    buf_fill_var: format!("deframe_buf_fill_{idx}"),
                    frames_var: format!("deframe_frames_{idx}"),
                });
            }
            PipeXfV1::MapBytes { .. } | PipeXfV1::Filter { .. } | PipeXfV1::FrameU32Le => {}
            PipeXfV1::JsonCanonStreamV1 { cfg: jcfg } => {
                let max_depth = if jcfg.max_depth > 0 {
                    jcfg.max_depth
                } else {
                    64
                };
                let max_total_json_bytes = if jcfg.max_total_json_bytes > 0 {
                    jcfg.max_total_json_bytes
                } else {
                    cfg.max_in_bytes
                };
                let max_object_members = if jcfg.max_object_members > 0 {
                    jcfg.max_object_members
                } else {
                    4096
                };
                let max_object_total_bytes = if jcfg.max_object_total_bytes > 0 {
                    jcfg.max_object_total_bytes
                } else {
                    cfg.max_out_bytes.min(4 * 1024 * 1024)
                };
                let emit_chunk_max_bytes = if jcfg.emit_chunk_max_bytes > 0 {
                    jcfg.emit_chunk_max_bytes
                } else {
                    cfg.chunk_max_bytes
                };

                if max_depth <= 0
                    || max_total_json_bytes <= 0
                    || max_object_members <= 0
                    || max_object_total_bytes <= 0
                    || emit_chunk_max_bytes <= 0
                {
                    return Ok(err_doc_const(
                        E_CFG_INVALID,
                        "stream:json_canon_cfg_invalid",
                    ));
                }

                json_canon_states.push(JsonCanonState {
                    stage_idx: idx,
                    cfg: JsonCanonStreamCfgV1 {
                        max_depth,
                        max_total_json_bytes,
                        max_object_members,
                        max_object_total_bytes,
                        emit_chunk_max_bytes,
                    },
                    buf_var: format!("json_buf_{idx}"),
                });
            }
        }
    }

    let has_take = !take_states.is_empty();

    let sink_vec_var = match sink_shape.base {
        SinkBaseV1::CollectBytes | SinkBaseV1::WorldFsWriteFile { .. } => {
            Some("sink_vec".to_string())
        }
        SinkBaseV1::HashFnv1a32
        | SinkBaseV1::Null
        | SinkBaseV1::WorldFsWriteStream { .. }
        | SinkBaseV1::WorldFsWriteStreamHashFnv1a32 { .. }
        | SinkBaseV1::NetTcpWriteStreamHandle { .. }
        | SinkBaseV1::NetTcpConnectWrite { .. } => None,
    };
    let hash_var = match sink_shape.base {
        SinkBaseV1::HashFnv1a32 | SinkBaseV1::WorldFsWriteStreamHashFnv1a32 { .. } => {
            Some("hash".to_string())
        }
        _ => None,
    };

    let cg = PipeCodegen {
        cfg: &cfg,
        chain: pipe.chain.as_slice(),
        emit_payload,
        emit_stats,
        max_steps,
        sink: sink_shape,
        bytes_in_var: "bytes_in".to_string(),
        bytes_out_var: "bytes_out".to_string(),
        items_in_var: "items_in".to_string(),
        items_out_var: "items_out".to_string(),
        stop_var: if has_take {
            Some("stop".to_string())
        } else {
            None
        },
        take_states,
        map_in_place_states,
        split_states,
        deframe_states,
        json_canon_states,
        sink_vec_var,
        hash_var,
    };

    let mut items: Vec<Expr> = vec![expr_ident("begin")];

    items.push(let_i32(&cg.bytes_in_var, 0));
    items.push(let_i32(&cg.bytes_out_var, 0));
    items.push(let_i32(&cg.items_in_var, 0));
    items.push(let_i32(&cg.items_out_var, 0));
    if let Some(stop) = &cg.stop_var {
        items.push(let_i32(stop, 0));
    }

    // Init map_in_place_buf scratch handles.
    for s in &cg.map_in_place_states {
        if s.scratch_cap_bytes <= 0 {
            items.push(expr_list(vec![
                expr_ident("return"),
                err_doc_const(E_CFG_INVALID, "stream:scratch_cap_bytes_invalid"),
            ]));
            continue;
        }
        items.push(expr_list(vec![
            expr_ident("let"),
            expr_ident(s.scratch_var.clone()),
            expr_list(vec![
                expr_ident("scratch_u8_fixed_v1.new"),
                expr_int(s.scratch_cap_bytes),
            ]),
        ]));
    }

    // Init take stage counters.
    for t in &cg.take_states {
        let p = param_ident(t.n_param);
        items.push(expr_list(vec![
            expr_ident("let"),
            expr_ident(t.rem_var.clone()),
            expr_list(vec![
                expr_ident("if"),
                expr_list(vec![expr_ident("<"), p, expr_int(0)]),
                expr_int(0),
                param_ident(t.n_param),
            ]),
        ]));
    }

    // Init split_lines carry buffers (and validate max_line_bytes).
    for s in &cg.split_states {
        items.push(expr_list(vec![
            expr_ident("if"),
            expr_list(vec![
                expr_ident("<="),
                param_ident(s.max_line_bytes_param),
                expr_int(0),
            ]),
            expr_list(vec![
                expr_ident("return"),
                err_doc_const(E_CFG_INVALID, "stream:split_lines_max_line_bytes"),
            ]),
            expr_int(0),
        ]));
        items.push(expr_list(vec![
            expr_ident("let"),
            expr_ident(s.carry_var.clone()),
            expr_list(vec![
                expr_ident("vec_u8.with_capacity"),
                param_ident(s.max_line_bytes_param),
            ]),
        ]));
    }

    // Init deframe_u32le state.
    for d in &cg.deframe_states {
        if d.cfg.max_frame_bytes <= 0 {
            items.push(expr_list(vec![
                expr_ident("return"),
                err_doc_const(E_CFG_INVALID, "stream:deframe_max_frame_bytes"),
            ]));
            continue;
        }

        items.push(let_i32(&d.hdr_fill_var, 0));
        items.push(let_i32(&d.need_var, 0));
        items.push(let_i32(&d.buf_fill_var, 0));
        items.push(let_i32(&d.frames_var, 0));

        items.push(expr_list(vec![
            expr_ident("let"),
            expr_ident(d.hdr_var.clone()),
            expr_list(vec![expr_ident("vec_u8.with_capacity"), expr_int(4)]),
        ]));
        items.push(expr_list(vec![
            expr_ident("set"),
            expr_ident(d.hdr_var.clone()),
            expr_list(vec![
                expr_ident("vec_u8.extend_zeroes"),
                expr_ident(d.hdr_var.clone()),
                expr_int(4),
            ]),
        ]));

        items.push(expr_list(vec![
            expr_ident("let"),
            expr_ident(d.buf_var.clone()),
            expr_list(vec![
                expr_ident("vec_u8.with_capacity"),
                expr_int(d.cfg.max_frame_bytes),
            ]),
        ]));
        items.push(expr_list(vec![
            expr_ident("set"),
            expr_ident(d.buf_var.clone()),
            expr_list(vec![
                expr_ident("vec_u8.extend_zeroes"),
                expr_ident(d.buf_var.clone()),
                expr_int(d.cfg.max_frame_bytes),
            ]),
        ]));
    }

    // Init json_canon_stream buffers.
    for j in &cg.json_canon_states {
        let cap = j
            .cfg
            .max_total_json_bytes
            .min(cg.cfg.chunk_max_bytes)
            .max(1);
        items.push(expr_list(vec![
            expr_ident("let"),
            expr_ident(j.buf_var.clone()),
            expr_list(vec![expr_ident("vec_u8.with_capacity"), expr_int(cap)]),
        ]));
    }

    // Init sink state.
    if let Some(vec_name) = &cg.sink_vec_var {
        items.push(expr_list(vec![
            expr_ident("let"),
            expr_ident(vec_name.clone()),
            expr_list(vec![
                expr_ident("vec_u8.with_capacity"),
                expr_int(cg.cfg.max_out_bytes),
            ]),
        ]));
    }
    if let Some(hash_name) = &cg.hash_var {
        items.push(let_i32(hash_name, FNV1A32_OFFSET_BASIS));
    }
    match &cg.sink.base {
        SinkBaseV1::NetTcpWriteStreamHandle {
            stream_handle_param,
            caps_param,
            cfg,
        } => {
            items.push(expr_list(vec![
                expr_ident("let"),
                expr_ident("net_sink_h".to_string()),
                param_ident(*stream_handle_param),
            ]));
            items.push(let_i32("net_sink_owned", 0));
            items.push(expr_list(vec![
                expr_ident("let"),
                expr_ident("net_sink_caps_b".to_string()),
                param_ident(*caps_param),
            ]));
            items.push(expr_list(vec![
                expr_ident("let"),
                expr_ident("net_sink_caps".to_string()),
                expr_list(vec![
                    expr_ident("bytes.view"),
                    expr_ident("net_sink_caps_b".to_string()),
                ]),
            ]));
            emit_net_sink_validate_caps(&mut items);
            emit_net_sink_init_limits_and_buffer(&mut items, cg.cfg.max_out_bytes, *cfg);
        }
        SinkBaseV1::NetTcpConnectWrite {
            addr_param,
            caps_param,
            cfg,
        } => {
            items.push(expr_list(vec![
                expr_ident("let"),
                expr_ident("net_sink_addr_b".to_string()),
                param_ident(*addr_param),
            ]));
            items.push(let_i32("net_sink_owned", 1));
            items.push(expr_list(vec![
                expr_ident("let"),
                expr_ident("net_sink_addr".to_string()),
                expr_list(vec![
                    expr_ident("bytes.view"),
                    expr_ident("net_sink_addr_b".to_string()),
                ]),
            ]));
            items.push(expr_list(vec![
                expr_ident("let"),
                expr_ident("net_sink_caps_b".to_string()),
                param_ident(*caps_param),
            ]));
            items.push(expr_list(vec![
                expr_ident("let"),
                expr_ident("net_sink_caps".to_string()),
                expr_list(vec![
                    expr_ident("bytes.view"),
                    expr_ident("net_sink_caps_b".to_string()),
                ]),
            ]));
            emit_net_sink_validate_caps(&mut items);

            // Connect once (owned).
            items.push(expr_list(vec![
                expr_ident("let"),
                expr_ident("net_sink_connect_doc".to_string()),
                expr_list(vec![
                    expr_ident("std.net.tcp.connect_v1"),
                    expr_ident("net_sink_addr".to_string()),
                    expr_ident("net_sink_caps".to_string()),
                ]),
            ]));
            items.push(expr_list(vec![
                expr_ident("let"),
                expr_ident("net_sink_connect_dv".to_string()),
                expr_list(vec![
                    expr_ident("bytes.view"),
                    expr_ident("net_sink_connect_doc".to_string()),
                ]),
            ]));
            items.push(expr_list(vec![
                expr_ident("if"),
                expr_list(vec![
                    expr_ident("std.net.err.is_err_doc_v1"),
                    expr_ident("net_sink_connect_dv".to_string()),
                ]),
                expr_list(vec![
                    expr_ident("return"),
                    err_doc_with_payload(
                        expr_int(E_NET_WRITE_FAILED),
                        "stream:net_connect_failed",
                        expr_ident("net_sink_connect_doc".to_string()),
                    ),
                ]),
                expr_int(0),
            ]));
            items.push(expr_list(vec![
                expr_ident("let"),
                expr_ident("net_sink_h".to_string()),
                expr_list(vec![
                    expr_ident("std.net.tcp.connect_stream_handle_v1"),
                    expr_ident("net_sink_connect_dv".to_string()),
                ]),
            ]));
            items.push(expr_list(vec![
                expr_ident("if"),
                expr_list(vec![
                    expr_ident("<="),
                    expr_ident("net_sink_h".to_string()),
                    expr_int(0),
                ]),
                expr_list(vec![
                    expr_ident("return"),
                    err_doc_const(E_NET_READ_DOC_INVALID, "stream:net_connect_doc_invalid"),
                ]),
                expr_int(0),
            ]));
            emit_net_sink_init_limits_and_buffer(&mut items, cg.cfg.max_out_bytes, *cfg);
        }
        _ => {}
    }

    if let SinkBaseV1::WorldFsWriteStream {
        path_param,
        caps_param,
        cfg,
    }
    | SinkBaseV1::WorldFsWriteStreamHashFnv1a32 {
        path_param,
        caps_param,
        cfg,
    } = &cg.sink.base
    {
        items.push(expr_list(vec![
            expr_ident("let"),
            expr_ident("fs_sink_path_b".to_string()),
            param_ident(*path_param),
        ]));
        items.push(expr_list(vec![
            expr_ident("let"),
            expr_ident("fs_sink_caps_b".to_string()),
            param_ident(*caps_param),
        ]));
        items.push(expr_list(vec![
            expr_ident("let"),
            expr_ident("fs_open".to_string()),
            expr_list(vec![
                expr_ident("os.fs.stream_open_write_v1"),
                expr_ident("fs_sink_path_b".to_string()),
                expr_ident("fs_sink_caps_b".to_string()),
            ]),
        ]));
        items.push(expr_list(vec![
            expr_ident("if"),
            expr_list(vec![
                expr_ident("result_i32.is_ok"),
                expr_ident("fs_open".to_string()),
            ]),
            expr_int(0),
            expr_list(vec![
                expr_ident("begin"),
                expr_list(vec![
                    expr_ident("let"),
                    expr_ident("ec".to_string()),
                    expr_list(vec![
                        expr_ident("result_i32.err_code"),
                        expr_ident("fs_open".to_string()),
                    ]),
                ]),
                expr_list(vec![
                    expr_ident("let"),
                    expr_ident("pl".to_string()),
                    expr_list(vec![expr_ident("vec_u8.with_capacity"), expr_int(12)]),
                ]),
                extend_u32("pl", expr_ident("ec".to_string())),
                extend_u32("pl", expr_int(0)),
                extend_u32("pl", expr_ident(cg.bytes_out_var.clone())),
                expr_list(vec![
                    expr_ident("return"),
                    err_doc_with_payload(
                        expr_int(E_SINK_FS_OPEN_FAILED),
                        "stream:fs_open_failed",
                        expr_list(vec![
                            expr_ident("vec_u8.into_bytes"),
                            expr_ident("pl".to_string()),
                        ]),
                    ),
                ]),
            ]),
        ]));
        items.push(expr_list(vec![
            expr_ident("let"),
            expr_ident("fs_sink_h".to_string()),
            expr_list(vec![
                expr_ident("result_i32.unwrap_or"),
                expr_ident("fs_open".to_string()),
                expr_int(-1),
            ]),
        ]));
        items.push(let_i32("fs_sink_flushes", 0));
        items.push(let_i32("fs_sink_max_flushes", cfg.max_flushes));
        items.push(expr_list(vec![
            expr_ident("let"),
            expr_ident("fs_sink_buf".to_string()),
            expr_list(vec![
                expr_ident("vec_u8.with_capacity"),
                expr_int(cfg.buf_cap_bytes),
            ]),
        ]));
    }

    let main = match &pipe.src {
        PipeSrcV1::Bytes { bytes_param } => cg.gen_run_bytes_source(*bytes_param)?,
        PipeSrcV1::FsOpenRead { path_param } => cg.gen_run_reader_source(
            "fs.open_read",
            param_ident(*path_param),
            cfg.bufread_cap_bytes,
        )?,
        PipeSrcV1::RrSend { key_param } => {
            cg.gen_run_reader_source("rr.send", param_ident(*key_param), cfg.bufread_cap_bytes)?
        }
        PipeSrcV1::DbRowsDoc {
            conn_param,
            sql_param,
            params_doc_param,
            qcaps_doc_param,
        } => cg.gen_run_db_rows_doc_source(
            *conn_param,
            *sql_param,
            *params_doc_param,
            *qcaps_doc_param,
        )?,
        PipeSrcV1::NetTcpReadStreamHandle {
            stream_handle_param,
            caps_param,
            on_timeout,
            on_eof,
        } => cg.gen_run_net_tcp_read_stream_handle_source(
            *stream_handle_param,
            *caps_param,
            *on_timeout,
            *on_eof,
        )?,
        PipeSrcV1::NetTcpReadU32Frames { .. } => {
            return Err(CompilerError::new(
                CompileErrorKind::Internal,
                "internal error: net_tcp_read_u32frames should have been desugared".to_string(),
            ));
        }
    };
    items.push(main);

    Ok(expr_list(items))
}

const E_CFG_INVALID: i32 = 1;
const E_BUDGET_IN_BYTES: i32 = 2;
const E_BUDGET_OUT_BYTES: i32 = 3;
const E_BUDGET_ITEMS: i32 = 4;
const E_LINE_TOO_LONG: i32 = 5;
const E_DB_QUERY_FAILED: i32 = 7;
const E_SCRATCH_OVERFLOW: i32 = 8;
const E_STAGE_FAILED: i32 = 9;
const E_FRAME_TOO_LARGE: i32 = 10;

const E_JSON_SYNTAX: i32 = 20;
const E_JSON_NOT_IJSON: i32 = 21;
const E_JSON_TOO_DEEP: i32 = 22;
const E_JSON_OBJECT_TOO_LARGE: i32 = 23;
const E_JSON_TRAILING_DATA: i32 = 24;

const E_SINK_FS_OPEN_FAILED: i32 = 40;
const E_SINK_FS_WRITE_FAILED: i32 = 41;
const E_SINK_FS_CLOSE_FAILED: i32 = 42;
const E_SINK_TOO_MANY_FLUSHES: i32 = 43;

const E_NET_CAPS_INVALID: i32 = 60;
const E_NET_READ_FAILED: i32 = 61;
const E_NET_READ_DOC_INVALID: i32 = 62;
const E_NET_BACKEND_OVERRAN_MAX: i32 = 63;
const E_NET_WRITE_FAILED: i32 = 64;
const E_NET_SINK_MAX_FLUSHES: i32 = 65;
const E_NET_SINK_MAX_WRITES: i32 = 66;

const E_DEFRAME_FRAME_TOO_LARGE: i32 = 80;
const E_DEFRAME_TRUNCATED: i32 = 81;
const E_DEFRAME_EMPTY_FORBIDDEN: i32 = 82;
const E_DEFRAME_MAX_FRAMES: i32 = 83;
const E_DEFRAME_TRUNCATED_TIMEOUT: i32 = 84;

const FNV1A32_OFFSET_BASIS: i32 = -2128831035; // 0x811c9dc5
const FNV1A32_PRIME: i32 = 16777619;

#[derive(Clone)]
struct TakeState {
    stage_idx: usize,
    n_param: usize,
    rem_var: String,
}

#[derive(Clone)]
struct MapInPlaceState {
    stage_idx: usize,
    scratch_cap_bytes: i32,
    clear_before_each: i32,
    fn_id: String,
    scratch_var: String,
}

#[derive(Clone)]
struct SplitLinesState {
    stage_idx: usize,
    delim_param: usize,
    max_line_bytes_param: usize,
    carry_var: String,
}

#[derive(Clone)]
struct DeframeState {
    stage_idx: usize,
    cfg: DeframeU32LeCfgV1,
    hdr_var: String,
    hdr_fill_var: String,
    need_var: String,
    buf_var: String,
    buf_fill_var: String,
    frames_var: String,
}

#[derive(Clone)]
struct JsonCanonState {
    stage_idx: usize,
    cfg: JsonCanonStreamCfgV1,
    buf_var: String,
}

#[derive(Clone)]
enum SinkBaseV1 {
    CollectBytes,
    HashFnv1a32,
    Null,
    WorldFsWriteFile {
        path_param: usize,
    },
    WorldFsWriteStream {
        path_param: usize,
        caps_param: usize,
        cfg: WorldFsWriteStreamCfgV1,
    },
    WorldFsWriteStreamHashFnv1a32 {
        path_param: usize,
        caps_param: usize,
        cfg: WorldFsWriteStreamCfgV1,
    },
    NetTcpWriteStreamHandle {
        stream_handle_param: usize,
        caps_param: usize,
        cfg: NetTcpWriteStreamHandleCfgV1,
    },
    NetTcpConnectWrite {
        addr_param: usize,
        caps_param: usize,
        cfg: NetTcpWriteStreamHandleCfgV1,
    },
}

#[derive(Clone)]
struct SinkShapeV1 {
    framing_u32frames: bool,
    base: SinkBaseV1,
}

fn sink_shape_v1(sink: &PipeSinkV1) -> Result<SinkShapeV1, CompilerError> {
    match sink {
        PipeSinkV1::CollectBytes => Ok(SinkShapeV1 {
            framing_u32frames: false,
            base: SinkBaseV1::CollectBytes,
        }),
        PipeSinkV1::HashFnv1a32 => Ok(SinkShapeV1 {
            framing_u32frames: false,
            base: SinkBaseV1::HashFnv1a32,
        }),
        PipeSinkV1::Null => Ok(SinkShapeV1 {
            framing_u32frames: false,
            base: SinkBaseV1::Null,
        }),
        PipeSinkV1::WorldFsWriteFile { path_param } => Ok(SinkShapeV1 {
            framing_u32frames: false,
            base: SinkBaseV1::WorldFsWriteFile {
                path_param: *path_param,
            },
        }),
        PipeSinkV1::WorldFsWriteStream {
            path_param,
            caps_param,
            cfg,
        } => Ok(SinkShapeV1 {
            framing_u32frames: false,
            base: SinkBaseV1::WorldFsWriteStream {
                path_param: *path_param,
                caps_param: *caps_param,
                cfg: *cfg,
            },
        }),
        PipeSinkV1::WorldFsWriteStreamHashFnv1a32 {
            path_param,
            caps_param,
            cfg,
        } => Ok(SinkShapeV1 {
            framing_u32frames: false,
            base: SinkBaseV1::WorldFsWriteStreamHashFnv1a32 {
                path_param: *path_param,
                caps_param: *caps_param,
                cfg: *cfg,
            },
        }),
        PipeSinkV1::NetTcpWriteStreamHandle {
            stream_handle_param,
            caps_param,
            cfg,
        } => Ok(SinkShapeV1 {
            framing_u32frames: false,
            base: SinkBaseV1::NetTcpWriteStreamHandle {
                stream_handle_param: *stream_handle_param,
                caps_param: *caps_param,
                cfg: *cfg,
            },
        }),
        PipeSinkV1::NetTcpConnectWrite {
            addr_param,
            caps_param,
            cfg,
        } => Ok(SinkShapeV1 {
            framing_u32frames: false,
            base: SinkBaseV1::NetTcpConnectWrite {
                addr_param: *addr_param,
                caps_param: *caps_param,
                cfg: *cfg,
            },
        }),
        PipeSinkV1::U32Frames { inner } => {
            let inner = sink_shape_v1(inner)?;
            if inner.framing_u32frames {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    "std.stream.sink.u32frames_v1 cannot wrap another u32frames_v1".to_string(),
                ));
            }
            Ok(SinkShapeV1 {
                framing_u32frames: true,
                base: inner.base,
            })
        }
    }
}

struct PipeCodegen<'a> {
    cfg: &'a PipeCfgV1,
    chain: &'a [PipeXfV1],
    emit_payload: bool,
    emit_stats: bool,
    max_steps: i32,

    sink: SinkShapeV1,
    bytes_in_var: String,
    bytes_out_var: String,
    items_in_var: String,
    items_out_var: String,

    stop_var: Option<String>,
    take_states: Vec<TakeState>,
    map_in_place_states: Vec<MapInPlaceState>,
    split_states: Vec<SplitLinesState>,
    deframe_states: Vec<DeframeState>,
    json_canon_states: Vec<JsonCanonState>,

    sink_vec_var: Option<String>,
    hash_var: Option<String>,
}

impl PipeCodegen<'_> {
    fn has_flush_stages(&self) -> bool {
        !self.split_states.is_empty()
            || !self.deframe_states.is_empty()
            || !self.json_canon_states.is_empty()
    }

    fn gen_run_bytes_source(&self, bytes_param: usize) -> Result<Expr, CompilerError> {
        let item_b = param_ident(bytes_param);
        let item_v = expr_list(vec![expr_ident("bytes.view"), item_b.clone()]);

        let mut stmts = vec![expr_list(vec![
            expr_ident("let"),
            expr_ident("item_len".to_string()),
            expr_list(vec![expr_ident("view.len"), item_v.clone()]),
        ])];
        stmts.push(set_add_i32(
            &self.bytes_in_var,
            expr_ident("item_len".to_string()),
        ));
        stmts.push(set_add_i32(&self.items_in_var, expr_int(1)));
        stmts.push(self.budget_check_in()?);
        stmts.push(self.gen_process_from(0, item_v)?);

        if let Some(stop) = &self.stop_var {
            stmts.push(expr_list(vec![
                expr_ident("if"),
                expr_list(vec![expr_ident("="), expr_ident(stop.clone()), expr_int(1)]),
                self.gen_return_ok()?,
                expr_int(0),
            ]));
        }

        if self.has_flush_stages() {
            stmts.push(self.gen_flush_from(0)?);
        }
        stmts.push(self.gen_return_ok()?);

        Ok(expr_list(
            vec![expr_ident("begin")].into_iter().chain(stmts).collect(),
        ))
    }

    fn gen_run_reader_source(
        &self,
        open_head: &str,
        open_arg: Expr,
        bufread_cap_bytes: i32,
    ) -> Result<Expr, CompilerError> {
        let mut stmts = Vec::new();
        stmts.push(expr_list(vec![
            expr_ident("let"),
            expr_ident("reader".to_string()),
            expr_list(vec![expr_ident(open_head.to_string()), open_arg]),
        ]));
        stmts.push(expr_list(vec![
            expr_ident("let"),
            expr_ident("br".to_string()),
            expr_list(vec![
                expr_ident("bufread.new"),
                expr_ident("reader".to_string()),
                expr_int(bufread_cap_bytes),
            ]),
        ]));

        let loop_body = self.gen_reader_loop_body()?;
        stmts.push(expr_list(vec![
            expr_ident("for"),
            expr_ident("step".to_string()),
            expr_int(0),
            expr_int(self.max_steps),
            loop_body,
        ]));
        stmts.push(expr_list(vec![
            expr_ident("return"),
            err_doc_const(E_CFG_INVALID, "stream:max_steps_exceeded"),
        ]));
        Ok(expr_list(
            vec![expr_ident("begin")].into_iter().chain(stmts).collect(),
        ))
    }

    fn gen_run_db_rows_doc_source(
        &self,
        conn_param: usize,
        sql_param: usize,
        params_doc_param: usize,
        qcaps_doc_param: usize,
    ) -> Result<Expr, CompilerError> {
        let mut stmts = vec![expr_list(vec![
            expr_ident("let"),
            expr_ident("resp".to_string()),
            expr_list(vec![
                expr_ident("std.db.query_v1"),
                param_ident(conn_param),
                expr_list(vec![expr_ident("view.to_bytes"), param_ident(sql_param)]),
                param_ident(params_doc_param),
                expr_list(vec![expr_ident("bytes.view"), param_ident(qcaps_doc_param)]),
            ]),
        ])];
        stmts.push(expr_list(vec![
            expr_ident("let"),
            expr_ident("rows_doc".to_string()),
            expr_list(vec![
                expr_ident("std.db.query_rows_doc_v1"),
                expr_ident("resp".to_string()),
            ]),
        ]));

        stmts.push(expr_list(vec![
            expr_ident("let"),
            expr_ident("doc".to_string()),
            expr_list(vec![
                expr_ident("bytes.view"),
                expr_ident("rows_doc".to_string()),
            ]),
        ]));

        stmts.push(expr_list(vec![
            expr_ident("if"),
            expr_list(vec![
                expr_ident("="),
                expr_list(vec![
                    expr_ident("ext.data_model.doc_is_err"),
                    expr_ident("doc".to_string()),
                ]),
                expr_int(1),
            ]),
            expr_list(vec![
                expr_ident("return"),
                err_doc_with_payload(
                    expr_int(E_DB_QUERY_FAILED),
                    "stream:db_query_failed",
                    expr_list(vec![
                        expr_ident("codec.write_u32_le"),
                        expr_list(vec![
                            expr_ident("ext.data_model.doc_error_code"),
                            expr_ident("doc".to_string()),
                        ]),
                    ]),
                ),
            ]),
            expr_int(0),
        ]));

        stmts.push(expr_list(vec![
            expr_ident("if"),
            expr_list(vec![
                expr_ident("!="),
                expr_list(vec![
                    expr_ident("ext.data_model.root_kind"),
                    expr_ident("doc".to_string()),
                ]),
                expr_int(5),
            ]),
            expr_list(vec![
                expr_ident("return"),
                err_doc_const(E_CFG_INVALID, "stream:db_rows_doc_invalid"),
            ]),
            expr_int(0),
        ]));

        stmts.push(expr_list(vec![
            expr_ident("let"),
            expr_ident("root".to_string()),
            expr_list(vec![
                expr_ident("ext.data_model.root_offset"),
                expr_ident("doc".to_string()),
            ]),
        ]));

        stmts.push(expr_list(vec![
            expr_ident("let"),
            expr_ident("k_rows".to_string()),
            expr_list(vec![
                expr_ident("bytes.lit"),
                expr_ident("rows".to_string()),
            ]),
        ]));
        stmts.push(expr_list(vec![
            expr_ident("let"),
            expr_ident("rows_off".to_string()),
            expr_list(vec![
                expr_ident("ext.data_model.map_find"),
                expr_ident("doc".to_string()),
                expr_ident("root".to_string()),
                expr_list(vec![
                    expr_ident("bytes.view"),
                    expr_ident("k_rows".to_string()),
                ]),
            ]),
        ]));
        stmts.push(expr_list(vec![
            expr_ident("if"),
            expr_list(vec![
                expr_ident("<"),
                expr_ident("rows_off".to_string()),
                expr_int(0),
            ]),
            expr_list(vec![
                expr_ident("return"),
                err_doc_const(E_CFG_INVALID, "stream:db_rows_doc_missing_rows"),
            ]),
            expr_int(0),
        ]));

        stmts.push(expr_list(vec![
            expr_ident("let"),
            expr_ident("rows_count".to_string()),
            expr_list(vec![
                expr_ident("ext.data_model.seq_len"),
                expr_ident("doc".to_string()),
                expr_ident("rows_off".to_string()),
            ]),
        ]));
        stmts.push(expr_list(vec![
            expr_ident("if"),
            expr_list(vec![
                expr_ident("<"),
                expr_ident("rows_count".to_string()),
                expr_int(0),
            ]),
            expr_list(vec![
                expr_ident("return"),
                err_doc_const(E_CFG_INVALID, "stream:db_rows_doc_rows_not_seq"),
            ]),
            expr_int(0),
        ]));

        stmts.push(expr_list(vec![
            expr_ident("let"),
            expr_ident("pos".to_string()),
            expr_list(vec![
                expr_ident("+"),
                expr_ident("rows_off".to_string()),
                expr_int(5),
            ]),
        ]));

        let mut loop_body = vec![expr_list(vec![
            expr_ident("let"),
            expr_ident("end".to_string()),
            expr_list(vec![
                expr_ident("ext.data_model.skip_value"),
                expr_ident("doc".to_string()),
                expr_ident("pos".to_string()),
            ]),
        ])];
        loop_body.push(expr_list(vec![
            expr_ident("if"),
            expr_list(vec![
                expr_ident("<"),
                expr_ident("end".to_string()),
                expr_int(0),
            ]),
            expr_list(vec![
                expr_ident("return"),
                err_doc_const(E_CFG_INVALID, "stream:db_rows_doc_row_bad"),
            ]),
            expr_int(0),
        ]));
        loop_body.push(expr_list(vec![
            expr_ident("let"),
            expr_ident("row".to_string()),
            expr_list(vec![
                expr_ident("view.slice"),
                expr_ident("doc".to_string()),
                expr_ident("pos".to_string()),
                expr_list(vec![
                    expr_ident("-"),
                    expr_ident("end".to_string()),
                    expr_ident("pos".to_string()),
                ]),
            ]),
        ]));

        loop_body.push(expr_list(vec![
            expr_ident("let"),
            expr_ident("row_len".to_string()),
            expr_list(vec![expr_ident("view.len"), expr_ident("row".to_string())]),
        ]));
        loop_body.push(set_add_i32(
            &self.bytes_in_var,
            expr_ident("row_len".to_string()),
        ));
        loop_body.push(set_add_i32(&self.items_in_var, expr_int(1)));
        loop_body.push(self.budget_check_in()?);
        loop_body.push(self.gen_process_from(0, expr_ident("row".to_string()))?);

        loop_body.push(expr_list(vec![
            expr_ident("set"),
            expr_ident("pos".to_string()),
            expr_ident("end".to_string()),
        ]));

        if let Some(stop) = &self.stop_var {
            loop_body.push(expr_list(vec![
                expr_ident("if"),
                expr_list(vec![expr_ident("="), expr_ident(stop.clone()), expr_int(1)]),
                self.gen_return_ok()?,
                expr_int(0),
            ]));
        }

        stmts.push(expr_list(vec![
            expr_ident("for"),
            expr_ident("_".to_string()),
            expr_int(0),
            expr_ident("rows_count".to_string()),
            expr_list(
                vec![expr_ident("begin")]
                    .into_iter()
                    .chain(loop_body)
                    .collect(),
            ),
        ]));

        if self.has_flush_stages() {
            stmts.push(self.gen_flush_from(0)?);
        }
        stmts.push(self.gen_return_ok()?);

        Ok(expr_list(
            vec![expr_ident("begin")].into_iter().chain(stmts).collect(),
        ))
    }

    fn gen_run_net_tcp_read_stream_handle_source(
        &self,
        stream_handle_param: usize,
        caps_param: usize,
        on_timeout: NetOnTimeoutV1,
        on_eof: NetOnEofV1,
    ) -> Result<Expr, CompilerError> {
        let h_var = "net_h".to_string();
        let caps_b_var = "net_caps_b".to_string();
        let caps_v_var = "net_caps".to_string();
        let read_max_var = "net_read_max".to_string();

        let cleanup_stmts: Vec<Expr> = match on_eof {
            NetOnEofV1::LeaveOpen => Vec::new(),
            NetOnEofV1::ShutdownRead => vec![expr_list(vec![
                expr_ident("std.net.tcp.stream_shutdown_v1"),
                expr_ident(h_var.clone()),
                expr_list(vec![expr_ident("std.net.tcp.shutdown_read_v1")]),
            ])],
            NetOnEofV1::Close => vec![
                expr_list(vec![
                    expr_ident("std.net.tcp.stream_close_v1"),
                    expr_ident(h_var.clone()),
                ]),
                expr_list(vec![
                    expr_ident("std.net.tcp.stream_drop_v1"),
                    expr_ident(h_var.clone()),
                ]),
            ],
        };

        let end_ok_with_flush = {
            let mut end_stmts = Vec::new();
            if self.has_flush_stages() {
                end_stmts.push(self.gen_flush_from(0)?);
            }
            end_stmts.extend(cleanup_stmts.clone());
            end_stmts.push(self.gen_return_ok()?);
            expr_list(
                vec![expr_ident("begin")]
                    .into_iter()
                    .chain(end_stmts)
                    .collect(),
            )
        };

        let end_ok_no_flush = {
            let mut end_stmts = Vec::new();
            end_stmts.extend(cleanup_stmts);
            end_stmts.push(self.gen_return_ok()?);
            expr_list(
                vec![expr_ident("begin")]
                    .into_iter()
                    .chain(end_stmts)
                    .collect(),
            )
        };

        let timeout_stop_if_clean_expr = if on_timeout == NetOnTimeoutV1::StopIfClean {
            let Some(d0) = self.deframe_states.iter().find(|d| d.stage_idx == 0) else {
                return Err(CompilerError::new(
                    CompileErrorKind::Internal,
                    "internal error: on_timeout=stop_if_clean without deframe at stage 0"
                        .to_string(),
                ));
            };
            expr_list(vec![
                expr_ident("if"),
                expr_list(vec![
                    expr_ident("if"),
                    expr_list(vec![
                        expr_ident("="),
                        expr_ident(d0.hdr_fill_var.clone()),
                        expr_int(0),
                    ]),
                    expr_list(vec![
                        expr_ident("="),
                        expr_ident(d0.buf_fill_var.clone()),
                        expr_int(0),
                    ]),
                    expr_int(0),
                ]),
                end_ok_with_flush.clone(),
                expr_list(vec![
                    expr_ident("return"),
                    err_doc_const(
                        E_DEFRAME_TRUNCATED_TIMEOUT,
                        "stream:deframe_truncated_timeout",
                    ),
                ]),
            ])
        } else {
            expr_int(0)
        };

        let mut stmts = vec![
            expr_list(vec![
                expr_ident("let"),
                expr_ident(h_var.clone()),
                param_ident(stream_handle_param),
            ]),
            expr_list(vec![
                expr_ident("let"),
                expr_ident(caps_b_var.clone()),
                param_ident(caps_param),
            ]),
            expr_list(vec![
                expr_ident("let"),
                expr_ident(caps_v_var.clone()),
                expr_list(vec![
                    expr_ident("bytes.view"),
                    expr_ident(caps_b_var.clone()),
                ]),
            ]),
        ];

        // Minimal caps validation (strict): len>=24, ver==1, reserved==0.
        stmts.push(expr_list(vec![
            expr_ident("if"),
            expr_list(vec![
                expr_ident("<"),
                expr_list(vec![expr_ident("view.len"), expr_ident(caps_v_var.clone())]),
                expr_int(24),
            ]),
            expr_list(vec![
                expr_ident("return"),
                err_doc_const(E_NET_CAPS_INVALID, "stream:net_caps_invalid"),
            ]),
            expr_int(0),
        ]));
        stmts.push(expr_list(vec![
            expr_ident("if"),
            expr_list(vec![
                expr_ident("!="),
                expr_list(vec![
                    expr_ident("codec.read_u32_le"),
                    expr_ident(caps_v_var.clone()),
                    expr_int(0),
                ]),
                expr_int(1),
            ]),
            expr_list(vec![
                expr_ident("return"),
                err_doc_const(E_NET_CAPS_INVALID, "stream:net_caps_invalid"),
            ]),
            expr_int(0),
        ]));
        stmts.push(expr_list(vec![
            expr_ident("if"),
            expr_list(vec![
                expr_ident("!="),
                expr_list(vec![
                    expr_ident("codec.read_u32_le"),
                    expr_ident(caps_v_var.clone()),
                    expr_int(20),
                ]),
                expr_int(0),
            ]),
            expr_list(vec![
                expr_ident("return"),
                err_doc_const(E_NET_CAPS_INVALID, "stream:net_caps_invalid"),
            ]),
            expr_int(0),
        ]));

        // Compute effective read max: min(cfg.chunk_max_bytes, caps.max_read_bytes) unless caps=0 (use cfg).
        stmts.push(expr_list(vec![
            expr_ident("let"),
            expr_ident("caps_max_read".to_string()),
            expr_list(vec![
                expr_ident("codec.read_u32_le"),
                expr_ident(caps_v_var.clone()),
                expr_int(12),
            ]),
        ]));
        stmts.push(expr_list(vec![
            expr_ident("let"),
            expr_ident(read_max_var.clone()),
            expr_list(vec![
                expr_ident("if"),
                expr_list(vec![
                    expr_ident("<="),
                    expr_ident("caps_max_read".to_string()),
                    expr_int(0),
                ]),
                expr_int(self.cfg.chunk_max_bytes),
                expr_list(vec![
                    expr_ident("if"),
                    expr_list(vec![
                        expr_ident("<u"),
                        expr_ident("caps_max_read".to_string()),
                        expr_int(self.cfg.chunk_max_bytes),
                    ]),
                    expr_ident("caps_max_read".to_string()),
                    expr_int(self.cfg.chunk_max_bytes),
                ]),
            ]),
        ]));

        let mut loop_body = Vec::new();
        loop_body.push(expr_list(vec![
            expr_ident("let"),
            expr_ident("doc".to_string()),
            expr_list(vec![
                expr_ident("std.net.tcp.stream_read_v1"),
                expr_ident(h_var.clone()),
                expr_ident(read_max_var.clone()),
                expr_ident(caps_v_var.clone()),
            ]),
        ]));
        loop_body.push(expr_list(vec![
            expr_ident("let"),
            expr_ident("dv".to_string()),
            expr_list(vec![
                expr_ident("bytes.view"),
                expr_ident("doc".to_string()),
            ]),
        ]));
        loop_body.push(expr_list(vec![
            expr_ident("if"),
            expr_list(vec![
                expr_ident("std.net.err.is_err_doc_v1"),
                expr_ident("dv".to_string()),
            ]),
            expr_list(vec![
                expr_ident("begin"),
                expr_list(vec![
                    expr_ident("let"),
                    expr_ident("code".to_string()),
                    expr_list(vec![
                        expr_ident("std.net.err.err_code_v1"),
                        expr_ident("dv".to_string()),
                    ]),
                ]),
                expr_list(vec![
                    expr_ident("if"),
                    expr_list(vec![
                        expr_ident("="),
                        expr_ident("code".to_string()),
                        expr_list(vec![expr_ident("std.net.err.code_timeout_v1")]),
                    ]),
                    match on_timeout {
                        NetOnTimeoutV1::Err => expr_list(vec![
                            expr_ident("return"),
                            err_doc_with_payload(
                                expr_int(E_NET_READ_FAILED),
                                "stream:net_timeout",
                                expr_ident("doc".to_string()),
                            ),
                        ]),
                        NetOnTimeoutV1::Stop => end_ok_with_flush.clone(),
                        NetOnTimeoutV1::StopIfClean => timeout_stop_if_clean_expr,
                    },
                    expr_int(0),
                ]),
                expr_list(vec![
                    expr_ident("return"),
                    err_doc_with_payload(
                        expr_int(E_NET_READ_FAILED),
                        "stream:net_read_failed",
                        expr_ident("doc".to_string()),
                    ),
                ]),
            ]),
            // OK doc path
            expr_list(vec![
                expr_ident("begin"),
                expr_list(vec![
                    expr_ident("let"),
                    expr_ident("doc_len".to_string()),
                    expr_list(vec![expr_ident("view.len"), expr_ident("dv".to_string())]),
                ]),
                expr_list(vec![
                    expr_ident("if"),
                    expr_list(vec![
                        expr_ident("<"),
                        expr_ident("doc_len".to_string()),
                        expr_int(8),
                    ]),
                    expr_list(vec![
                        expr_ident("return"),
                        err_doc_const(E_NET_READ_DOC_INVALID, "stream:net_read_doc_invalid"),
                    ]),
                    expr_int(0),
                ]),
                expr_list(vec![
                    expr_ident("if"),
                    expr_list(vec![
                        expr_ident("!="),
                        expr_list(vec![
                            expr_ident("view.get_u8"),
                            expr_ident("dv".to_string()),
                            expr_int(0),
                        ]),
                        expr_int(1),
                    ]),
                    expr_list(vec![
                        expr_ident("return"),
                        err_doc_const(E_NET_READ_DOC_INVALID, "stream:net_read_doc_invalid"),
                    ]),
                    expr_int(0),
                ]),
                expr_list(vec![
                    expr_ident("if"),
                    expr_list(vec![
                        expr_ident("!="),
                        expr_list(vec![
                            expr_ident("view.get_u8"),
                            expr_ident("dv".to_string()),
                            expr_int(1),
                        ]),
                        expr_int(1),
                    ]),
                    expr_list(vec![
                        expr_ident("return"),
                        err_doc_const(E_NET_READ_DOC_INVALID, "stream:net_read_doc_invalid"),
                    ]),
                    expr_int(0),
                ]),
                expr_list(vec![
                    expr_ident("if"),
                    expr_list(vec![
                        expr_ident("!="),
                        expr_list(vec![
                            expr_ident("view.get_u8"),
                            expr_ident("dv".to_string()),
                            expr_int(2),
                        ]),
                        expr_int(0),
                    ]),
                    expr_list(vec![
                        expr_ident("return"),
                        err_doc_const(E_NET_READ_DOC_INVALID, "stream:net_read_doc_invalid"),
                    ]),
                    expr_int(0),
                ]),
                expr_list(vec![
                    expr_ident("if"),
                    expr_list(vec![
                        expr_ident("!="),
                        expr_list(vec![
                            expr_ident("view.get_u8"),
                            expr_ident("dv".to_string()),
                            expr_int(3),
                        ]),
                        expr_int(0),
                    ]),
                    expr_list(vec![
                        expr_ident("return"),
                        err_doc_const(E_NET_READ_DOC_INVALID, "stream:net_read_doc_invalid"),
                    ]),
                    expr_int(0),
                ]),
                expr_list(vec![
                    expr_ident("let"),
                    expr_ident("pl_len".to_string()),
                    expr_list(vec![
                        expr_ident("codec.read_u32_le"),
                        expr_ident("dv".to_string()),
                        expr_int(4),
                    ]),
                ]),
                expr_list(vec![
                    expr_ident("if"),
                    expr_list(vec![
                        expr_ident("<"),
                        expr_ident("pl_len".to_string()),
                        expr_int(0),
                    ]),
                    expr_list(vec![
                        expr_ident("return"),
                        err_doc_const(E_NET_READ_DOC_INVALID, "stream:net_read_doc_invalid"),
                    ]),
                    expr_int(0),
                ]),
                expr_list(vec![
                    expr_ident("if"),
                    expr_list(vec![
                        expr_ident("!="),
                        expr_list(vec![
                            expr_ident("+"),
                            expr_int(8),
                            expr_ident("pl_len".to_string()),
                        ]),
                        expr_ident("doc_len".to_string()),
                    ]),
                    expr_list(vec![
                        expr_ident("return"),
                        err_doc_const(E_NET_READ_DOC_INVALID, "stream:net_read_doc_invalid"),
                    ]),
                    expr_int(0),
                ]),
                // EOF
                expr_list(vec![
                    expr_ident("if"),
                    expr_list(vec![
                        expr_ident("="),
                        expr_ident("pl_len".to_string()),
                        expr_int(0),
                    ]),
                    end_ok_with_flush.clone(),
                    expr_int(0),
                ]),
                expr_list(vec![
                    expr_ident("if"),
                    expr_list(vec![
                        expr_ident(">u"),
                        expr_ident("pl_len".to_string()),
                        expr_ident(read_max_var.clone()),
                    ]),
                    expr_list(vec![
                        expr_ident("return"),
                        err_doc_const(E_NET_BACKEND_OVERRAN_MAX, "stream:net_backend_overran_max"),
                    ]),
                    expr_int(0),
                ]),
                expr_list(vec![
                    expr_ident("let"),
                    expr_ident("chunk".to_string()),
                    expr_list(vec![
                        expr_ident("view.slice"),
                        expr_ident("dv".to_string()),
                        expr_int(8),
                        expr_ident("pl_len".to_string()),
                    ]),
                ]),
                set_add_i32(&self.bytes_in_var, expr_ident("pl_len".to_string())),
                set_add_i32(&self.items_in_var, expr_int(1)),
                self.budget_check_in()?,
                self.gen_process_from(0, expr_ident("chunk".to_string()))?,
            ]),
        ]));
        if let Some(stop) = &self.stop_var {
            loop_body.push(expr_list(vec![
                expr_ident("if"),
                expr_list(vec![expr_ident("="), expr_ident(stop.clone()), expr_int(1)]),
                end_ok_no_flush,
                expr_int(0),
            ]));
        }
        loop_body.push(expr_int(0));

        stmts.push(expr_list(vec![
            expr_ident("for"),
            expr_ident("step".to_string()),
            expr_int(0),
            expr_int(self.max_steps),
            expr_list(
                vec![expr_ident("begin")]
                    .into_iter()
                    .chain(loop_body)
                    .collect(),
            ),
        ]));
        stmts.push(expr_list(vec![
            expr_ident("return"),
            err_doc_const(E_CFG_INVALID, "stream:max_steps_exceeded"),
        ]));

        Ok(expr_list(
            vec![expr_ident("begin")].into_iter().chain(stmts).collect(),
        ))
    }

    fn gen_reader_loop_body(&self) -> Result<Expr, CompilerError> {
        let mut body = Vec::new();

        body.push(expr_list(vec![
            expr_ident("let"),
            expr_ident("chunk0".to_string()),
            expr_list(vec![
                expr_ident("bufread.fill"),
                expr_ident("br".to_string()),
            ]),
        ]));
        body.push(expr_list(vec![
            expr_ident("let"),
            expr_ident("n0".to_string()),
            expr_list(vec![
                expr_ident("view.len"),
                expr_ident("chunk0".to_string()),
            ]),
        ]));
        body.push(expr_list(vec![
            expr_ident("let"),
            expr_ident("n".to_string()),
            expr_list(vec![
                expr_ident("if"),
                expr_list(vec![
                    expr_ident(">u"),
                    expr_ident("n0".to_string()),
                    expr_int(self.cfg.chunk_max_bytes),
                ]),
                expr_int(self.cfg.chunk_max_bytes),
                expr_ident("n0".to_string()),
            ]),
        ]));

        // EOF: flush and return OK.
        let mut eof_stmts = Vec::new();
        if self.has_flush_stages() {
            eof_stmts.push(self.gen_flush_from(0)?);
        }
        eof_stmts.push(self.gen_return_ok()?);
        body.push(expr_list(vec![
            expr_ident("if"),
            expr_list(vec![
                expr_ident("="),
                expr_ident("n".to_string()),
                expr_int(0),
            ]),
            expr_list(
                vec![expr_ident("begin")]
                    .into_iter()
                    .chain(eof_stmts)
                    .collect(),
            ),
            expr_int(0),
        ]));

        body.push(expr_list(vec![
            expr_ident("let"),
            expr_ident("chunk".to_string()),
            expr_list(vec![
                expr_ident("view.slice"),
                expr_ident("chunk0".to_string()),
                expr_int(0),
                expr_ident("n".to_string()),
            ]),
        ]));

        body.push(set_add_i32(&self.bytes_in_var, expr_ident("n".to_string())));
        body.push(set_add_i32(&self.items_in_var, expr_int(1)));
        body.push(self.budget_check_in()?);

        body.push(self.gen_process_from(0, expr_ident("chunk".to_string()))?);

        body.push(expr_list(vec![
            expr_ident("bufread.consume"),
            expr_ident("br".to_string()),
            expr_ident("n".to_string()),
        ]));

        if let Some(stop) = &self.stop_var {
            body.push(expr_list(vec![
                expr_ident("if"),
                expr_list(vec![expr_ident("="), expr_ident(stop.clone()), expr_int(1)]),
                self.gen_return_ok()?,
                expr_int(0),
            ]));
        }

        Ok(expr_list(
            vec![expr_ident("begin")].into_iter().chain(body).collect(),
        ))
    }

    fn budget_check_in(&self) -> Result<Expr, CompilerError> {
        Ok(expr_list(vec![
            expr_ident("if"),
            expr_list(vec![
                expr_ident(">u"),
                expr_ident(self.bytes_in_var.clone()),
                expr_int(self.cfg.max_in_bytes),
            ]),
            expr_list(vec![
                expr_ident("return"),
                err_doc_const(E_BUDGET_IN_BYTES, "stream:budget_in_bytes"),
            ]),
            expr_int(0),
        ]))
    }

    fn gen_process_from(&self, stage_idx: usize, item: Expr) -> Result<Expr, CompilerError> {
        if stage_idx >= self.chain.len() {
            return self.emit_item(item);
        }
        match &self.chain[stage_idx] {
            PipeXfV1::MapBytes { fn_id } => {
                let mapped_b = format!("mapped_{stage_idx}");
                let mapped_v = format!("mappedv_{stage_idx}");
                Ok(expr_list(vec![
                    expr_ident("begin"),
                    expr_list(vec![
                        expr_ident("let"),
                        expr_ident(mapped_b.clone()),
                        expr_list(vec![expr_ident(fn_id.clone()), item]),
                    ]),
                    expr_list(vec![
                        expr_ident("let"),
                        expr_ident(mapped_v.clone()),
                        expr_list(vec![expr_ident("bytes.view"), expr_ident(mapped_b)]),
                    ]),
                    self.gen_process_from(stage_idx + 1, expr_ident(mapped_v))?,
                ]))
            }
            PipeXfV1::Filter { fn_id } => {
                let keep = format!("keep_{stage_idx}");
                Ok(expr_list(vec![
                    expr_ident("begin"),
                    expr_list(vec![
                        expr_ident("let"),
                        expr_ident(keep.clone()),
                        expr_list(vec![expr_ident(fn_id.clone()), item.clone()]),
                    ]),
                    expr_list(vec![
                        expr_ident("if"),
                        expr_list(vec![expr_ident("="), expr_ident(keep), expr_int(0)]),
                        expr_int(0),
                        self.gen_process_from(stage_idx + 1, item)?,
                    ]),
                ]))
            }
            PipeXfV1::Take { .. } => {
                let t = self
                    .take_states
                    .iter()
                    .find(|t| t.stage_idx == stage_idx)
                    .ok_or_else(|| {
                        CompilerError::new(
                            CompileErrorKind::Internal,
                            "internal error: missing take state".to_string(),
                        )
                    })?;
                let stop = self.stop_var.as_ref().ok_or_else(|| {
                    CompilerError::new(
                        CompileErrorKind::Internal,
                        "internal error: take stage without stop var".to_string(),
                    )
                })?;
                Ok(expr_list(vec![
                    expr_ident("if"),
                    expr_list(vec![
                        expr_ident("<="),
                        expr_ident(t.rem_var.clone()),
                        expr_int(0),
                    ]),
                    expr_list(vec![
                        expr_ident("begin"),
                        expr_list(vec![
                            expr_ident("set"),
                            expr_ident(stop.clone()),
                            expr_int(1),
                        ]),
                        expr_int(0),
                    ]),
                    expr_list(vec![
                        expr_ident("begin"),
                        expr_list(vec![
                            expr_ident("set"),
                            expr_ident(t.rem_var.clone()),
                            expr_list(vec![
                                expr_ident("-"),
                                expr_ident(t.rem_var.clone()),
                                expr_int(1),
                            ]),
                        ]),
                        self.gen_process_from(stage_idx + 1, item)?,
                    ]),
                ]))
            }
            PipeXfV1::FrameU32Le => {
                let n_var = format!("frame_n_{stage_idx}");
                let hdr_var = format!("frame_hdr_{stage_idx}");
                let out_var = format!("frame_out_{stage_idx}");
                let bytes_var = format!("frame_b_{stage_idx}");
                let view_var = format!("frame_v_{stage_idx}");
                Ok(expr_list(vec![
                    expr_ident("begin"),
                    expr_list(vec![
                        expr_ident("let"),
                        expr_ident(n_var.clone()),
                        expr_list(vec![expr_ident("view.len"), item.clone()]),
                    ]),
                    expr_list(vec![
                        expr_ident("if"),
                        expr_list(vec![
                            expr_ident("<"),
                            expr_ident(n_var.clone()),
                            expr_int(0),
                        ]),
                        expr_list(vec![
                            expr_ident("return"),
                            err_doc_const(E_FRAME_TOO_LARGE, "stream:frame_too_large"),
                        ]),
                        expr_int(0),
                    ]),
                    expr_list(vec![
                        expr_ident("let"),
                        expr_ident(hdr_var.clone()),
                        expr_list(vec![
                            expr_ident("codec.write_u32_le"),
                            expr_ident(n_var.clone()),
                        ]),
                    ]),
                    expr_list(vec![
                        expr_ident("let"),
                        expr_ident(out_var.clone()),
                        expr_list(vec![
                            expr_ident("vec_u8.with_capacity"),
                            expr_list(vec![
                                expr_ident("+"),
                                expr_int(4),
                                expr_ident(n_var.clone()),
                            ]),
                        ]),
                    ]),
                    expr_list(vec![
                        expr_ident("set"),
                        expr_ident(out_var.clone()),
                        expr_list(vec![
                            expr_ident("vec_u8.extend_bytes"),
                            expr_ident(out_var.clone()),
                            expr_ident(hdr_var.clone()),
                        ]),
                    ]),
                    expr_list(vec![
                        expr_ident("set"),
                        expr_ident(out_var.clone()),
                        expr_list(vec![
                            expr_ident("vec_u8.extend_bytes_range"),
                            expr_ident(out_var.clone()),
                            item,
                            expr_int(0),
                            expr_ident(n_var.clone()),
                        ]),
                    ]),
                    expr_list(vec![
                        expr_ident("let"),
                        expr_ident(bytes_var.clone()),
                        expr_list(vec![expr_ident("vec_u8.into_bytes"), expr_ident(out_var)]),
                    ]),
                    expr_list(vec![
                        expr_ident("let"),
                        expr_ident(view_var.clone()),
                        expr_list(vec![expr_ident("bytes.view"), expr_ident(bytes_var)]),
                    ]),
                    self.gen_process_from(stage_idx + 1, expr_ident(view_var))?,
                ]))
            }
            PipeXfV1::MapInPlaceBufV1 { .. } => self.gen_map_in_place_buf(stage_idx, item),
            PipeXfV1::SplitLines { .. } => self.gen_split_lines(stage_idx, item),
            PipeXfV1::DeframeU32LeV1 { .. } => self.gen_deframe_u32le(stage_idx, item),
            PipeXfV1::JsonCanonStreamV1 { .. } => {
                self.gen_json_canon_stream_process(stage_idx, item)
            }
        }
    }

    fn gen_map_in_place_buf(&self, stage_idx: usize, item: Expr) -> Result<Expr, CompilerError> {
        let s = self
            .map_in_place_states
            .iter()
            .find(|s| s.stage_idx == stage_idx)
            .ok_or_else(|| {
                CompilerError::new(
                    CompileErrorKind::Internal,
                    "internal error: missing map_in_place state".to_string(),
                )
            })?;

        let scratch = expr_ident(s.scratch_var.clone());

        let mut stmts = Vec::new();
        if s.clear_before_each != 0 {
            stmts.push(expr_list(vec![
                expr_ident("set"),
                scratch.clone(),
                expr_list(vec![
                    expr_ident("scratch_u8_fixed_v1.clear"),
                    scratch.clone(),
                ]),
            ]));
        }

        stmts.push(expr_list(vec![
            expr_ident("let"),
            expr_ident("r".to_string()),
            expr_list(vec![expr_ident(s.fn_id.clone()), item, scratch.clone()]),
        ]));

        stmts.push(expr_list(vec![
            expr_ident("if"),
            expr_list(vec![
                expr_ident("result_i32.is_ok"),
                expr_ident("r".to_string()),
            ]),
            expr_list(vec![
                expr_ident("begin"),
                expr_list(vec![
                    expr_ident("let"),
                    expr_ident("out".to_string()),
                    expr_list(vec![
                        expr_ident("scratch_u8_fixed_v1.as_view"),
                        scratch.clone(),
                    ]),
                ]),
                self.gen_process_from(stage_idx + 1, expr_ident("out".to_string()))?,
            ]),
            expr_list(vec![
                expr_ident("begin"),
                expr_list(vec![
                    expr_ident("let"),
                    expr_ident("ec".to_string()),
                    expr_list(vec![
                        expr_ident("result_i32.err_code"),
                        expr_ident("r".to_string()),
                    ]),
                ]),
                expr_list(vec![
                    expr_ident("if"),
                    expr_list(vec![
                        expr_ident("="),
                        expr_ident("ec".to_string()),
                        expr_int(E_SCRATCH_OVERFLOW),
                    ]),
                    expr_list(vec![
                        expr_ident("return"),
                        err_doc_const(E_SCRATCH_OVERFLOW, "stream:scratch_overflow"),
                    ]),
                    expr_int(0),
                ]),
                expr_list(vec![
                    expr_ident("let"),
                    expr_ident("pl".to_string()),
                    expr_list(vec![expr_ident("vec_u8.with_capacity"), expr_int(16)]),
                ]),
                extend_u32("pl", expr_int(i32::try_from(stage_idx).unwrap_or(i32::MAX))),
                extend_u32("pl", expr_ident(self.bytes_in_var.clone())),
                extend_u32("pl", expr_ident(self.items_in_var.clone())),
                extend_u32("pl", expr_ident("ec".to_string())),
                expr_list(vec![
                    expr_ident("return"),
                    err_doc_with_payload(
                        expr_int(E_STAGE_FAILED),
                        "stream:stage_failed",
                        expr_list(vec![
                            expr_ident("vec_u8.into_bytes"),
                            expr_ident("pl".to_string()),
                        ]),
                    ),
                ]),
            ]),
        ]));

        Ok(expr_list(
            vec![expr_ident("begin")].into_iter().chain(stmts).collect(),
        ))
    }

    fn gen_split_lines(&self, stage_idx: usize, chunk: Expr) -> Result<Expr, CompilerError> {
        let s = self
            .split_states
            .iter()
            .find(|s| s.stage_idx == stage_idx)
            .ok_or_else(|| {
                CompilerError::new(
                    CompileErrorKind::Internal,
                    "internal error: missing split_lines state".to_string(),
                )
            })?;

        let delim = param_ident(s.delim_param);
        let max_line = param_ident(s.max_line_bytes_param);
        let carry = expr_ident(s.carry_var.clone());

        let mut stmts = vec![expr_list(vec![
            expr_ident("let"),
            expr_ident("chunk_len".to_string()),
            expr_list(vec![expr_ident("view.len"), chunk.clone()]),
        ])];
        stmts.push(let_i32("start", 0));

        // Scan for delimiters.
        stmts.push(expr_list(vec![
            expr_ident("for"),
            expr_ident("i".to_string()),
            expr_int(0),
            expr_ident("chunk_len".to_string()),
            expr_list(vec![
                expr_ident("begin"),
                expr_list(vec![
                    expr_ident("if"),
                    expr_list(vec![
                        expr_ident("="),
                        expr_list(vec![
                            expr_ident("view.get_u8"),
                            chunk.clone(),
                            expr_ident("i".to_string()),
                        ]),
                        delim.clone(),
                    ]),
                    expr_list(vec![
                        expr_ident("begin"),
                        expr_list(vec![
                            expr_ident("let"),
                            expr_ident("seg_len".to_string()),
                            expr_list(vec![
                                expr_ident("-"),
                                expr_ident("i".to_string()),
                                expr_ident("start".to_string()),
                            ]),
                        ]),
                        // If carry_len + seg_len > max_line -> error.
                        expr_list(vec![
                            expr_ident("if"),
                            expr_list(vec![
                                expr_ident(">u"),
                                expr_list(vec![
                                    expr_ident("+"),
                                    expr_list(vec![expr_ident("vec_u8.len"), carry.clone()]),
                                    expr_ident("seg_len".to_string()),
                                ]),
                                max_line.clone(),
                            ]),
                            expr_list(vec![
                                expr_ident("return"),
                                err_doc_const(E_LINE_TOO_LONG, "stream:line_too_long"),
                            ]),
                            expr_int(0),
                        ]),
                        // If carry empty, emit view.slice directly; else extend carry and emit bytes.
                        expr_list(vec![
                            expr_ident("if"),
                            expr_list(vec![
                                expr_ident("="),
                                expr_list(vec![expr_ident("vec_u8.len"), carry.clone()]),
                                expr_int(0),
                            ]),
                            self.gen_process_from(
                                stage_idx + 1,
                                expr_list(vec![
                                    expr_ident("view.slice"),
                                    chunk.clone(),
                                    expr_ident("start".to_string()),
                                    expr_ident("seg_len".to_string()),
                                ]),
                            )?,
                            expr_list(vec![
                                expr_ident("begin"),
                                expr_list(vec![
                                    expr_ident("set"),
                                    carry.clone(),
                                    expr_list(vec![
                                        expr_ident("vec_u8.extend_bytes_range"),
                                        carry.clone(),
                                        chunk.clone(),
                                        expr_ident("start".to_string()),
                                        expr_ident("seg_len".to_string()),
                                    ]),
                                ]),
                                expr_list(vec![
                                    expr_ident("let"),
                                    expr_ident("line_b".to_string()),
                                    expr_list(vec![expr_ident("vec_u8.into_bytes"), carry.clone()]),
                                ]),
                                expr_list(vec![
                                    expr_ident("let"),
                                    expr_ident("line_v".to_string()),
                                    expr_list(vec![
                                        expr_ident("bytes.view"),
                                        expr_ident("line_b".to_string()),
                                    ]),
                                ]),
                                self.gen_process_from(
                                    stage_idx + 1,
                                    expr_ident("line_v".to_string()),
                                )?,
                                expr_list(vec![
                                    expr_ident("set"),
                                    carry.clone(),
                                    expr_list(vec![
                                        expr_ident("vec_u8.with_capacity"),
                                        max_line.clone(),
                                    ]),
                                ]),
                                expr_int(0),
                            ]),
                        ]),
                        expr_list(vec![
                            expr_ident("set"),
                            expr_ident("start".to_string()),
                            expr_list(vec![
                                expr_ident("+"),
                                expr_ident("i".to_string()),
                                expr_int(1),
                            ]),
                        ]),
                        expr_int(0),
                    ]),
                    expr_int(0),
                ]),
            ]),
        ]));

        // Append tail bytes (after last delimiter) to carry.
        stmts.push(expr_list(vec![
            expr_ident("let"),
            expr_ident("tail_len".to_string()),
            expr_list(vec![
                expr_ident("-"),
                expr_ident("chunk_len".to_string()),
                expr_ident("start".to_string()),
            ]),
        ]));
        stmts.push(expr_list(vec![
            expr_ident("if"),
            expr_list(vec![
                expr_ident(">u"),
                expr_ident("tail_len".to_string()),
                expr_int(0),
            ]),
            expr_list(vec![
                expr_ident("begin"),
                expr_list(vec![
                    expr_ident("if"),
                    expr_list(vec![
                        expr_ident(">u"),
                        expr_list(vec![
                            expr_ident("+"),
                            expr_list(vec![expr_ident("vec_u8.len"), carry.clone()]),
                            expr_ident("tail_len".to_string()),
                        ]),
                        max_line.clone(),
                    ]),
                    expr_list(vec![
                        expr_ident("return"),
                        err_doc_const(E_LINE_TOO_LONG, "stream:line_too_long"),
                    ]),
                    expr_int(0),
                ]),
                expr_list(vec![
                    expr_ident("set"),
                    carry.clone(),
                    expr_list(vec![
                        expr_ident("vec_u8.extend_bytes_range"),
                        carry.clone(),
                        chunk,
                        expr_ident("start".to_string()),
                        expr_ident("tail_len".to_string()),
                    ]),
                ]),
                expr_int(0),
            ]),
            expr_int(0),
        ]));

        Ok(expr_list(
            vec![expr_ident("begin")].into_iter().chain(stmts).collect(),
        ))
    }

    fn gen_deframe_u32le(&self, stage_idx: usize, chunk: Expr) -> Result<Expr, CompilerError> {
        let d = self
            .deframe_states
            .iter()
            .find(|d| d.stage_idx == stage_idx)
            .ok_or_else(|| {
                CompilerError::new(
                    CompileErrorKind::Internal,
                    "internal error: missing deframe state".to_string(),
                )
            })?;

        let cfg = &d.cfg;
        let max_frame_bytes = expr_int(cfg.max_frame_bytes);
        let max_frames = cfg.max_frames;
        let allow_empty = cfg.allow_empty != 0;

        let hdr = expr_ident(d.hdr_var.clone());
        let hdr_fill = expr_ident(d.hdr_fill_var.clone());
        let need = expr_ident(d.need_var.clone());
        let buf = expr_ident(d.buf_var.clone());
        let buf_fill = expr_ident(d.buf_fill_var.clone());
        let frames = expr_ident(d.frames_var.clone());

        let chunk_len_var = format!("deframe_chunk_len_{stage_idx}");
        let new_frames_var = format!("deframe_new_frames_{stage_idx}");

        let mut stmts = Vec::new();
        stmts.push(expr_list(vec![
            expr_ident("let"),
            expr_ident(chunk_len_var.clone()),
            expr_list(vec![expr_ident("view.len"), chunk.clone()]),
        ]));

        // Process byte-by-byte to keep lowering small (avoids max-locals blowups).
        let mut loop_body = Vec::new();
        loop_body.push(expr_list(vec![
            expr_ident("if"),
            expr_list(vec![expr_ident("<"), hdr_fill.clone(), expr_int(4)]),
            // READ_HDR
            expr_list(vec![
                expr_ident("begin"),
                expr_list(vec![
                    expr_ident("set"),
                    hdr.clone(),
                    expr_list(vec![
                        expr_ident("vec_u8.set"),
                        hdr.clone(),
                        hdr_fill.clone(),
                        expr_list(vec![
                            expr_ident("view.get_u8"),
                            chunk.clone(),
                            expr_ident("i".to_string()),
                        ]),
                    ]),
                ]),
                expr_list(vec![
                    expr_ident("set"),
                    hdr_fill.clone(),
                    expr_list(vec![expr_ident("+"), hdr_fill.clone(), expr_int(1)]),
                ]),
                expr_list(vec![
                    expr_ident("if"),
                    expr_list(vec![expr_ident("="), hdr_fill.clone(), expr_int(4)]),
                    expr_list(vec![
                        expr_ident("begin"),
                        expr_list(vec![
                            expr_ident("set"),
                            need.clone(),
                            expr_list(vec![
                                expr_ident("codec.read_u32_le"),
                                expr_list(vec![expr_ident("vec_u8.as_view"), hdr.clone()]),
                                expr_int(0),
                            ]),
                        ]),
                        expr_list(vec![
                            expr_ident("if"),
                            expr_list(vec![expr_ident("<"), need.clone(), expr_int(0)]),
                            expr_list(vec![
                                expr_ident("return"),
                                err_doc_const(
                                    E_DEFRAME_FRAME_TOO_LARGE,
                                    "stream:deframe_frame_too_large",
                                ),
                            ]),
                            expr_int(0),
                        ]),
                        expr_list(vec![
                            expr_ident("if"),
                            expr_list(vec![
                                expr_ident(">u"),
                                need.clone(),
                                max_frame_bytes.clone(),
                            ]),
                            expr_list(vec![
                                expr_ident("return"),
                                err_doc_const(
                                    E_DEFRAME_FRAME_TOO_LARGE,
                                    "stream:deframe_frame_too_large",
                                ),
                            ]),
                            expr_int(0),
                        ]),
                        expr_list(vec![
                            expr_ident("if"),
                            expr_list(vec![expr_ident("="), need.clone(), expr_int(0)]),
                            if allow_empty {
                                expr_list(vec![
                                    expr_ident("begin"),
                                    self.gen_deframe_emit_frame(
                                        stage_idx,
                                        expr_list(vec![
                                            expr_ident("view.slice"),
                                            chunk.clone(),
                                            expr_ident("i".to_string()),
                                            expr_int(0),
                                        ]),
                                        max_frames,
                                        &new_frames_var,
                                        &frames,
                                    )?,
                                    expr_list(vec![
                                        expr_ident("set"),
                                        hdr_fill.clone(),
                                        expr_int(0),
                                    ]),
                                    expr_list(vec![expr_ident("set"), need.clone(), expr_int(0)]),
                                    expr_list(vec![
                                        expr_ident("set"),
                                        buf_fill.clone(),
                                        expr_int(0),
                                    ]),
                                    expr_int(0),
                                ])
                            } else {
                                expr_list(vec![
                                    expr_ident("return"),
                                    err_doc_const(
                                        E_DEFRAME_EMPTY_FORBIDDEN,
                                        "stream:deframe_empty_forbidden",
                                    ),
                                ])
                            },
                            expr_int(0),
                        ]),
                        expr_list(vec![expr_ident("set"), buf_fill.clone(), expr_int(0)]),
                        expr_int(0),
                    ]),
                    expr_int(0),
                ]),
                expr_int(0),
            ]),
            // READ_PAYLOAD
            expr_list(vec![
                expr_ident("begin"),
                expr_list(vec![
                    expr_ident("set"),
                    buf.clone(),
                    expr_list(vec![
                        expr_ident("vec_u8.set"),
                        buf.clone(),
                        buf_fill.clone(),
                        expr_list(vec![
                            expr_ident("view.get_u8"),
                            chunk.clone(),
                            expr_ident("i".to_string()),
                        ]),
                    ]),
                ]),
                expr_list(vec![
                    expr_ident("set"),
                    buf_fill.clone(),
                    expr_list(vec![expr_ident("+"), buf_fill.clone(), expr_int(1)]),
                ]),
                expr_list(vec![
                    expr_ident("if"),
                    expr_list(vec![expr_ident("="), buf_fill.clone(), need.clone()]),
                    expr_list(vec![
                        expr_ident("begin"),
                        expr_list(vec![
                            expr_ident("let"),
                            expr_ident(format!("deframe_bv_{stage_idx}")),
                            expr_list(vec![expr_ident("vec_u8.as_view"), buf.clone()]),
                        ]),
                        expr_list(vec![
                            expr_ident("let"),
                            expr_ident(format!("deframe_payload_{stage_idx}")),
                            expr_list(vec![
                                expr_ident("view.slice"),
                                expr_ident(format!("deframe_bv_{stage_idx}")),
                                expr_int(0),
                                need.clone(),
                            ]),
                        ]),
                        self.gen_deframe_emit_frame(
                            stage_idx,
                            expr_ident(format!("deframe_payload_{stage_idx}")),
                            max_frames,
                            &new_frames_var,
                            &frames,
                        )?,
                        expr_list(vec![expr_ident("set"), hdr_fill.clone(), expr_int(0)]),
                        expr_list(vec![expr_ident("set"), need.clone(), expr_int(0)]),
                        expr_list(vec![expr_ident("set"), buf_fill.clone(), expr_int(0)]),
                        expr_int(0),
                    ]),
                    expr_int(0),
                ]),
                expr_int(0),
            ]),
        ]));
        loop_body.push(expr_int(0));

        stmts.push(expr_list(vec![
            expr_ident("for"),
            expr_ident("i".to_string()),
            expr_int(0),
            expr_ident(chunk_len_var),
            expr_list(
                vec![expr_ident("begin")]
                    .into_iter()
                    .chain(loop_body)
                    .collect(),
            ),
        ]));
        stmts.push(expr_int(0));

        Ok(expr_list(
            vec![expr_ident("begin")].into_iter().chain(stmts).collect(),
        ))
    }

    fn gen_json_canon_stream_process(
        &self,
        stage_idx: usize,
        item: Expr,
    ) -> Result<Expr, CompilerError> {
        let j = self
            .json_canon_states
            .iter()
            .find(|j| j.stage_idx == stage_idx)
            .ok_or_else(|| {
                CompilerError::new(
                    CompileErrorKind::Internal,
                    "internal error: missing json_canon_stream state".to_string(),
                )
            })?;

        let buf = expr_ident(j.buf_var.clone());
        let max_total = expr_int(j.cfg.max_total_json_bytes);

        let mut stmts: Vec<Expr> = vec![expr_list(vec![
            expr_ident("let"),
            expr_ident("json_n".to_string()),
            expr_list(vec![expr_ident("view.len"), item.clone()]),
        ])];
        stmts.push(expr_list(vec![
            expr_ident("if"),
            expr_list(vec![
                expr_ident("<"),
                expr_ident("json_n".to_string()),
                expr_int(0),
            ]),
            expr_list(vec![
                expr_ident("return"),
                err_doc_const(E_BUDGET_IN_BYTES, "stream:json_input_too_large"),
            ]),
            expr_int(0),
        ]));
        stmts.push(expr_list(vec![
            expr_ident("let"),
            expr_ident("json_new_len".to_string()),
            expr_list(vec![
                expr_ident("+"),
                expr_list(vec![expr_ident("vec_u8.len"), buf.clone()]),
                expr_ident("json_n".to_string()),
            ]),
        ]));
        stmts.push(expr_list(vec![
            expr_ident("if"),
            expr_list(vec![
                expr_ident(">u"),
                expr_ident("json_new_len".to_string()),
                max_total,
            ]),
            expr_list(vec![
                expr_ident("return"),
                err_doc_const(E_BUDGET_IN_BYTES, "stream:json_max_total_json_bytes"),
            ]),
            expr_int(0),
        ]));
        stmts.push(expr_list(vec![
            expr_ident("set"),
            buf.clone(),
            expr_list(vec![
                expr_ident("vec_u8.extend_bytes_range"),
                buf.clone(),
                item,
                expr_int(0),
                expr_ident("json_n".to_string()),
            ]),
        ]));
        stmts.push(expr_int(0));
        Ok(expr_list(
            vec![expr_ident("begin")].into_iter().chain(stmts).collect(),
        ))
    }

    fn gen_json_canon_stream_flush(&self, stage_idx: usize) -> Result<Expr, CompilerError> {
        let j = self
            .json_canon_states
            .iter()
            .find(|j| j.stage_idx == stage_idx)
            .ok_or_else(|| {
                CompilerError::new(
                    CompileErrorKind::Internal,
                    "internal error: missing json_canon_stream state".to_string(),
                )
            })?;

        let buf = expr_ident(j.buf_var.clone());
        let cfg_max_depth = expr_int(j.cfg.max_depth);
        let cfg_max_object_members = expr_int(j.cfg.max_object_members);
        let cfg_max_object_total_bytes = expr_int(j.cfg.max_object_total_bytes);
        let cfg_emit_chunk_max_bytes = expr_int(j.cfg.emit_chunk_max_bytes);

        let stage_idx_i32 = i32::try_from(stage_idx).unwrap_or(i32::MAX);

        let mut stmts: Vec<Expr> = vec![expr_list(vec![
            expr_ident("let"),
            expr_ident("json_in".to_string()),
            expr_list(vec![expr_ident("vec_u8.as_view"), buf.clone()]),
        ])];
        stmts.push(expr_list(vec![
            expr_ident("let"),
            expr_ident("json_doc".to_string()),
            expr_list(vec![
                expr_ident("json.jcs.canon_doc_v1"),
                expr_ident("json_in".to_string()),
                cfg_max_depth,
                cfg_max_object_members,
                cfg_max_object_total_bytes,
            ]),
        ]));
        stmts.push(expr_list(vec![
            expr_ident("let"),
            expr_ident("json_dv".to_string()),
            expr_list(vec![
                expr_ident("bytes.view"),
                expr_ident("json_doc".to_string()),
            ]),
        ]));
        stmts.push(expr_list(vec![
            expr_ident("let"),
            expr_ident("json_dl".to_string()),
            expr_list(vec![
                expr_ident("view.len"),
                expr_ident("json_dv".to_string()),
            ]),
        ]));
        stmts.push(expr_list(vec![
            expr_ident("if"),
            expr_list(vec![
                expr_ident("<"),
                expr_ident("json_dl".to_string()),
                expr_int(1),
            ]),
            expr_list(vec![
                expr_ident("return"),
                err_doc_const(E_CFG_INVALID, "stream:json_doc_invalid"),
            ]),
            expr_int(0),
        ]));
        stmts.push(expr_list(vec![
            expr_ident("let"),
            expr_ident("json_tag".to_string()),
            expr_list(vec![
                expr_ident("view.get_u8"),
                expr_ident("json_dv".to_string()),
                expr_int(0),
            ]),
        ]));

        stmts.push(expr_list(vec![
            expr_ident("if"),
            expr_list(vec![
                expr_ident("="),
                expr_ident("json_tag".to_string()),
                expr_int(0),
            ]),
            expr_list(vec![
                expr_ident("begin"),
                expr_list(vec![
                    expr_ident("if"),
                    expr_list(vec![
                        expr_ident("<"),
                        expr_ident("json_dl".to_string()),
                        expr_int(9),
                    ]),
                    expr_list(vec![
                        expr_ident("return"),
                        err_doc_const(E_CFG_INVALID, "stream:json_doc_invalid"),
                    ]),
                    expr_int(0),
                ]),
                expr_list(vec![
                    expr_ident("let"),
                    expr_ident("json_code".to_string()),
                    expr_list(vec![
                        expr_ident("codec.read_u32_le"),
                        expr_ident("json_dv".to_string()),
                        expr_int(1),
                    ]),
                ]),
                expr_list(vec![
                    expr_ident("let"),
                    expr_ident("json_off".to_string()),
                    expr_list(vec![
                        expr_ident("codec.read_u32_le"),
                        expr_ident("json_dv".to_string()),
                        expr_int(5),
                    ]),
                ]),
                expr_list(vec![
                    expr_ident("let"),
                    expr_ident("pl".to_string()),
                    expr_list(vec![expr_ident("vec_u8.with_capacity"), expr_int(8)]),
                ]),
                extend_u32("pl", expr_ident("json_off".to_string())),
                extend_u32("pl", expr_int(stage_idx_i32)),
                expr_list(vec![
                    expr_ident("let"),
                    expr_ident("plb".to_string()),
                    expr_list(vec![
                        expr_ident("vec_u8.into_bytes"),
                        expr_ident("pl".to_string()),
                    ]),
                ]),
                expr_list(vec![
                    expr_ident("if"),
                    expr_list(vec![
                        expr_ident("="),
                        expr_ident("json_code".to_string()),
                        expr_int(E_JSON_SYNTAX),
                    ]),
                    expr_list(vec![
                        expr_ident("return"),
                        err_doc_with_payload(
                            expr_int(E_JSON_SYNTAX),
                            "stream:json_syntax",
                            expr_ident("plb".to_string()),
                        ),
                    ]),
                    expr_int(0),
                ]),
                expr_list(vec![
                    expr_ident("if"),
                    expr_list(vec![
                        expr_ident("="),
                        expr_ident("json_code".to_string()),
                        expr_int(E_JSON_NOT_IJSON),
                    ]),
                    expr_list(vec![
                        expr_ident("return"),
                        err_doc_with_payload(
                            expr_int(E_JSON_NOT_IJSON),
                            "stream:json_not_ijson",
                            expr_ident("plb".to_string()),
                        ),
                    ]),
                    expr_int(0),
                ]),
                expr_list(vec![
                    expr_ident("if"),
                    expr_list(vec![
                        expr_ident("="),
                        expr_ident("json_code".to_string()),
                        expr_int(E_JSON_TOO_DEEP),
                    ]),
                    expr_list(vec![
                        expr_ident("return"),
                        err_doc_with_payload(
                            expr_int(E_JSON_TOO_DEEP),
                            "stream:json_too_deep",
                            expr_ident("plb".to_string()),
                        ),
                    ]),
                    expr_int(0),
                ]),
                expr_list(vec![
                    expr_ident("if"),
                    expr_list(vec![
                        expr_ident("="),
                        expr_ident("json_code".to_string()),
                        expr_int(E_JSON_OBJECT_TOO_LARGE),
                    ]),
                    expr_list(vec![
                        expr_ident("return"),
                        err_doc_with_payload(
                            expr_int(E_JSON_OBJECT_TOO_LARGE),
                            "stream:json_object_too_large",
                            expr_ident("plb".to_string()),
                        ),
                    ]),
                    expr_int(0),
                ]),
                expr_list(vec![
                    expr_ident("if"),
                    expr_list(vec![
                        expr_ident("="),
                        expr_ident("json_code".to_string()),
                        expr_int(E_JSON_TRAILING_DATA),
                    ]),
                    expr_list(vec![
                        expr_ident("return"),
                        err_doc_with_payload(
                            expr_int(E_JSON_TRAILING_DATA),
                            "stream:json_trailing_data",
                            expr_ident("plb".to_string()),
                        ),
                    ]),
                    expr_int(0),
                ]),
                expr_list(vec![
                    expr_ident("return"),
                    err_doc_const(E_CFG_INVALID, "stream:json_doc_invalid"),
                ]),
            ]),
            expr_int(0),
        ]));

        stmts.push(expr_list(vec![
            expr_ident("if"),
            expr_list(vec![
                expr_ident("!="),
                expr_ident("json_tag".to_string()),
                expr_int(1),
            ]),
            expr_list(vec![
                expr_ident("return"),
                err_doc_const(E_CFG_INVALID, "stream:json_doc_invalid"),
            ]),
            expr_int(0),
        ]));

        stmts.push(expr_list(vec![
            expr_ident("let"),
            expr_ident("canon_len".to_string()),
            expr_list(vec![
                expr_ident("-"),
                expr_ident("json_dl".to_string()),
                expr_int(1),
            ]),
        ]));
        stmts.push(expr_list(vec![
            expr_ident("let"),
            expr_ident("canon".to_string()),
            expr_list(vec![
                expr_ident("view.slice"),
                expr_ident("json_dv".to_string()),
                expr_int(1),
                expr_ident("canon_len".to_string()),
            ]),
        ]));
        stmts.push(expr_list(vec![
            expr_ident("let"),
            expr_ident("chunks".to_string()),
            expr_list(vec![
                expr_ident("/"),
                expr_list(vec![
                    expr_ident("+"),
                    expr_ident("canon_len".to_string()),
                    expr_list(vec![
                        expr_ident("-"),
                        cfg_emit_chunk_max_bytes.clone(),
                        expr_int(1),
                    ]),
                ]),
                cfg_emit_chunk_max_bytes.clone(),
            ]),
        ]));

        stmts.push(expr_list(vec![
            expr_ident("for"),
            expr_ident("i".to_string()),
            expr_int(0),
            expr_ident("chunks".to_string()),
            expr_list(vec![
                expr_ident("begin"),
                expr_list(vec![
                    expr_ident("let"),
                    expr_ident("pos".to_string()),
                    expr_list(vec![
                        expr_ident("*"),
                        expr_ident("i".to_string()),
                        cfg_emit_chunk_max_bytes.clone(),
                    ]),
                ]),
                expr_list(vec![
                    expr_ident("let"),
                    expr_ident("remain".to_string()),
                    expr_list(vec![
                        expr_ident("-"),
                        expr_ident("canon_len".to_string()),
                        expr_ident("pos".to_string()),
                    ]),
                ]),
                expr_list(vec![
                    expr_ident("let"),
                    expr_ident("take".to_string()),
                    expr_list(vec![
                        expr_ident("if"),
                        expr_list(vec![
                            expr_ident("<u"),
                            expr_ident("remain".to_string()),
                            cfg_emit_chunk_max_bytes.clone(),
                        ]),
                        expr_ident("remain".to_string()),
                        cfg_emit_chunk_max_bytes.clone(),
                    ]),
                ]),
                self.gen_process_from(
                    stage_idx + 1,
                    expr_list(vec![
                        expr_ident("view.slice"),
                        expr_ident("canon".to_string()),
                        expr_ident("pos".to_string()),
                        expr_ident("take".to_string()),
                    ]),
                )?,
                expr_int(0),
            ]),
        ]));

        stmts.push(self.gen_flush_from(stage_idx + 1)?);
        stmts.push(expr_int(0));
        Ok(expr_list(
            vec![expr_ident("begin")].into_iter().chain(stmts).collect(),
        ))
    }

    fn gen_flush_from(&self, stage_idx: usize) -> Result<Expr, CompilerError> {
        if stage_idx >= self.chain.len() {
            return Ok(expr_int(0));
        }
        match &self.chain[stage_idx] {
            PipeXfV1::JsonCanonStreamV1 { .. } => self.gen_json_canon_stream_flush(stage_idx),
            PipeXfV1::SplitLines { .. } => {
                let s = self
                    .split_states
                    .iter()
                    .find(|s| s.stage_idx == stage_idx)
                    .ok_or_else(|| {
                        CompilerError::new(
                            CompileErrorKind::Internal,
                            "internal error: missing split_lines state".to_string(),
                        )
                    })?;
                let carry = expr_ident(s.carry_var.clone());
                let mut stmts = vec![expr_list(vec![
                    expr_ident("if"),
                    expr_list(vec![
                        expr_ident(">u"),
                        expr_list(vec![expr_ident("vec_u8.len"), carry.clone()]),
                        expr_int(0),
                    ]),
                    expr_list(vec![
                        expr_ident("begin"),
                        expr_list(vec![
                            expr_ident("let"),
                            expr_ident("line_b".to_string()),
                            expr_list(vec![expr_ident("vec_u8.into_bytes"), carry.clone()]),
                        ]),
                        expr_list(vec![
                            expr_ident("let"),
                            expr_ident("line_v".to_string()),
                            expr_list(vec![
                                expr_ident("bytes.view"),
                                expr_ident("line_b".to_string()),
                            ]),
                        ]),
                        self.gen_process_from(stage_idx + 1, expr_ident("line_v".to_string()))?,
                        expr_int(0),
                    ]),
                    expr_int(0),
                ])];
                stmts.push(self.gen_flush_from(stage_idx + 1)?);
                Ok(expr_list(
                    vec![expr_ident("begin")].into_iter().chain(stmts).collect(),
                ))
            }
            PipeXfV1::DeframeU32LeV1 { .. } => {
                let d = self
                    .deframe_states
                    .iter()
                    .find(|d| d.stage_idx == stage_idx)
                    .ok_or_else(|| {
                        CompilerError::new(
                            CompileErrorKind::Internal,
                            "internal error: missing deframe state".to_string(),
                        )
                    })?;

                let hdr_fill = expr_ident(d.hdr_fill_var.clone());
                let need = expr_ident(d.need_var.clone());
                let buf_fill = expr_ident(d.buf_fill_var.clone());

                let mut stmts = vec![expr_list(vec![
                    expr_ident("if"),
                    expr_list(vec![
                        expr_ident("if"),
                        expr_list(vec![expr_ident("="), hdr_fill.clone(), expr_int(0)]),
                        expr_list(vec![expr_ident("="), buf_fill.clone(), expr_int(0)]),
                        expr_int(0),
                    ]),
                    expr_int(0),
                    match d.cfg.on_truncated {
                        DeframeOnTruncatedV1::Drop => expr_list(vec![
                            expr_ident("begin"),
                            expr_list(vec![expr_ident("set"), hdr_fill.clone(), expr_int(0)]),
                            expr_list(vec![expr_ident("set"), need.clone(), expr_int(0)]),
                            expr_list(vec![expr_ident("set"), buf_fill.clone(), expr_int(0)]),
                            expr_int(0),
                        ]),
                        DeframeOnTruncatedV1::Err => expr_list(vec![
                            expr_ident("return"),
                            err_doc_const(E_DEFRAME_TRUNCATED, "stream:deframe_truncated"),
                        ]),
                    },
                ])];

                stmts.push(self.gen_flush_from(stage_idx + 1)?);
                Ok(expr_list(
                    vec![expr_ident("begin")].into_iter().chain(stmts).collect(),
                ))
            }
            _ => self.gen_flush_from(stage_idx + 1),
        }
    }

    fn gen_deframe_emit_frame(
        &self,
        stage_idx: usize,
        payload: Expr,
        max_frames: i32,
        new_frames_var: &str,
        frames_var: &Expr,
    ) -> Result<Expr, CompilerError> {
        let mut stmts = Vec::new();
        stmts.push(expr_list(vec![
            expr_ident("let"),
            expr_ident(new_frames_var.to_string()),
            expr_list(vec![expr_ident("+"), frames_var.clone(), expr_int(1)]),
        ]));
        if max_frames > 0 {
            stmts.push(expr_list(vec![
                expr_ident("if"),
                expr_list(vec![
                    expr_ident(">u"),
                    expr_ident(new_frames_var.to_string()),
                    expr_int(max_frames),
                ]),
                expr_list(vec![
                    expr_ident("return"),
                    err_doc_const(E_DEFRAME_MAX_FRAMES, "stream:deframe_max_frames"),
                ]),
                expr_int(0),
            ]));
        }
        stmts.push(expr_list(vec![
            expr_ident("set"),
            frames_var.clone(),
            expr_ident(new_frames_var.to_string()),
        ]));
        stmts.push(self.gen_process_from(stage_idx + 1, payload)?);
        stmts.push(expr_int(0));
        Ok(expr_list(
            vec![expr_ident("begin")].into_iter().chain(stmts).collect(),
        ))
    }

    fn gen_net_sink_return_err(&self, doc: Expr) -> Expr {
        expr_list(vec![
            expr_ident("begin"),
            expr_list(vec![
                expr_ident("if"),
                expr_list(vec![
                    expr_ident("="),
                    expr_ident("net_sink_owned".to_string()),
                    expr_int(1),
                ]),
                expr_list(vec![
                    expr_ident("begin"),
                    expr_list(vec![
                        expr_ident("std.net.tcp.stream_close_v1"),
                        expr_ident("net_sink_h".to_string()),
                    ]),
                    expr_list(vec![
                        expr_ident("std.net.tcp.stream_drop_v1"),
                        expr_ident("net_sink_h".to_string()),
                    ]),
                    expr_int(0),
                ]),
                expr_int(0),
            ]),
            expr_list(vec![expr_ident("return"), doc]),
        ])
    }

    fn gen_net_sink_flush(&self, cfg: NetTcpWriteStreamHandleCfgV1) -> Expr {
        let buf = expr_ident("net_sink_buf".to_string());
        expr_list(vec![
            expr_ident("if"),
            expr_list(vec![
                expr_ident("<="),
                expr_list(vec![expr_ident("vec_u8.len"), buf.clone()]),
                expr_int(0),
            ]),
            expr_int(0),
            expr_list(vec![
                expr_ident("begin"),
                expr_list(vec![
                    expr_ident("let"),
                    expr_ident("bv".to_string()),
                    expr_list(vec![expr_ident("vec_u8.as_view"), buf.clone()]),
                ]),
                expr_list(vec![
                    expr_ident("let"),
                    expr_ident("bl".to_string()),
                    expr_list(vec![expr_ident("vec_u8.len"), buf.clone()]),
                ]),
                expr_list(vec![
                    expr_ident("set"),
                    expr_ident("net_sink_flushes".to_string()),
                    expr_list(vec![
                        expr_ident("+"),
                        expr_ident("net_sink_flushes".to_string()),
                        expr_int(1),
                    ]),
                ]),
                expr_list(vec![
                    expr_ident("if"),
                    expr_list(vec![
                        expr_ident(">u"),
                        expr_ident("net_sink_flushes".to_string()),
                        expr_ident("net_sink_max_flushes".to_string()),
                    ]),
                    self.gen_net_sink_return_err(err_doc_const(
                        E_NET_SINK_MAX_FLUSHES,
                        "stream:net_sink_max_flushes",
                    )),
                    expr_int(0),
                ]),
                expr_list(vec![
                    expr_ident("let"),
                    expr_ident("pos".to_string()),
                    expr_int(0),
                ]),
                expr_list(vec![
                    expr_ident("let"),
                    expr_ident("end".to_string()),
                    expr_list(vec![
                        expr_ident("+"),
                        expr_ident("bl".to_string()),
                        expr_int(1),
                    ]),
                ]),
                expr_list(vec![
                    expr_ident("for"),
                    expr_ident("_".to_string()),
                    expr_int(0),
                    expr_ident("end".to_string()),
                    expr_list(vec![
                        expr_ident("begin"),
                        expr_list(vec![
                            expr_ident("if"),
                            expr_list(vec![
                                expr_ident(">=u"),
                                expr_ident("pos".to_string()),
                                expr_ident("bl".to_string()),
                            ]),
                            expr_int(0),
                            expr_list(vec![
                                expr_ident("begin"),
                                expr_list(vec![
                                    expr_ident("let"),
                                    expr_ident("remain".to_string()),
                                    expr_list(vec![
                                        expr_ident("-"),
                                        expr_ident("bl".to_string()),
                                        expr_ident("pos".to_string()),
                                    ]),
                                ]),
                                expr_list(vec![
                                    expr_ident("let"),
                                    expr_ident("seg_len".to_string()),
                                    expr_list(vec![
                                        expr_ident("if"),
                                        expr_list(vec![
                                            expr_ident("<u"),
                                            expr_ident("remain".to_string()),
                                            expr_ident("net_sink_mw".to_string()),
                                        ]),
                                        expr_ident("remain".to_string()),
                                        expr_ident("net_sink_mw".to_string()),
                                    ]),
                                ]),
                                expr_list(vec![
                                    expr_ident("let"),
                                    expr_ident("seg".to_string()),
                                    expr_list(vec![
                                        expr_ident("view.slice"),
                                        expr_ident("bv".to_string()),
                                        expr_ident("pos".to_string()),
                                        expr_ident("seg_len".to_string()),
                                    ]),
                                ]),
                                expr_list(vec![
                                    expr_ident("let"),
                                    expr_ident("doc".to_string()),
                                    expr_list(vec![
                                        expr_ident("std.net.io.write_all_v1"),
                                        expr_ident("net_sink_h".to_string()),
                                        expr_ident("seg".to_string()),
                                        expr_ident("net_sink_caps".to_string()),
                                    ]),
                                ]),
                                expr_list(vec![
                                    expr_ident("let"),
                                    expr_ident("dv".to_string()),
                                    expr_list(vec![
                                        expr_ident("bytes.view"),
                                        expr_ident("doc".to_string()),
                                    ]),
                                ]),
                                expr_list(vec![
                                    expr_ident("if"),
                                    expr_list(vec![
                                        expr_ident("std.net.err.is_err_doc_v1"),
                                        expr_ident("dv".to_string()),
                                    ]),
                                    self.gen_net_sink_return_err(err_doc_with_payload(
                                        expr_int(E_NET_WRITE_FAILED),
                                        "stream:net_write_failed",
                                        expr_ident("doc".to_string()),
                                    )),
                                    expr_int(0),
                                ]),
                                expr_list(vec![
                                    expr_ident("set"),
                                    expr_ident("net_sink_write_calls".to_string()),
                                    expr_list(vec![
                                        expr_ident("+"),
                                        expr_ident("net_sink_write_calls".to_string()),
                                        expr_int(1),
                                    ]),
                                ]),
                                expr_list(vec![
                                    expr_ident("if"),
                                    expr_list(vec![
                                        expr_ident(">u"),
                                        expr_ident("net_sink_write_calls".to_string()),
                                        expr_ident("net_sink_max_write_calls".to_string()),
                                    ]),
                                    self.gen_net_sink_return_err(err_doc_const(
                                        E_NET_SINK_MAX_WRITES,
                                        "stream:net_sink_max_writes",
                                    )),
                                    expr_int(0),
                                ]),
                                expr_list(vec![
                                    expr_ident("set"),
                                    expr_ident("pos".to_string()),
                                    expr_list(vec![
                                        expr_ident("+"),
                                        expr_ident("pos".to_string()),
                                        expr_ident("seg_len".to_string()),
                                    ]),
                                ]),
                                expr_int(0),
                            ]),
                        ]),
                        expr_int(0),
                    ]),
                ]),
                expr_list(vec![
                    expr_ident("if"),
                    expr_list(vec![
                        expr_ident("<u"),
                        expr_ident("pos".to_string()),
                        expr_ident("bl".to_string()),
                    ]),
                    self.gen_net_sink_return_err(err_doc_const(
                        E_CFG_INVALID,
                        "stream:net_write_loop_overflow",
                    )),
                    expr_int(0),
                ]),
                expr_list(vec![
                    expr_ident("set"),
                    expr_ident("net_sink_buf".to_string()),
                    expr_list(vec![
                        expr_ident("vec_u8.with_capacity"),
                        expr_int(cfg.buf_cap_bytes),
                    ]),
                ]),
                expr_int(0),
            ]),
        ])
    }

    fn gen_net_sink_write_direct(&self, data: Expr, data_len: Expr) -> Expr {
        expr_list(vec![
            expr_ident("if"),
            expr_list(vec![expr_ident("<="), data_len.clone(), expr_int(0)]),
            expr_int(0),
            expr_list(vec![
                expr_ident("begin"),
                expr_list(vec![
                    expr_ident("set"),
                    expr_ident("net_sink_flushes".to_string()),
                    expr_list(vec![
                        expr_ident("+"),
                        expr_ident("net_sink_flushes".to_string()),
                        expr_int(1),
                    ]),
                ]),
                expr_list(vec![
                    expr_ident("if"),
                    expr_list(vec![
                        expr_ident(">u"),
                        expr_ident("net_sink_flushes".to_string()),
                        expr_ident("net_sink_max_flushes".to_string()),
                    ]),
                    self.gen_net_sink_return_err(err_doc_const(
                        E_NET_SINK_MAX_FLUSHES,
                        "stream:net_sink_max_flushes",
                    )),
                    expr_int(0),
                ]),
                expr_list(vec![
                    expr_ident("let"),
                    expr_ident("pos".to_string()),
                    expr_int(0),
                ]),
                expr_list(vec![
                    expr_ident("let"),
                    expr_ident("end".to_string()),
                    expr_list(vec![expr_ident("+"), data_len.clone(), expr_int(1)]),
                ]),
                expr_list(vec![
                    expr_ident("for"),
                    expr_ident("_".to_string()),
                    expr_int(0),
                    expr_ident("end".to_string()),
                    expr_list(vec![
                        expr_ident("begin"),
                        expr_list(vec![
                            expr_ident("if"),
                            expr_list(vec![
                                expr_ident(">=u"),
                                expr_ident("pos".to_string()),
                                data_len.clone(),
                            ]),
                            expr_int(0),
                            expr_list(vec![
                                expr_ident("begin"),
                                expr_list(vec![
                                    expr_ident("let"),
                                    expr_ident("remain".to_string()),
                                    expr_list(vec![
                                        expr_ident("-"),
                                        data_len.clone(),
                                        expr_ident("pos".to_string()),
                                    ]),
                                ]),
                                expr_list(vec![
                                    expr_ident("let"),
                                    expr_ident("seg_len".to_string()),
                                    expr_list(vec![
                                        expr_ident("if"),
                                        expr_list(vec![
                                            expr_ident("<u"),
                                            expr_ident("remain".to_string()),
                                            expr_ident("net_sink_mw".to_string()),
                                        ]),
                                        expr_ident("remain".to_string()),
                                        expr_ident("net_sink_mw".to_string()),
                                    ]),
                                ]),
                                expr_list(vec![
                                    expr_ident("let"),
                                    expr_ident("seg".to_string()),
                                    expr_list(vec![
                                        expr_ident("view.slice"),
                                        data.clone(),
                                        expr_ident("pos".to_string()),
                                        expr_ident("seg_len".to_string()),
                                    ]),
                                ]),
                                expr_list(vec![
                                    expr_ident("let"),
                                    expr_ident("doc".to_string()),
                                    expr_list(vec![
                                        expr_ident("std.net.io.write_all_v1"),
                                        expr_ident("net_sink_h".to_string()),
                                        expr_ident("seg".to_string()),
                                        expr_ident("net_sink_caps".to_string()),
                                    ]),
                                ]),
                                expr_list(vec![
                                    expr_ident("let"),
                                    expr_ident("dv".to_string()),
                                    expr_list(vec![
                                        expr_ident("bytes.view"),
                                        expr_ident("doc".to_string()),
                                    ]),
                                ]),
                                expr_list(vec![
                                    expr_ident("if"),
                                    expr_list(vec![
                                        expr_ident("std.net.err.is_err_doc_v1"),
                                        expr_ident("dv".to_string()),
                                    ]),
                                    self.gen_net_sink_return_err(err_doc_with_payload(
                                        expr_int(E_NET_WRITE_FAILED),
                                        "stream:net_write_failed",
                                        expr_ident("doc".to_string()),
                                    )),
                                    expr_int(0),
                                ]),
                                expr_list(vec![
                                    expr_ident("set"),
                                    expr_ident("net_sink_write_calls".to_string()),
                                    expr_list(vec![
                                        expr_ident("+"),
                                        expr_ident("net_sink_write_calls".to_string()),
                                        expr_int(1),
                                    ]),
                                ]),
                                expr_list(vec![
                                    expr_ident("if"),
                                    expr_list(vec![
                                        expr_ident(">u"),
                                        expr_ident("net_sink_write_calls".to_string()),
                                        expr_ident("net_sink_max_write_calls".to_string()),
                                    ]),
                                    self.gen_net_sink_return_err(err_doc_const(
                                        E_NET_SINK_MAX_WRITES,
                                        "stream:net_sink_max_writes",
                                    )),
                                    expr_int(0),
                                ]),
                                expr_list(vec![
                                    expr_ident("set"),
                                    expr_ident("pos".to_string()),
                                    expr_list(vec![
                                        expr_ident("+"),
                                        expr_ident("pos".to_string()),
                                        expr_ident("seg_len".to_string()),
                                    ]),
                                ]),
                                expr_int(0),
                            ]),
                        ]),
                        expr_int(0),
                    ]),
                ]),
                expr_list(vec![
                    expr_ident("if"),
                    expr_list(vec![
                        expr_ident("<u"),
                        expr_ident("pos".to_string()),
                        data_len.clone(),
                    ]),
                    self.gen_net_sink_return_err(err_doc_const(
                        E_CFG_INVALID,
                        "stream:net_write_loop_overflow",
                    )),
                    expr_int(0),
                ]),
                expr_int(0),
            ]),
        ])
    }

    fn gen_net_sink_push(
        &self,
        data: Expr,
        data_len: Expr,
        cfg: NetTcpWriteStreamHandleCfgV1,
    ) -> Expr {
        let buf = expr_ident("net_sink_buf".to_string());
        expr_list(vec![
            expr_ident("if"),
            expr_list(vec![expr_ident("<="), data_len.clone(), expr_int(0)]),
            expr_int(0),
            expr_list(vec![
                expr_ident("begin"),
                expr_list(vec![
                    expr_ident("let"),
                    expr_ident("bl".to_string()),
                    expr_list(vec![expr_ident("vec_u8.len"), buf.clone()]),
                ]),
                expr_list(vec![
                    expr_ident("if"),
                    expr_list(vec![
                        expr_ident(">u"),
                        expr_list(vec![
                            expr_ident("+"),
                            expr_ident("bl".to_string()),
                            data_len.clone(),
                        ]),
                        expr_int(cfg.buf_cap_bytes),
                    ]),
                    expr_list(vec![
                        expr_ident("begin"),
                        self.gen_net_sink_flush(cfg),
                        expr_list(vec![
                            expr_ident("if"),
                            expr_list(vec![
                                expr_ident(">=u"),
                                data_len.clone(),
                                expr_int(cfg.buf_cap_bytes),
                            ]),
                            self.gen_net_sink_write_direct(data.clone(), data_len.clone()),
                            expr_list(vec![
                                expr_ident("set"),
                                buf.clone(),
                                expr_list(vec![
                                    expr_ident("vec_u8.extend_bytes_range"),
                                    buf.clone(),
                                    data.clone(),
                                    expr_int(0),
                                    data_len.clone(),
                                ]),
                            ]),
                        ]),
                        expr_int(0),
                    ]),
                    expr_list(vec![
                        expr_ident("set"),
                        buf.clone(),
                        expr_list(vec![
                            expr_ident("vec_u8.extend_bytes_range"),
                            buf.clone(),
                            data.clone(),
                            expr_int(0),
                            data_len.clone(),
                        ]),
                    ]),
                ]),
                expr_list(vec![
                    expr_ident("if"),
                    expr_list(vec![
                        expr_ident(">=u"),
                        expr_list(vec![expr_ident("vec_u8.len"), buf.clone()]),
                        expr_int(cfg.flush_min_bytes),
                    ]),
                    self.gen_net_sink_flush(cfg),
                    expr_int(0),
                ]),
                expr_int(0),
            ]),
        ])
    }

    fn gen_fs_sink_write_direct(&self, data: Expr, hash: bool) -> Expr {
        let mut stmts = vec![expr_list(vec![
            expr_ident("set"),
            expr_ident("fs_sink_flushes".to_string()),
            expr_list(vec![
                expr_ident("+"),
                expr_ident("fs_sink_flushes".to_string()),
                expr_int(1),
            ]),
        ])];
        stmts.push(expr_list(vec![
            expr_ident("if"),
            expr_list(vec![
                expr_ident(">u"),
                expr_ident("fs_sink_flushes".to_string()),
                expr_ident("fs_sink_max_flushes".to_string()),
            ]),
            expr_list(vec![
                expr_ident("begin"),
                expr_list(vec![
                    expr_ident("os.fs.stream_close_v1"),
                    expr_ident("fs_sink_h".to_string()),
                ]),
                expr_list(vec![
                    expr_ident("os.fs.stream_drop_v1"),
                    expr_ident("fs_sink_h".to_string()),
                ]),
                expr_list(vec![
                    expr_ident("let"),
                    expr_ident("pl".to_string()),
                    expr_list(vec![expr_ident("vec_u8.with_capacity"), expr_int(12)]),
                ]),
                extend_u32("pl", expr_int(0)),
                extend_u32("pl", expr_ident("fs_sink_flushes".to_string())),
                extend_u32("pl", expr_ident(self.bytes_out_var.clone())),
                expr_list(vec![
                    expr_ident("return"),
                    err_doc_with_payload(
                        expr_int(E_SINK_TOO_MANY_FLUSHES),
                        "stream:fs_too_many_flushes",
                        expr_list(vec![
                            expr_ident("vec_u8.into_bytes"),
                            expr_ident("pl".to_string()),
                        ]),
                    ),
                ]),
            ]),
            expr_int(0),
        ]));

        stmts.push(expr_list(vec![
            expr_ident("let"),
            expr_ident("wr".to_string()),
            expr_list(vec![
                expr_ident("os.fs.stream_write_all_v1"),
                expr_ident("fs_sink_h".to_string()),
                data.clone(),
            ]),
        ]));
        stmts.push(expr_list(vec![
            expr_ident("if"),
            expr_list(vec![
                expr_ident("result_i32.is_ok"),
                expr_ident("wr".to_string()),
            ]),
            expr_int(0),
            expr_list(vec![
                expr_ident("begin"),
                expr_list(vec![
                    expr_ident("os.fs.stream_close_v1"),
                    expr_ident("fs_sink_h".to_string()),
                ]),
                expr_list(vec![
                    expr_ident("os.fs.stream_drop_v1"),
                    expr_ident("fs_sink_h".to_string()),
                ]),
                expr_list(vec![
                    expr_ident("let"),
                    expr_ident("ec".to_string()),
                    expr_list(vec![
                        expr_ident("result_i32.err_code"),
                        expr_ident("wr".to_string()),
                    ]),
                ]),
                expr_list(vec![
                    expr_ident("let"),
                    expr_ident("pl".to_string()),
                    expr_list(vec![expr_ident("vec_u8.with_capacity"), expr_int(12)]),
                ]),
                extend_u32("pl", expr_ident("ec".to_string())),
                extend_u32("pl", expr_ident("fs_sink_flushes".to_string())),
                extend_u32("pl", expr_ident(self.bytes_out_var.clone())),
                expr_list(vec![
                    expr_ident("return"),
                    err_doc_with_payload(
                        expr_int(E_SINK_FS_WRITE_FAILED),
                        "stream:fs_write_failed",
                        expr_list(vec![
                            expr_ident("vec_u8.into_bytes"),
                            expr_ident("pl".to_string()),
                        ]),
                    ),
                ]),
            ]),
        ]));

        if hash {
            let hash_name = self.hash_var.as_ref().expect("hash state");
            stmts.push(self.gen_fnv1a_update(data, hash_name));
        }

        stmts.push(expr_int(0));
        expr_list(vec![expr_ident("begin")].into_iter().chain(stmts).collect())
    }

    fn gen_fs_sink_flush(&self, hash: bool) -> Expr {
        let buf = expr_ident("fs_sink_buf".to_string());
        expr_list(vec![
            expr_ident("if"),
            expr_list(vec![
                expr_ident("<="),
                expr_list(vec![expr_ident("vec_u8.len"), buf.clone()]),
                expr_int(0),
            ]),
            expr_int(0),
            expr_list(vec![
                expr_ident("begin"),
                expr_list(vec![
                    expr_ident("let"),
                    expr_ident("bv".to_string()),
                    expr_list(vec![expr_ident("vec_u8.as_view"), buf.clone()]),
                ]),
                self.gen_fs_sink_write_direct(expr_ident("bv".to_string()), hash),
                expr_list(vec![
                    expr_ident("set"),
                    expr_ident("fs_sink_buf".to_string()),
                    expr_list(vec![
                        expr_ident("vec_u8.clear"),
                        expr_ident("fs_sink_buf".to_string()),
                    ]),
                ]),
                expr_int(0),
            ]),
        ])
    }

    fn gen_fs_sink_push(
        &self,
        data: Expr,
        data_len: Expr,
        cfg: WorldFsWriteStreamCfgV1,
        hash: bool,
    ) -> Expr {
        let buf = expr_ident("fs_sink_buf".to_string());
        expr_list(vec![
            expr_ident("if"),
            expr_list(vec![expr_ident("<="), data_len.clone(), expr_int(0)]),
            expr_int(0),
            expr_list(vec![
                expr_ident("begin"),
                expr_list(vec![
                    expr_ident("let"),
                    expr_ident("bl".to_string()),
                    expr_list(vec![expr_ident("vec_u8.len"), buf.clone()]),
                ]),
                expr_list(vec![
                    expr_ident("if"),
                    expr_list(vec![
                        expr_ident(">u"),
                        expr_list(vec![
                            expr_ident("+"),
                            expr_ident("bl".to_string()),
                            data_len.clone(),
                        ]),
                        expr_int(cfg.buf_cap_bytes),
                    ]),
                    expr_list(vec![
                        expr_ident("begin"),
                        self.gen_fs_sink_flush(hash),
                        expr_list(vec![
                            expr_ident("if"),
                            expr_list(vec![
                                expr_ident(">=u"),
                                data_len.clone(),
                                expr_int(cfg.buf_cap_bytes),
                            ]),
                            self.gen_fs_sink_write_direct(data.clone(), hash),
                            expr_list(vec![
                                expr_ident("set"),
                                buf.clone(),
                                expr_list(vec![
                                    expr_ident("vec_u8.extend_bytes_range"),
                                    buf.clone(),
                                    data.clone(),
                                    expr_int(0),
                                    data_len.clone(),
                                ]),
                            ]),
                        ]),
                        expr_int(0),
                    ]),
                    expr_list(vec![
                        expr_ident("set"),
                        buf.clone(),
                        expr_list(vec![
                            expr_ident("vec_u8.extend_bytes_range"),
                            buf.clone(),
                            data.clone(),
                            expr_int(0),
                            data_len.clone(),
                        ]),
                    ]),
                ]),
                expr_list(vec![
                    expr_ident("if"),
                    expr_list(vec![
                        expr_ident(">=u"),
                        expr_list(vec![expr_ident("vec_u8.len"), buf.clone()]),
                        expr_int(cfg.flush_min_bytes),
                    ]),
                    self.gen_fs_sink_flush(hash),
                    expr_int(0),
                ]),
                expr_int(0),
            ]),
        ])
    }

    fn emit_item(&self, item: Expr) -> Result<Expr, CompilerError> {
        let len_var = "emit_len".to_string();
        let mut stmts = Vec::new();
        stmts.push(expr_list(vec![
            expr_ident("let"),
            expr_ident(len_var.clone()),
            expr_list(vec![expr_ident("view.len"), item.clone()]),
        ]));
        if self.sink.framing_u32frames {
            stmts.push(expr_list(vec![
                expr_ident("if"),
                expr_list(vec![
                    expr_ident("<"),
                    expr_ident(len_var.clone()),
                    expr_int(0),
                ]),
                expr_list(vec![
                    expr_ident("return"),
                    err_doc_const(E_FRAME_TOO_LARGE, "stream:frame_too_large"),
                ]),
                expr_int(0),
            ]));
        }

        // items_out += 1 (budgeted)
        stmts.push(expr_list(vec![
            expr_ident("let"),
            expr_ident("new_items_out".to_string()),
            expr_list(vec![
                expr_ident("+"),
                expr_ident(self.items_out_var.clone()),
                expr_int(1),
            ]),
        ]));
        stmts.push(expr_list(vec![
            expr_ident("if"),
            expr_list(vec![
                expr_ident(">u"),
                expr_ident("new_items_out".to_string()),
                expr_int(self.cfg.max_items),
            ]),
            expr_list(vec![
                expr_ident("return"),
                err_doc_const(E_BUDGET_ITEMS, "stream:budget_items"),
            ]),
            expr_int(0),
        ]));
        stmts.push(expr_list(vec![
            expr_ident("set"),
            expr_ident(self.items_out_var.clone()),
            expr_ident("new_items_out".to_string()),
        ]));

        let bytes_inc = if self.sink.framing_u32frames {
            expr_list(vec![
                expr_ident("+"),
                expr_int(4),
                expr_ident(len_var.clone()),
            ])
        } else {
            expr_ident(len_var.clone())
        };

        stmts.push(expr_list(vec![
            expr_ident("let"),
            expr_ident("new_bytes_out".to_string()),
            expr_list(vec![
                expr_ident("+"),
                expr_ident(self.bytes_out_var.clone()),
                bytes_inc,
            ]),
        ]));
        stmts.push(expr_list(vec![
            expr_ident("if"),
            expr_list(vec![
                expr_ident(">u"),
                expr_ident("new_bytes_out".to_string()),
                expr_int(self.cfg.max_out_bytes),
            ]),
            expr_list(vec![
                expr_ident("return"),
                err_doc_const(E_BUDGET_OUT_BYTES, "stream:budget_out_bytes"),
            ]),
            expr_int(0),
        ]));
        stmts.push(expr_list(vec![
            expr_ident("set"),
            expr_ident(self.bytes_out_var.clone()),
            expr_ident("new_bytes_out".to_string()),
        ]));

        // Feed to sink.
        match &self.sink.base {
            SinkBaseV1::CollectBytes | SinkBaseV1::WorldFsWriteFile { .. } => {
                let vec_name = self.sink_vec_var.as_ref().ok_or_else(|| {
                    CompilerError::new(
                        CompileErrorKind::Internal,
                        "internal error: missing sink_vec".to_string(),
                    )
                })?;
                if self.sink.framing_u32frames {
                    stmts.push(expr_list(vec![
                        expr_ident("let"),
                        expr_ident("hdr".to_string()),
                        expr_list(vec![
                            expr_ident("codec.write_u32_le"),
                            expr_ident(len_var.clone()),
                        ]),
                    ]));
                    stmts.push(expr_list(vec![
                        expr_ident("set"),
                        expr_ident(vec_name.clone()),
                        expr_list(vec![
                            expr_ident("vec_u8.extend_bytes"),
                            expr_ident(vec_name.clone()),
                            expr_ident("hdr".to_string()),
                        ]),
                    ]));
                }
                stmts.push(expr_list(vec![
                    expr_ident("set"),
                    expr_ident(vec_name.clone()),
                    expr_list(vec![
                        expr_ident("vec_u8.extend_bytes_range"),
                        expr_ident(vec_name.clone()),
                        item,
                        expr_int(0),
                        expr_ident(len_var),
                    ]),
                ]));
            }
            SinkBaseV1::HashFnv1a32 => {
                let hash_name = self.hash_var.as_ref().ok_or_else(|| {
                    CompilerError::new(
                        CompileErrorKind::Internal,
                        "internal error: missing hash state".to_string(),
                    )
                })?;
                if self.sink.framing_u32frames {
                    stmts.push(expr_list(vec![
                        expr_ident("let"),
                        expr_ident("hdr".to_string()),
                        expr_list(vec![
                            expr_ident("codec.write_u32_le"),
                            expr_ident(len_var.clone()),
                        ]),
                    ]));
                    stmts.push(self.gen_fnv1a_update(
                        expr_list(vec![
                            expr_ident("bytes.view"),
                            expr_ident("hdr".to_string()),
                        ]),
                        hash_name,
                    ));
                }
                stmts.push(self.gen_fnv1a_update(item, hash_name));
            }
            SinkBaseV1::Null => {}
            SinkBaseV1::WorldFsWriteStream { cfg, .. } => {
                if self.sink.framing_u32frames {
                    stmts.push(expr_list(vec![
                        expr_ident("let"),
                        expr_ident("hdr".to_string()),
                        expr_list(vec![
                            expr_ident("codec.write_u32_le"),
                            expr_ident(len_var.clone()),
                        ]),
                    ]));
                    stmts.push(self.gen_fs_sink_push(
                        expr_list(vec![
                            expr_ident("bytes.view"),
                            expr_ident("hdr".to_string()),
                        ]),
                        expr_int(4),
                        *cfg,
                        false,
                    ));
                }
                stmts.push(self.gen_fs_sink_push(item, expr_ident(len_var.clone()), *cfg, false));
            }
            SinkBaseV1::WorldFsWriteStreamHashFnv1a32 { cfg, .. } => {
                if self.sink.framing_u32frames {
                    stmts.push(expr_list(vec![
                        expr_ident("let"),
                        expr_ident("hdr".to_string()),
                        expr_list(vec![
                            expr_ident("codec.write_u32_le"),
                            expr_ident(len_var.clone()),
                        ]),
                    ]));
                    stmts.push(self.gen_fs_sink_push(
                        expr_list(vec![
                            expr_ident("bytes.view"),
                            expr_ident("hdr".to_string()),
                        ]),
                        expr_int(4),
                        *cfg,
                        true,
                    ));
                }
                stmts.push(self.gen_fs_sink_push(item, expr_ident(len_var.clone()), *cfg, true));
            }
            SinkBaseV1::NetTcpWriteStreamHandle { cfg, .. }
            | SinkBaseV1::NetTcpConnectWrite { cfg, .. } => {
                if self.sink.framing_u32frames {
                    stmts.push(expr_list(vec![
                        expr_ident("let"),
                        expr_ident("hdr".to_string()),
                        expr_list(vec![
                            expr_ident("codec.write_u32_le"),
                            expr_ident(len_var.clone()),
                        ]),
                    ]));
                    stmts.push(self.gen_net_sink_push(
                        expr_list(vec![
                            expr_ident("bytes.view"),
                            expr_ident("hdr".to_string()),
                        ]),
                        expr_int(4),
                        *cfg,
                    ));
                }
                stmts.push(self.gen_net_sink_push(item, expr_ident(len_var.clone()), *cfg));
            }
        }

        stmts.push(expr_int(0));
        Ok(expr_list(
            vec![expr_ident("begin")].into_iter().chain(stmts).collect(),
        ))
    }

    fn gen_fnv1a_update(&self, data: Expr, hash_name: &str) -> Expr {
        expr_list(vec![
            expr_ident("begin"),
            expr_list(vec![
                expr_ident("let"),
                expr_ident("n".to_string()),
                expr_list(vec![expr_ident("view.len"), data.clone()]),
            ]),
            expr_list(vec![
                expr_ident("for"),
                expr_ident("i".to_string()),
                expr_int(0),
                expr_ident("n".to_string()),
                expr_list(vec![
                    expr_ident("begin"),
                    expr_list(vec![
                        expr_ident("let"),
                        expr_ident("c".to_string()),
                        expr_list(vec![
                            expr_ident("view.get_u8"),
                            data.clone(),
                            expr_ident("i".to_string()),
                        ]),
                    ]),
                    expr_list(vec![
                        expr_ident("set"),
                        expr_ident(hash_name.to_string()),
                        expr_list(vec![
                            expr_ident("^"),
                            expr_ident(hash_name.to_string()),
                            expr_ident("c".to_string()),
                        ]),
                    ]),
                    expr_list(vec![
                        expr_ident("set"),
                        expr_ident(hash_name.to_string()),
                        expr_list(vec![
                            expr_ident("*"),
                            expr_ident(hash_name.to_string()),
                            expr_int(FNV1A32_PRIME),
                        ]),
                    ]),
                    expr_int(0),
                ]),
            ]),
            expr_int(0),
        ])
    }

    fn gen_return_ok(&self) -> Result<Expr, CompilerError> {
        let payload_expr = match &self.sink.base {
            SinkBaseV1::CollectBytes => {
                let vec_name = self.sink_vec_var.as_ref().ok_or_else(|| {
                    CompilerError::new(
                        CompileErrorKind::Internal,
                        "internal error: missing sink_vec".to_string(),
                    )
                })?;
                expr_list(vec![
                    expr_ident("vec_u8.into_bytes"),
                    expr_ident(vec_name.clone()),
                ])
            }
            SinkBaseV1::HashFnv1a32 => {
                let hash_name = self.hash_var.as_ref().ok_or_else(|| {
                    CompilerError::new(
                        CompileErrorKind::Internal,
                        "internal error: missing hash state".to_string(),
                    )
                })?;
                expr_list(vec![
                    expr_ident("codec.write_u32_le"),
                    expr_ident(hash_name.clone()),
                ])
            }
            SinkBaseV1::Null => expr_list(vec![expr_ident("bytes.alloc"), expr_int(0)]),
            SinkBaseV1::WorldFsWriteFile { path_param } => {
                let vec_name = self.sink_vec_var.as_ref().ok_or_else(|| {
                    CompilerError::new(
                        CompileErrorKind::Internal,
                        "internal error: missing sink_vec".to_string(),
                    )
                })?;
                let file_bytes = "file_bytes".to_string();
                let rc = "rc".to_string();
                return Ok(expr_list(vec![
                    expr_ident("begin"),
                    expr_list(vec![
                        expr_ident("let"),
                        expr_ident(file_bytes.clone()),
                        expr_list(vec![
                            expr_ident("vec_u8.into_bytes"),
                            expr_ident(vec_name.clone()),
                        ]),
                    ]),
                    expr_list(vec![
                        expr_ident("let"),
                        expr_ident(rc.clone()),
                        expr_list(vec![
                            expr_ident("os.fs.write_file"),
                            param_ident(*path_param),
                            expr_ident(file_bytes),
                        ]),
                    ]),
                    expr_list(vec![
                        expr_ident("if"),
                        expr_list(vec![expr_ident("!="), expr_ident(rc.clone()), expr_int(0)]),
                        expr_list(vec![
                            expr_ident("return"),
                            err_doc_with_payload(
                                expr_int(E_SINK_FS_WRITE_FAILED),
                                "stream:fs_write_failed",
                                expr_list(vec![expr_ident("codec.write_u32_le"), expr_ident(rc)]),
                            ),
                        ]),
                        expr_int(0),
                    ]),
                    expr_list(vec![
                        expr_ident("return"),
                        ok_doc(
                            self,
                            expr_list(vec![expr_ident("bytes.alloc"), expr_int(0)]),
                        ),
                    ]),
                ]));
            }
            SinkBaseV1::WorldFsWriteStream { .. }
            | SinkBaseV1::WorldFsWriteStreamHashFnv1a32 { .. } => {
                let hash_payload = match &self.sink.base {
                    SinkBaseV1::WorldFsWriteStreamHashFnv1a32 { .. } => {
                        let hash_name = self.hash_var.as_ref().ok_or_else(|| {
                            CompilerError::new(
                                CompileErrorKind::Internal,
                                "internal error: missing hash state".to_string(),
                            )
                        })?;
                        Some(expr_list(vec![
                            expr_ident("codec.write_u32_le"),
                            expr_ident(hash_name.clone()),
                        ]))
                    }
                    _ => None,
                };

                return Ok(expr_list(vec![
                    expr_ident("begin"),
                    self.gen_fs_sink_flush(hash_payload.is_some()),
                    expr_list(vec![
                        expr_ident("let"),
                        expr_ident("cl".to_string()),
                        expr_list(vec![
                            expr_ident("os.fs.stream_close_v1"),
                            expr_ident("fs_sink_h".to_string()),
                        ]),
                    ]),
                    expr_list(vec![
                        expr_ident("os.fs.stream_drop_v1"),
                        expr_ident("fs_sink_h".to_string()),
                    ]),
                    expr_list(vec![
                        expr_ident("if"),
                        expr_list(vec![
                            expr_ident("result_i32.is_ok"),
                            expr_ident("cl".to_string()),
                        ]),
                        expr_int(0),
                        expr_list(vec![
                            expr_ident("begin"),
                            expr_list(vec![
                                expr_ident("let"),
                                expr_ident("ec".to_string()),
                                expr_list(vec![
                                    expr_ident("result_i32.err_code"),
                                    expr_ident("cl".to_string()),
                                ]),
                            ]),
                            expr_list(vec![
                                expr_ident("let"),
                                expr_ident("pl".to_string()),
                                expr_list(vec![expr_ident("vec_u8.with_capacity"), expr_int(12)]),
                            ]),
                            extend_u32("pl", expr_ident("ec".to_string())),
                            extend_u32("pl", expr_ident("fs_sink_flushes".to_string())),
                            extend_u32("pl", expr_ident(self.bytes_out_var.clone())),
                            expr_list(vec![
                                expr_ident("return"),
                                err_doc_with_payload(
                                    expr_int(E_SINK_FS_CLOSE_FAILED),
                                    "stream:fs_close_failed",
                                    expr_list(vec![
                                        expr_ident("vec_u8.into_bytes"),
                                        expr_ident("pl".to_string()),
                                    ]),
                                ),
                            ]),
                        ]),
                    ]),
                    expr_list(vec![
                        expr_ident("return"),
                        ok_doc(
                            self,
                            hash_payload.unwrap_or_else(|| {
                                expr_list(vec![expr_ident("bytes.alloc"), expr_int(0)])
                            }),
                        ),
                    ]),
                ]));
            }
            SinkBaseV1::NetTcpWriteStreamHandle { cfg, .. }
            | SinkBaseV1::NetTcpConnectWrite { cfg, .. } => {
                let shutdown_write = expr_list(vec![
                    expr_ident("std.net.tcp.stream_shutdown_v1"),
                    expr_ident("net_sink_h".to_string()),
                    expr_list(vec![expr_ident("std.net.tcp.shutdown_write_v1")]),
                ]);
                let close_drop = expr_list(vec![
                    expr_ident("begin"),
                    expr_list(vec![
                        expr_ident("std.net.tcp.stream_close_v1"),
                        expr_ident("net_sink_h".to_string()),
                    ]),
                    expr_list(vec![
                        expr_ident("std.net.tcp.stream_drop_v1"),
                        expr_ident("net_sink_h".to_string()),
                    ]),
                    expr_int(0),
                ]);

                let finish = match cfg.on_finish {
                    NetSinkOnFinishV1::LeaveOpen => expr_int(0),
                    NetSinkOnFinishV1::ShutdownWrite => expr_list(vec![
                        expr_ident("begin"),
                        shutdown_write,
                        expr_list(vec![
                            expr_ident("if"),
                            expr_list(vec![
                                expr_ident("="),
                                expr_ident("net_sink_owned".to_string()),
                                expr_int(1),
                            ]),
                            close_drop.clone(),
                            expr_int(0),
                        ]),
                        expr_int(0),
                    ]),
                    NetSinkOnFinishV1::Close => close_drop,
                };

                return Ok(expr_list(vec![
                    expr_ident("begin"),
                    self.gen_net_sink_flush(*cfg),
                    finish,
                    expr_list(vec![
                        expr_ident("return"),
                        ok_doc(
                            self,
                            expr_list(vec![expr_ident("bytes.alloc"), expr_int(0)]),
                        ),
                    ]),
                ]));
            }
        };

        Ok(expr_list(vec![
            expr_ident("return"),
            ok_doc(self, payload_expr),
        ]))
    }
}

fn ok_doc(cg: &PipeCodegen<'_>, payload: Expr) -> Expr {
    let payload = if cg.emit_payload {
        payload
    } else {
        expr_list(vec![expr_ident("bytes.alloc"), expr_int(0)])
    };

    let bytes_in = if cg.emit_stats {
        expr_ident(cg.bytes_in_var.clone())
    } else {
        expr_int(0)
    };
    let bytes_out = if cg.emit_stats {
        expr_ident(cg.bytes_out_var.clone())
    } else {
        expr_int(0)
    };
    let items_in = if cg.emit_stats {
        expr_ident(cg.items_in_var.clone())
    } else {
        expr_int(0)
    };
    let items_out = if cg.emit_stats {
        expr_ident(cg.items_out_var.clone())
    } else {
        expr_int(0)
    };

    expr_list(vec![
        expr_ident("begin"),
        expr_list(vec![
            expr_ident("let"),
            expr_ident("plb".to_string()),
            payload,
        ]),
        expr_list(vec![
            expr_ident("let"),
            expr_ident("plv".to_string()),
            expr_list(vec![
                expr_ident("bytes.view"),
                expr_ident("plb".to_string()),
            ]),
        ]),
        expr_list(vec![
            expr_ident("let"),
            expr_ident("pl_len".to_string()),
            expr_list(vec![expr_ident("view.len"), expr_ident("plv".to_string())]),
        ]),
        expr_list(vec![
            expr_ident("let"),
            expr_ident("out".to_string()),
            expr_list(vec![
                expr_ident("vec_u8.with_capacity"),
                expr_list(vec![
                    expr_ident("+"),
                    expr_int(21),
                    expr_ident("pl_len".to_string()),
                ]),
            ]),
        ]),
        expr_list(vec![
            expr_ident("set"),
            expr_ident("out".to_string()),
            expr_list(vec![
                expr_ident("vec_u8.push"),
                expr_ident("out".to_string()),
                expr_int(1),
            ]),
        ]),
        extend_u32("out", bytes_in),
        extend_u32("out", bytes_out),
        extend_u32("out", items_in),
        extend_u32("out", items_out),
        extend_u32("out", expr_ident("pl_len".to_string())),
        expr_list(vec![
            expr_ident("set"),
            expr_ident("out".to_string()),
            expr_list(vec![
                expr_ident("vec_u8.extend_bytes"),
                expr_ident("out".to_string()),
                expr_ident("plb".to_string()),
            ]),
        ]),
        expr_list(vec![
            expr_ident("vec_u8.into_bytes"),
            expr_ident("out".to_string()),
        ]),
    ])
}

fn err_doc_const(code: i32, msg: &str) -> Expr {
    err_doc_with_payload(
        expr_int(code),
        msg,
        expr_list(vec![expr_ident("bytes.alloc"), expr_int(0)]),
    )
}

fn err_doc_with_payload(code: Expr, msg: &str, payload: Expr) -> Expr {
    expr_list(vec![
        expr_ident("begin"),
        expr_list(vec![
            expr_ident("let"),
            expr_ident("msg".to_string()),
            expr_list(vec![expr_ident("bytes.lit"), expr_ident(msg.to_string())]),
        ]),
        expr_list(vec![
            expr_ident("let"),
            expr_ident("msgv".to_string()),
            expr_list(vec![
                expr_ident("bytes.view"),
                expr_ident("msg".to_string()),
            ]),
        ]),
        expr_list(vec![
            expr_ident("let"),
            expr_ident("msg_len".to_string()),
            expr_list(vec![expr_ident("view.len"), expr_ident("msgv".to_string())]),
        ]),
        expr_list(vec![
            expr_ident("let"),
            expr_ident("plb".to_string()),
            payload,
        ]),
        expr_list(vec![
            expr_ident("let"),
            expr_ident("plv".to_string()),
            expr_list(vec![
                expr_ident("bytes.view"),
                expr_ident("plb".to_string()),
            ]),
        ]),
        expr_list(vec![
            expr_ident("let"),
            expr_ident("pl_len".to_string()),
            expr_list(vec![expr_ident("view.len"), expr_ident("plv".to_string())]),
        ]),
        expr_list(vec![
            expr_ident("let"),
            expr_ident("out".to_string()),
            expr_list(vec![
                expr_ident("vec_u8.with_capacity"),
                expr_list(vec![
                    expr_ident("+"),
                    expr_list(vec![
                        expr_ident("+"),
                        expr_int(13),
                        expr_ident("msg_len".to_string()),
                    ]),
                    expr_ident("pl_len".to_string()),
                ]),
            ]),
        ]),
        expr_list(vec![
            expr_ident("set"),
            expr_ident("out".to_string()),
            expr_list(vec![
                expr_ident("vec_u8.push"),
                expr_ident("out".to_string()),
                expr_int(0),
            ]),
        ]),
        expr_list(vec![
            expr_ident("set"),
            expr_ident("out".to_string()),
            expr_list(vec![
                expr_ident("vec_u8.extend_bytes"),
                expr_ident("out".to_string()),
                expr_list(vec![expr_ident("codec.write_u32_le"), code]),
            ]),
        ]),
        extend_u32("out", expr_ident("msg_len".to_string())),
        expr_list(vec![
            expr_ident("set"),
            expr_ident("out".to_string()),
            expr_list(vec![
                expr_ident("vec_u8.extend_bytes"),
                expr_ident("out".to_string()),
                expr_ident("msg".to_string()),
            ]),
        ]),
        extend_u32("out", expr_ident("pl_len".to_string())),
        expr_list(vec![
            expr_ident("set"),
            expr_ident("out".to_string()),
            expr_list(vec![
                expr_ident("vec_u8.extend_bytes"),
                expr_ident("out".to_string()),
                expr_ident("plb".to_string()),
            ]),
        ]),
        expr_list(vec![
            expr_ident("vec_u8.into_bytes"),
            expr_ident("out".to_string()),
        ]),
    ])
}

fn extend_u32(out: &str, x: Expr) -> Expr {
    expr_list(vec![
        expr_ident("set"),
        expr_ident(out.to_string()),
        expr_list(vec![
            expr_ident("vec_u8.extend_bytes"),
            expr_ident(out.to_string()),
            expr_list(vec![expr_ident("codec.write_u32_le"), x]),
        ]),
    ])
}

fn param_ident(idx: usize) -> Expr {
    expr_ident(format!("p{idx}"))
}

fn let_i32(name: &str, value: i32) -> Expr {
    expr_list(vec![
        expr_ident("let"),
        expr_ident(name.to_string()),
        expr_int(value),
    ])
}

fn set_add_i32(name: &str, delta: Expr) -> Expr {
    expr_list(vec![
        expr_ident("set"),
        expr_ident(name.to_string()),
        expr_list(vec![expr_ident("+"), expr_ident(name.to_string()), delta]),
    ])
}

fn emit_net_sink_validate_caps(items: &mut Vec<Expr>) {
    // Minimal caps validation (strict): len>=24, ver==1, reserved==0, max_write_bytes>0.
    items.push(expr_list(vec![
        expr_ident("if"),
        expr_list(vec![
            expr_ident("<"),
            expr_list(vec![
                expr_ident("view.len"),
                expr_ident("net_sink_caps".to_string()),
            ]),
            expr_int(24),
        ]),
        expr_list(vec![
            expr_ident("return"),
            err_doc_const(E_NET_CAPS_INVALID, "stream:net_caps_invalid"),
        ]),
        expr_int(0),
    ]));
    items.push(expr_list(vec![
        expr_ident("if"),
        expr_list(vec![
            expr_ident("!="),
            expr_list(vec![
                expr_ident("codec.read_u32_le"),
                expr_ident("net_sink_caps".to_string()),
                expr_int(0),
            ]),
            expr_int(1),
        ]),
        expr_list(vec![
            expr_ident("return"),
            err_doc_const(E_NET_CAPS_INVALID, "stream:net_caps_invalid"),
        ]),
        expr_int(0),
    ]));
    items.push(expr_list(vec![
        expr_ident("if"),
        expr_list(vec![
            expr_ident("!="),
            expr_list(vec![
                expr_ident("codec.read_u32_le"),
                expr_ident("net_sink_caps".to_string()),
                expr_int(20),
            ]),
            expr_int(0),
        ]),
        expr_list(vec![
            expr_ident("return"),
            err_doc_const(E_NET_CAPS_INVALID, "stream:net_caps_invalid"),
        ]),
        expr_int(0),
    ]));
    items.push(expr_list(vec![
        expr_ident("let"),
        expr_ident("net_sink_mw".to_string()),
        expr_list(vec![
            expr_ident("codec.read_u32_le"),
            expr_ident("net_sink_caps".to_string()),
            expr_int(16),
        ]),
    ]));
    items.push(expr_list(vec![
        expr_ident("if"),
        expr_list(vec![
            expr_ident("<="),
            expr_ident("net_sink_mw".to_string()),
            expr_int(0),
        ]),
        expr_list(vec![
            expr_ident("return"),
            err_doc_const(E_NET_CAPS_INVALID, "stream:net_caps_invalid"),
        ]),
        expr_int(0),
    ]));
}

fn emit_net_sink_init_limits_and_buffer(
    items: &mut Vec<Expr>,
    pipe_max_out_bytes: i32,
    cfg: NetTcpWriteStreamHandleCfgV1,
) {
    items.push(let_i32("net_sink_flushes", 0));
    items.push(let_i32("net_sink_write_calls", 0));

    // Derive max_flushes/max_write_calls when 0.
    let b = pipe_max_out_bytes;
    let f = cfg.flush_min_bytes.max(1);
    let derived_max_flushes = if cfg.max_flushes > 0 {
        cfg.max_flushes
    } else {
        ((b + f - 1) / f) + 4
    };
    items.push(let_i32("net_sink_max_flushes", derived_max_flushes.max(1)));
    items.push(expr_list(vec![
        expr_ident("let"),
        expr_ident("net_sink_max_write_calls".to_string()),
        expr_list(vec![
            expr_ident("if"),
            expr_list(vec![
                expr_ident(">"),
                expr_int(cfg.max_write_calls),
                expr_int(0),
            ]),
            expr_int(cfg.max_write_calls),
            expr_list(vec![
                expr_ident("begin"),
                expr_list(vec![
                    expr_ident("let"),
                    expr_ident("d".to_string()),
                    expr_list(vec![
                        expr_ident("if"),
                        expr_list(vec![
                            expr_ident("<u"),
                            expr_ident("net_sink_mw".to_string()),
                            expr_int(f),
                        ]),
                        expr_ident("net_sink_mw".to_string()),
                        expr_int(f),
                    ]),
                ]),
                expr_list(vec![
                    expr_ident("+"),
                    expr_list(vec![
                        expr_ident("/"),
                        expr_list(vec![
                            expr_ident("+"),
                            expr_int(b),
                            expr_list(vec![
                                expr_ident("-"),
                                expr_ident("d".to_string()),
                                expr_int(1),
                            ]),
                        ]),
                        expr_ident("d".to_string()),
                    ]),
                    expr_int(8),
                ]),
            ]),
        ]),
    ]));

    items.push(expr_list(vec![
        expr_ident("let"),
        expr_ident("net_sink_buf".to_string()),
        expr_list(vec![
            expr_ident("vec_u8.with_capacity"),
            expr_int(cfg.buf_cap_bytes),
        ]),
    ]));
}

fn parse_expr_v1(
    params: &mut Vec<PipeParam>,
    ty: Ty,
    wrapper: &Expr,
) -> Result<usize, CompilerError> {
    let Expr::List { items, .. } = wrapper else {
        return Err(CompilerError::new(
            CompileErrorKind::Typing,
            "std.stream.expr_v1 wrapper must be a list".to_string(),
        ));
    };
    if items.first().and_then(Expr::as_ident) != Some("std.stream.expr_v1") || items.len() != 2 {
        return Err(CompilerError::new(
            CompileErrorKind::Typing,
            "expected std.stream.expr_v1 wrapper".to_string(),
        ));
    }
    let expr = items[1].clone();
    let idx = params.len();
    params.push(PipeParam { ty, expr });
    Ok(idx)
}

fn parse_fn_v1(wrapper: &Expr) -> Result<String, CompilerError> {
    let Expr::List { items, .. } = wrapper else {
        return Err(CompilerError::new(
            CompileErrorKind::Typing,
            "std.stream.fn_v1 wrapper must be a list".to_string(),
        ));
    };
    if items.first().and_then(Expr::as_ident) != Some("std.stream.fn_v1") || items.len() != 2 {
        return Err(CompilerError::new(
            CompileErrorKind::Typing,
            "expected std.stream.fn_v1 wrapper".to_string(),
        ));
    }
    let Some(fn_id) = items[1].as_ident() else {
        return Err(CompilerError::new(
            CompileErrorKind::Typing,
            "std.stream.fn_v1 expects a function identifier".to_string(),
        ));
    };
    Ok(fn_id.to_string())
}

fn parse_kv_fields(head: &str, tail: &[Expr]) -> Result<BTreeMap<String, Expr>, CompilerError> {
    let mut out = BTreeMap::new();
    for item in tail {
        let Expr::List { items: kv, .. } = item else {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("{head} fields must be pairs"),
            ));
        };
        if kv.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("{head} fields must be pairs"),
            ));
        }
        let Some(key) = kv[0].as_ident() else {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("{head} field key must be an identifier"),
            ));
        };
        if out.insert(key.to_string(), kv[1].clone()).is_some() {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("{head} has duplicate field: {key}"),
            ));
        }
    }
    Ok(out)
}

fn hash_pipe_without_expr_bodies(pipe_expr: &Expr) -> Result<String, CompilerError> {
    let mut v = x07ast::expr_to_value(pipe_expr);
    scrub_expr_bodies(&mut v);
    canon_stream_descriptor_pairs(&mut v);
    x07ast::canon_value_jcs(&mut v);
    let bytes = serde_json::to_vec(&v).map_err(|e| {
        CompilerError::new(
            CompileErrorKind::Internal,
            format!("internal error: failed to serialize pipe descriptor: {e}"),
        )
    })?;

    let digest = blake3::hash(&bytes);
    let b = digest.as_bytes();
    Ok(format!("{:02x}{:02x}{:02x}{:02x}", b[0], b[1], b[2], b[3]))
}

fn scrub_expr_bodies(v: &mut Value) {
    match v {
        Value::Array(items) => {
            if items.len() == 2 && items[0].as_str() == Some("std.stream.expr_v1") {
                items[1] = Value::Null;
                return;
            }
            for item in items {
                scrub_expr_bodies(item);
            }
        }
        Value::Object(map) => {
            for value in map.values_mut() {
                scrub_expr_bodies(value);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

fn canon_stream_descriptor_pairs(v: &mut Value) {
    match v {
        Value::Array(items) => {
            if let Some(head) = items.first().and_then(Value::as_str) {
                match head {
                    "std.stream.cfg_v1"
                    | "std.stream.sink.net_tcp_connect_write_v1"
                    | "std.stream.sink.net_tcp_write_u32frames_v1"
                    | "std.stream.sink.net_tcp_write_stream_handle_v1"
                    | "std.stream.src.net_tcp_read_stream_handle_v1"
                    | "std.stream.xf.deframe_u32le_v1"
                    | "std.stream.xf.json_canon_stream_v1" => {
                        canon_pair_tail(items, 1);
                    }
                    "std.stream.xf.map_in_place_buf_v1" => canon_map_in_place_buf(items),
                    "std.stream.sink.world_fs_write_stream_v1"
                    | "std.stream.sink.world_fs_write_stream_hash_fnv1a32_v1" => {
                        canon_pair_tail(items, 3);
                    }
                    _ => {}
                }
            }

            for item in items {
                canon_stream_descriptor_pairs(item);
            }
        }
        Value::Object(map) => {
            for value in map.values_mut() {
                canon_stream_descriptor_pairs(value);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

fn canon_map_in_place_buf(items: &mut Vec<Value>) {
    // Canonicalize v1.1 positional mix:
    //   [head, ["scratch_cap_bytes",...], ["std.stream.fn_v1",...], ["clear_before_each",...]?]
    // Also accepts the keyed form: ["fn", ["std.stream.fn_v1",...]].
    if items.is_empty() {
        return;
    }
    let mut scratch: Option<Value> = None;
    let mut f: Option<Value> = None;
    let mut clear: Option<Value> = None;
    let mut rest: Vec<Value> = Vec::new();

    for v in items.iter().skip(1) {
        let Some(arr) = v.as_array() else {
            rest.push(v.clone());
            continue;
        };
        match arr.first().and_then(Value::as_str) {
            Some("scratch_cap_bytes") if arr.len() == 2 => {
                if scratch.is_none() {
                    scratch = Some(v.clone());
                } else {
                    rest.push(v.clone());
                }
            }
            Some("clear_before_each") if arr.len() == 2 => {
                if clear.is_none() {
                    clear = Some(v.clone());
                } else {
                    rest.push(v.clone());
                }
            }
            Some("fn") if arr.len() == 2 => {
                if f.is_none() {
                    f = Some(arr[1].clone());
                } else {
                    rest.push(v.clone());
                }
            }
            Some("std.stream.fn_v1") => {
                if f.is_none() {
                    f = Some(v.clone());
                } else {
                    rest.push(v.clone());
                }
            }
            _ => rest.push(v.clone()),
        }
    }

    let mut out: Vec<Value> = Vec::new();
    out.push(items[0].clone());
    if let Some(v) = scratch {
        out.push(v);
    }
    if let Some(v) = f {
        out.push(v);
    }
    if let Some(v) = clear {
        out.push(v);
    }
    out.extend(rest);
    *items = out;
}

fn canon_pair_tail(items: &mut Vec<Value>, start: usize) {
    if items.len() <= start {
        return;
    }
    let mut head: Vec<Value> = items[..start].to_vec();
    let mut tail: Vec<Value> = items[start..].to_vec();
    tail.sort_by(|a, b| {
        let ak = a
            .as_array()
            .and_then(|a| a.first())
            .and_then(Value::as_str)
            .unwrap_or("");
        let bk = b
            .as_array()
            .and_then(|a| a.first())
            .and_then(Value::as_str)
            .unwrap_or("");
        ak.as_bytes().cmp(bk.as_bytes())
    });
    head.extend(tail);
    *items = head;
}

fn function_module_id(full_name: &str) -> Result<&str, CompilerError> {
    let Some((module_id, _name)) = full_name.rsplit_once('.') else {
        return Err(CompilerError::new(
            CompileErrorKind::Internal,
            format!("internal error: function name missing module prefix: {full_name:?}"),
        ));
    };
    Ok(module_id)
}

fn expect_i32(expr: &Expr, message: &str) -> Result<i32, CompilerError> {
    match expr {
        Expr::Int { value, .. } => Ok(*value),
        _ => Err(CompilerError::new(
            CompileErrorKind::Typing,
            message.to_string(),
        )),
    }
}

fn expr_ident(name: impl Into<String>) -> Expr {
    Expr::Ident {
        name: name.into(),
        ptr: String::new(),
    }
}

fn expr_int(value: i32) -> Expr {
    Expr::Int {
        value,
        ptr: String::new(),
    }
}

fn expr_list(items: Vec<Expr>) -> Expr {
    Expr::List {
        items,
        ptr: String::new(),
    }
}
