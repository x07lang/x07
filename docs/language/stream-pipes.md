# Streaming pipes (`std.stream.pipe_v1`)

`std.stream.pipe_v1` is a compiler special form for deterministic, budgeted streaming pipelines: **source → transducers → sink**.

Pipes are represented in x07AST as descriptor lists and are elaborated deterministically by the compiler.

## AST shape

```jsonc
[
  "std.stream.pipe_v1",
  <cfg_v1>,
  <source_v1>,
  <chain_v1>,
  <sink_v1>
]
```

- `<cfg_v1>` MUST be `["std.stream.cfg_v1", ...]`
- `<source_v1>` MUST be a `std.stream.src.*_v1` descriptor
- `<chain_v1>` MUST be `["std.stream.chain_v1", <xf>*]` (may be empty)
- `<sink_v1>` MUST be a `std.stream.sink.*_v1` descriptor

Runtime-evaluated expressions inside descriptors are wrapped explicitly:

- `["std.stream.expr_v1", <x07Expr>]` (evaluated once at pipe init)
- `["std.stream.fn_v1", "<module_id>.<fn_name>"]` (compile-time function reference)

## Result doc format

`std.stream.pipe_v1` returns a bytes “doc”:

### OK doc

```
byte  tag         = 1
u32   bytes_in
u32   bytes_out
u32   items_in
u32   items_out
u32   payload_len
bytes payload[payload_len]
```

### ERR doc

```
byte  tag         = 0
u32   err_code
u32   msg_len
bytes msg[msg_len]
u32   payload_len
bytes payload[payload_len]
```

To read `err_code`:

```jsonc
["begin",
  ["let","dv",["bytes.view","doc"]],
  ["if",["=",["view.get_u8","dv",0],0],
    ["codec.read_u32_le","dv",1],
    0
  ]
]
```

## Config (`std.stream.cfg_v1`)

```jsonc
["std.stream.cfg_v1",
  ["chunk_max_bytes", <i32>],
  ["bufread_cap_bytes", <i32>],
  ["max_in_bytes", <u32>],
  ["max_out_bytes", <u32>],
  ["max_items", <u32>],

  // optional:
  ["max_steps", <u32>],
  ["emit_payload", <i32>],
  ["emit_stats", <i32>],
  ["allow_nondet_v1", <i32>],

  // optional brand typing (v1):
  ["typecheck_item_brands_v1", <i32>],           // default 1
  ["auto_require_brand_v1", <i32>],              // default 0
  ["brand_registry_ref_v1", ["bytes.lit","<module_id>"]],
  ["verify_produced_brands_v1", <i32>]           // default 0
]
```

Budgets are hard limits: exceeding a budget terminates the pipeline with an `ERR` doc.

`allow_nondet_v1`:

- `0` (default): reject nondeterministic pipeline stages (for example unordered parallelism).
- `1`: allow explicitly nondeterministic stages (currently: unordered `par_map_stream_*_v1`).

`typecheck_item_brands_v1`:

- `1` (default): the elaborator computes and checks a compile-time **item brand** along the pipeline.
- `0`: disables item brand typechecking (but `require_brand_v1` still resolves its validator via the registry).

`auto_require_brand_v1` (only when `typecheck_item_brands_v1=1`):

- `0` (default): brand mismatches are compile errors.
- `1`: on a mismatch, the elaborator inserts a `std.stream.xf.require_brand_v1` stage (using the brand registry).

`brand_registry_ref_v1`:

- Optional `bytes` literal containing a module id to use as the brand registry source.
- If omitted, the pipeline uses `meta.brands_v1` from the defining module, if present.

`verify_produced_brands_v1` (only when `typecheck_item_brands_v1=1`):

- `0` (default): trust `out_item_brand` declarations and brand-producing stages.
- `1`: insert `std.stream.xf.require_brand_v1` validators for any source/stage that *claims* it produces branded items (runtime cost).

## Item brands (v1) (compile-time only)

Pipelines process items as `bytes_view`. Item brands extend this with a nominal tag:

- `bytes_view@<brand_id>` means each item is a complete, valid instance of that encoding.
- Brands exist only at compile time; runtime items are still `bytes_view`.

### Source item brand (`out_item_brand`)

Sources may declare a brand for the items they emit:

```jsonc
["std.stream.src.fs_open_read_v1",
  ["std.stream.expr_v1", ["bytes.lit","input.txt"]],
  ["out_item_brand", "my.encoding_v1"]
]
```

Important: a source should only declare `out_item_brand` if its contract guarantees every item is valid for that brand.

### Stage item brand (`in_item_brand` / `out_item_brand`)

Most stages accept optional item brand contracts:

```jsonc
["std.stream.xf.map_bytes_v1",
  ["std.stream.fn_v1", "main.map_one"],
  ["in_item_brand", "same"],
  ["out_item_brand", "none"]
]
```

`in_item_brand` is one of:

- `any`: accept unbranded or branded items
- `same`: require the current brand (if any)
- `<brand_id>`: require exactly that brand

`out_item_brand` is one of:

- `same`: preserve the current brand
- `none`: drop the brand
- `<brand_id>`: claim the stage produces valid items of that brand

Brand inference:

- `std.stream.xf.map_bytes_v1` infers input/output brands from the referenced `defn` signature (bytes_view param brand and bytes result `result_brand`).
- `std.stream.xf.par_map_stream_*_v1` infers brands from the referenced `defasync` signature (owned bytes param brand and bytes/result_bytes `result_brand`).

### Sink item brand (`in_item_brand`)

Sinks may require branded input:

```jsonc
["std.stream.sink.null_v1",
  ["in_item_brand", "std.text.slices.x7sl_v1"]
]
```

Sinks accept:

- `any`
- `<brand_id>`

## Brand validation stage: `std.stream.xf.require_brand_v1`

This stage validates each item and brands it (no copies).

Descriptor:

```jsonc
["std.stream.xf.require_brand_v1",
  ["brand", "std.text.slices.x7sl_v1"],
  ["validator", "std.text.slices.validate_v1"],
  ["max_item_bytes", 1048576]
]
```

- `brand` (required): the brand id to attach to each item.
- `validator` (optional): a `defn` symbol with signature `(bytes_view) -> result_i32`.
  - If omitted, the elaborator resolves it via `meta.brands_v1` (from `brand_registry_ref_v1` or the defining module).
- `max_item_bytes` (optional, default `0`): additional per-item size check (`0` = no extra check).

## Brand registry (`meta.brands_v1`)

To support `require_brand_v1` validator resolution and `auto_require_brand_v1`, a module may provide a registry:

```jsonc
"meta": {
  "brands_v1": {
    "std.text.slices.x7sl_v1": {
      "validate": "std.text.slices.validate_v1",
      "doc": "docs/text/x7sl-v1.md"
    }
  }
}
```

## Sources (`std.stream.src.*_v1`)

- `std.stream.src.bytes_v1` (single-item source)
- `std.stream.src.fs_open_read_v1` (filesystem reader; requires fs/OS worlds)
- `std.stream.src.rr_send_v1` (RR fixture streaming; requires rr/OS worlds)
- `std.stream.src.db_rows_doc_v1` (DB rows; OS worlds)
- `std.stream.src.net_tcp_read_stream_handle_v1` (OS worlds)
- `std.stream.src.net_tcp_read_u32frames_v1` (OS worlds; convenience source)

## Transducers (`std.stream.xf.*_v1`)

- `std.stream.xf.map_bytes_v1` (map `bytes_view -> bytes`)
- `std.stream.xf.require_brand_v1` (validate items and attach an item brand)
- `std.stream.xf.map_in_place_buf_v1` (scratch-buffer map; fixed-capacity)
- `std.stream.xf.filter_v1` (predicate `bytes_view -> i32`)
- `std.stream.xf.take_v1` (stop after N items)
- `std.stream.xf.split_lines_v1` (split text into line items)
- `std.stream.xf.frame_u32le_v1` (prefix each item with a u32 length)
- `std.stream.xf.deframe_u32le_v1` (parse u32-length frames from raw chunks)
- `std.stream.xf.json_canon_stream_v1` (RFC 8785 JCS canonicalization)
- `std.stream.xf.par_map_stream_v1` (parallel map; structured concurrency)

### Parallel map (`par_map_stream_*_v1`)

Parallel map runs a `defasync` mapper concurrently inside a structured concurrency scope.

Stage variants:

- `std.stream.xf.par_map_stream_v1` (ordered; mapper returns `bytes`)
- `std.stream.xf.par_map_stream_result_bytes_v1` (ordered; mapper returns `result_bytes`)
- `std.stream.xf.par_map_stream_unordered_v1` (unordered; mapper returns `bytes`; requires `allow_nondet_v1=1`)
- `std.stream.xf.par_map_stream_unordered_result_bytes_v1` (unordered; mapper returns `result_bytes`; requires `allow_nondet_v1=1`)

Shape:

```jsonc
["std.stream.xf.par_map_stream_v1",
  ["max_inflight", <i32>],
  ["max_item_bytes", <i32>],

  // optional:
  ["max_inflight_in_bytes", <i32>],
  ["max_out_item_bytes", <i32>],
  ["ctx", ["std.stream.expr_v1", <bytes_expr>]],
  ["mapper_defasync", ["std.stream.fn_v1", "<module_id>.<defasync_name>"]],
  ["scope_cfg", ["task.scope.cfg_v1", ...]]
]
```

Notes:

- Ordered variants preserve input order.
- Unordered variants may emit in completion order and are intentionally nondeterministic.
- `scope_cfg` controls the `task.scope_v1` budgets used by the stage (defaults to `["task.scope.cfg_v1"]`).

## Sinks (`std.stream.sink.*_v1`)

- `std.stream.sink.collect_bytes_v1`
- `std.stream.sink.hash_fnv1a32_v1`
- `std.stream.sink.null_v1`
- `std.stream.sink.world_fs_write_file_v1` (fs/OS worlds)
- `std.stream.sink.world_fs_write_stream_v1` (fs/OS worlds; streaming sink)
- `std.stream.sink.world_fs_write_stream_hash_fnv1a32_v1` (fs/OS worlds)
- `std.stream.sink.u32frames_v1` (wraps another sink; frames output)
- `std.stream.sink.net_tcp_connect_write_v1` (OS worlds; owned connection)
- `std.stream.sink.net_tcp_write_stream_handle_v1` (OS worlds)
- `std.stream.sink.net_tcp_write_u32frames_v1` (OS worlds)

## Pipe error codes (stable)

- `1`: invalid config / invalid descriptor
- `2`: input bytes budget exceeded
- `3`: output bytes budget exceeded
- `4`: max items budget exceeded
- `5`: `split_lines_v1` line too long
- `7`: DB query failed
- `8`: `map_in_place_buf_v1` scratch overflow
- `9`: user stage failed (`map_bytes_v1` / `map_in_place_buf_v1`)
- `10`: framing stage produced an oversized frame
- `20..=24`: JSON canonicalization errors (syntax / I-JSON / depth / object size / trailing data)
- `40..=43`: filesystem sink errors
- `60..=66`: network sink/source errors
- `70`: `require_brand_v1` item too large
- `71`: `require_brand_v1` validator failed
- `80..=84`: `deframe_u32le_v1` errors (oversize / truncated / empty / max frames / timeout)
- `100`: `par_map_stream_*_v1` item too large (input item exceeded `max_item_bytes` / `max_inflight_in_bytes`)
- `101`: `par_map_stream_*_v1` output item too large (output exceeded `max_out_item_bytes`)
- `102`: `par_map_stream_result_bytes_v1` / `par_map_stream_unordered_result_bytes_v1` child returned `result_bytes` error
- `103`: `par_map_stream_result_bytes_v1` / `par_map_stream_unordered_result_bytes_v1` child was canceled

For `71`, the `ERR` doc payload is:

```
u32  brand_len
bytes brand_id[brand_len]
u32  validator_len
bytes validator_id[validator_len]
u32  validator_err_code
u32  item_index
```

For `102` and `103`, the `ERR` doc payload is:

```
u32  child_err_code
u32  item_index
```
