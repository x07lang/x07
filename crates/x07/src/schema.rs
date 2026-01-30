use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Args;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use x07c::ast::Expr;
use x07c::program::{FunctionDef, FunctionParam};
use x07c::types::Ty;
use x07c::x07ast::{X07AstFile, X07AstKind};

use crate::util;

const DEFAULT_MAX_DEPTH: i32 = 64;
const DEFAULT_MAX_SEQ_ITEMS: i32 = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SchemaVersion {
    SpecRows010,
    SpecRows020,
}

fn parse_schema_version(s: &str) -> Result<SchemaVersion> {
    match s.trim() {
        "x07schema.specrows@0.1.0" => Ok(SchemaVersion::SpecRows010),
        "x07schema.specrows@0.2.0" => Ok(SchemaVersion::SpecRows020),
        other => anyhow::bail!(
            "unsupported schema_version: expected \"x07schema.specrows@0.1.0\" or \"x07schema.specrows@0.2.0\" got {other:?}"
        ),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NumberStyleV1 {
    IntAsciiV1,
    UIntAsciiV1,
}

fn parse_number_style_v1(s: &str) -> Result<NumberStyleV1> {
    match s.trim() {
        "int_ascii_v1" => Ok(NumberStyleV1::IntAsciiV1),
        "uint_ascii_v1" => Ok(NumberStyleV1::UIntAsciiV1),
        other => anyhow::bail!(
            "unsupported number_style_v1: expected \"int_ascii_v1\" or \"uint_ascii_v1\" got {other:?}"
        ),
    }
}

#[derive(Debug, Args)]
pub struct SchemaArgs {
    #[command(subcommand)]
    pub cmd: Option<SchemaCommand>,
}

#[derive(clap::Subcommand, Debug)]
pub enum SchemaCommand {
    /// Derive schema modules from a x07schema JSON file.
    Derive(SchemaDeriveArgs),
}

#[derive(Debug, Args)]
pub struct SchemaDeriveArgs {
    #[arg(long, value_name = "PATH")]
    pub input: PathBuf,

    #[arg(long, value_name = "DIR")]
    pub out_dir: PathBuf,

    #[arg(long)]
    pub write: bool,

    #[arg(long)]
    pub check: bool,

    #[arg(long)]
    pub report_json: bool,
}

pub fn cmd_schema(args: SchemaArgs) -> Result<std::process::ExitCode> {
    let Some(cmd) = args.cmd else {
        anyhow::bail!("missing schema subcommand (try --help)");
    };
    match cmd {
        SchemaCommand::Derive(args) => cmd_schema_derive(args),
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct X07SchemaFile {
    schema_version: String,
    package: SchemaPackage,
    defaults: SchemaDefaults,
    #[serde(default)]
    types: Vec<SchemaType>,
    #[serde(default)]
    rows: Vec<Vec<Value>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct SchemaPackage {
    name: String,
    version: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct SchemaDefaults {
    #[serde(default)]
    codec: Option<String>,
    #[serde(default)]
    budgets: Option<SchemaDefaultBudgets>,
    #[serde(default)]
    canon_v1: Option<SchemaCanonV1>,
    #[serde(default)]
    number_style_default_v1: Option<String>,
    #[serde(default)]
    max_doc_bytes: Option<i32>,
    #[serde(default)]
    max_map_entries: Option<i32>,
    #[serde(default)]
    max_number_bytes: Option<i32>,
    #[serde(default)]
    max_string_bytes: Option<i32>,
    #[serde(default)]
    allow_unknown_fields: Option<bool>,
    #[serde(default)]
    max_depth: Option<i32>,
    #[serde(default)]
    max_seq_items: Option<i32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct SchemaCanonV1 {
    #[serde(default)]
    mode: Option<String>,
    #[serde(default)]
    map_key_order: Option<String>,
    #[serde(default)]
    unknown_fields: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct SchemaDefaultBudgets {
    max_doc_bytes: i32,
    max_depth: i32,
    max_map_entries: i32,
    max_seq_items: i32,
    max_string_bytes: i32,
    max_number_bytes: i32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct SchemaBudgets {
    max_doc_bytes: Option<i32>,
    max_map_entries: Option<i32>,
    #[serde(default)]
    max_number_bytes: Option<i32>,
    #[serde(default)]
    max_string_bytes: Option<i32>,
    allow_unknown_fields: Option<bool>,
    #[serde(default)]
    max_depth: Option<i32>,
    #[serde(default)]
    max_seq_items: Option<i32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct SchemaType {
    type_id: String,
    version: i32,
    kind: String,
    err_base: i32,
    budgets: Option<SchemaBudgets>,
    #[serde(default)]
    fields: Vec<SchemaField>,
    #[serde(default)]
    variants: Vec<SchemaVariant>,
    #[serde(default)]
    examples: Vec<SchemaExample>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct SchemaVariant {
    id: i32,
    name: String,
    payload_ty: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct SchemaField {
    id: i32,
    name: String,
    ty: String,
    required: bool,
    max_bytes: Option<i32>,
    #[serde(default)]
    max_items: Option<i32>,
    #[serde(default)]
    number_style_v1: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct SchemaExample {
    name: String,
    value: Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum FieldTy {
    Bool,
    Number,
    Bytes,
    Struct { type_id: String },
    Seq { elem: Box<FieldTy> },
}

impl FieldTy {
    fn parse(s: &str) -> Result<Self> {
        let s = s.trim();
        if let Some(rest) = s.strip_prefix("struct:") {
            let type_id = rest.trim();
            if type_id.is_empty() {
                anyhow::bail!("unsupported field ty: {s:?} (missing type_id)");
            }
            return Ok(FieldTy::Struct {
                type_id: type_id.to_string(),
            });
        }
        if let Some(rest) = s.strip_prefix("seq:") {
            let elem_s = rest.trim();
            if elem_s.is_empty() {
                anyhow::bail!("unsupported field ty: {s:?} (missing elem ty)");
            }
            let elem = FieldTy::parse(elem_s)?;
            if matches!(elem, FieldTy::Seq { .. }) {
                anyhow::bail!("unsupported field ty: {s:?} (nested seq)");
            }
            return Ok(FieldTy::Seq {
                elem: Box::new(elem),
            });
        }
        match s {
            "number" => Ok(FieldTy::Number),
            "bytes" => Ok(FieldTy::Bytes),
            "bool" => Ok(FieldTy::Bool),
            _ => anyhow::bail!(
                "unsupported field ty: {s:?} (expected \"number\", \"bytes\", \"bool\", \"struct:<type_id>\", or \"seq:<elem_ty>\")"
            ),
        }
    }

    fn kind_byte(&self) -> i32 {
        match self {
            FieldTy::Bool => 1,
            FieldTy::Number => 2,
            FieldTy::Bytes => 3,
            FieldTy::Seq { .. } => 4,
            FieldTy::Struct { .. } => 5,
        }
    }

    fn value_ctor(&self) -> Option<&'static str> {
        match self {
            FieldTy::Bool => Some("ext.data_model.value_bool"),
            FieldTy::Number => Some("ext.data_model.value_number"),
            FieldTy::Bytes => Some("ext.data_model.value_string"),
            FieldTy::Struct { .. } | FieldTy::Seq { .. } => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TypeKind {
    Struct,
    Enum,
}

#[derive(Debug, Clone)]
struct TypeDef {
    module_id: String,
    tests_module_id: String,
    type_id: String,
    version: i32,
    kind: TypeKind,
    err_base: i32,
    max_doc_bytes: i32,
    max_map_entries: i32,
    max_depth: i32,
    max_seq_items: i32,
    allow_unknown_fields: bool,
    schema_version: SchemaVersion,
    number_style_default_v1: Option<NumberStyleV1>,
    variants: Vec<VariantDef>,
    fields: Vec<FieldDef>,
    examples: Vec<ExampleDef>,
}

#[derive(Debug, Clone)]
struct VariantDef {
    id: i32,
    name: String,
    payload: VariantPayloadDef,
}

#[derive(Debug, Clone)]
enum VariantPayloadDef {
    Unit,
    Value {
        ty: FieldTy,
        max_bytes: Option<i32>,
        max_items: Option<i32>,
    },
}

#[derive(Debug, Clone)]
struct FieldDef {
    id: i32,
    name: String,
    ty: FieldTy,
    required: bool,
    max_bytes: Option<i32>,
    max_items: Option<i32>,
    number_style: Option<NumberStyleV1>,
}

#[derive(Debug, Clone)]
struct ExampleDef {
    name: String,
    kind: ExampleKind,
}

#[derive(Debug, Clone)]
enum ExampleKind {
    Struct {
        values: BTreeMap<String, ExampleValue>,
    },
    Enum {
        variant: String,
        payload: Option<ExampleValue>,
    },
}

#[derive(Debug, Clone)]
enum ExampleValue {
    Bytes(String),
    Bool(bool),
    Struct(BTreeMap<String, ExampleValue>),
    Seq(Vec<ExampleValue>),
}

#[derive(Debug, Clone, Serialize)]
struct SchemaDeriveReport {
    schema_version: &'static str,
    tool: SchemaDeriveTool,
    input: SchemaDeriveInput,
    outputs: Vec<SchemaDeriveOutput>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    diags: Vec<SchemaDeriveDiag>,
}

#[derive(Debug, Clone, Serialize)]
struct SchemaDeriveTool {
    name: &'static str,
    version: String,
}

#[derive(Debug, Clone, Serialize)]
struct SchemaDeriveInput {
    path: String,
    sha256_hex: String,
    jcs_sha256_hex: String,
    schema_version: String,
}

#[derive(Debug, Clone, Serialize)]
struct SchemaDeriveOutput {
    path: String,
    sha256_hex: String,
    kind: String,
}

#[derive(Debug, Clone, Serialize)]
struct SchemaDeriveDiag {
    severity: String,
    code: String,
    message: String,
}

#[derive(Debug, Clone)]
struct GeneratedOutput {
    rel_path: PathBuf,
    kind: &'static str,
    bytes: Vec<u8>,
}

#[derive(Debug, Clone)]
struct TypeInfo {
    module_id: String,
    kind: TypeKind,
    variants: Vec<VariantDef>,
    fields: Vec<FieldDef>,
}

type TypeIndex = BTreeMap<String, TypeInfo>;

fn cmd_schema_derive(args: SchemaDeriveArgs) -> Result<std::process::ExitCode> {
    if args.write && args.check {
        anyhow::bail!("set at most one of --write or --check");
    }

    let input_path = util::resolve_existing_path_upwards(&args.input);
    let input_bytes =
        std::fs::read(&input_path).with_context(|| format!("read: {}", input_path.display()))?;
    let raw_sha256_hex = util::sha256_hex(&input_bytes);

    let input_value: Value = serde_json::from_slice(&input_bytes).context("parse schema JSON")?;
    let jcs_sha256_hex = util::sha256_hex(&canonical_jcs_bytes(&input_value)?);

    let mut schema_file: X07SchemaFile =
        serde_json::from_value(input_value.clone()).context("parse schema JSON")?;
    let schema_version = parse_schema_version(&schema_file.schema_version)?;

    if !schema_file.rows.is_empty() {
        if !schema_file.types.is_empty() {
            anyhow::bail!("schema must set exactly one of \"types\" or \"rows\"");
        }
        schema_file.types = types_from_rows(&schema_file).context("parse schema rows")?;
    } else if schema_file.types.is_empty() {
        anyhow::bail!("schema missing \"types\" or \"rows\"");
    }

    let types = normalize_schema(schema_version, &schema_file).context("validate schema")?;
    let type_index = build_type_index(&types)?;

    let mut outputs: Vec<GeneratedOutput> = Vec::new();
    for td in &types {
        outputs.extend(generate_type_outputs(
            &schema_file.package.name,
            &type_index,
            td,
        )?);
    }
    outputs.push(generate_tests_manifest(&schema_file.package.name, &types)?);

    let report = SchemaDeriveReport {
        schema_version: "x07.schema.derive.report@0.1.0",
        tool: SchemaDeriveTool {
            name: "x07",
            version: env!("CARGO_PKG_VERSION").to_string(),
        },
        input: SchemaDeriveInput {
            path: input_path.display().to_string(),
            sha256_hex: raw_sha256_hex,
            jcs_sha256_hex,
            schema_version: schema_file.schema_version.clone(),
        },
        outputs: outputs
            .iter()
            .map(|o| SchemaDeriveOutput {
                path: o.rel_path.display().to_string(),
                sha256_hex: util::sha256_hex(&o.bytes),
                kind: o.kind.to_string(),
            })
            .collect(),
        diags: Vec::new(),
    };

    let mut drifted: Vec<PathBuf> = Vec::new();
    for o in &outputs {
        let path = args.out_dir.join(&o.rel_path);
        match std::fs::read(&path) {
            Ok(existing) if existing == o.bytes => {}
            _ => drifted.push(o.rel_path.clone()),
        }
    }

    if args.write {
        for o in &outputs {
            let path = args.out_dir.join(&o.rel_path);
            util::write_atomic(&path, &o.bytes)
                .with_context(|| format!("write: {}", path.display()))?;
        }
    }

    if args.report_json {
        let mut v = serde_json::to_value(&report)?;
        x07c::x07ast::canon_value_jcs(&mut v);
        let mut bytes = serde_json::to_vec(&v)?;
        if bytes.last() != Some(&b'\n') {
            bytes.push(b'\n');
        }
        std::io::Write::write_all(&mut std::io::stdout(), &bytes).context("write stdout")?;
    } else if !drifted.is_empty() {
        for p in &drifted {
            eprintln!("schema derive drift: {}", p.display());
        }
    }

    if !drifted.is_empty() && !args.write {
        return Ok(std::process::ExitCode::from(1));
    }
    Ok(std::process::ExitCode::SUCCESS)
}

fn canonical_jcs_bytes(v: &Value) -> Result<Vec<u8>> {
    let mut v = v.clone();
    x07c::x07ast::canon_value_jcs(&mut v);
    let bytes = serde_json::to_vec(&v)?;
    Ok(bytes)
}

#[derive(Debug, Clone, Copy)]
struct ResolvedDefaults {
    max_doc_bytes: i32,
    max_map_entries: i32,
    max_number_bytes: i32,
    max_string_bytes: i32,
    allow_unknown_fields: bool,
    max_depth: i32,
    max_seq_items: i32,
    number_style_default_v1: Option<NumberStyleV1>,
}

fn resolve_defaults(
    schema_version: SchemaVersion,
    defaults: &SchemaDefaults,
) -> Result<ResolvedDefaults> {
    if let Some(codec) = &defaults.codec {
        if codec.trim() != "ext.data_model.doc_v1" {
            anyhow::bail!(
                "defaults.codec unsupported: expected \"ext.data_model.doc_v1\" got {:?}",
                codec
            );
        }
    }

    let budgets = defaults.budgets.as_ref();

    fn pick_required(field: &str, top: Option<i32>, nested: Option<i32>) -> Result<i32> {
        match (top, nested) {
            (Some(a), Some(b)) if a != b => {
                anyhow::bail!("defaults.{field} conflicts with defaults.budgets.{field}");
            }
            (Some(v), _) => Ok(v),
            (None, Some(v)) => Ok(v),
            (None, None) => anyhow::bail!(
                "defaults.{field} missing (expected defaults.{field} or defaults.budgets.{field})"
            ),
        }
    }

    fn pick_optional(field: &str, top: Option<i32>, nested: Option<i32>) -> Result<Option<i32>> {
        match (top, nested) {
            (Some(a), Some(b)) if a != b => {
                anyhow::bail!("defaults.{field} conflicts with defaults.budgets.{field}");
            }
            (Some(v), _) => Ok(Some(v)),
            (None, Some(v)) => Ok(Some(v)),
            (None, None) => Ok(None),
        }
    }

    let max_doc_bytes = pick_required(
        "max_doc_bytes",
        defaults.max_doc_bytes,
        budgets.map(|b| b.max_doc_bytes),
    )?;
    let max_map_entries = pick_required(
        "max_map_entries",
        defaults.max_map_entries,
        budgets.map(|b| b.max_map_entries),
    )?;
    let max_number_bytes = pick_required(
        "max_number_bytes",
        defaults.max_number_bytes,
        budgets.map(|b| b.max_number_bytes),
    )?;
    let max_string_bytes = pick_required(
        "max_string_bytes",
        defaults.max_string_bytes,
        budgets.map(|b| b.max_string_bytes),
    )?;

    let max_depth = pick_optional(
        "max_depth",
        defaults.max_depth,
        budgets.map(|b| b.max_depth),
    )?
    .unwrap_or(DEFAULT_MAX_DEPTH);

    let max_seq_items = pick_optional(
        "max_seq_items",
        defaults.max_seq_items,
        budgets.map(|b| b.max_seq_items),
    )?
    .unwrap_or(DEFAULT_MAX_SEQ_ITEMS);

    if max_doc_bytes <= 0 {
        anyhow::bail!("defaults.max_doc_bytes must be >= 1");
    }
    if max_map_entries <= 0 {
        anyhow::bail!("defaults.max_map_entries must be >= 1");
    }
    if max_number_bytes <= 0 {
        anyhow::bail!("defaults.max_number_bytes must be >= 1");
    }
    if max_string_bytes <= 0 {
        anyhow::bail!("defaults.max_string_bytes must be >= 1");
    }
    if max_depth <= 0 {
        anyhow::bail!("defaults.max_depth must be >= 1");
    }
    if max_seq_items <= 0 {
        anyhow::bail!("defaults.max_seq_items must be >= 1");
    }

    let number_style_default_v1 = match schema_version {
        SchemaVersion::SpecRows010 => {
            if defaults.canon_v1.is_some() {
                anyhow::bail!(
                    "defaults.canon_v1 requires schema_version \"x07schema.specrows@0.2.0\""
                );
            }
            if defaults.number_style_default_v1.is_some() {
                anyhow::bail!(
                    "defaults.number_style_default_v1 requires schema_version \"x07schema.specrows@0.2.0\""
                );
            }
            None
        }
        SchemaVersion::SpecRows020 => Some(parse_number_style_v1(
            defaults
                .number_style_default_v1
                .as_deref()
                .unwrap_or("int_ascii_v1"),
        )?),
    };

    let allow_unknown_fields = match schema_version {
        SchemaVersion::SpecRows010 => defaults.allow_unknown_fields.ok_or_else(|| {
            anyhow::anyhow!(
                "defaults.allow_unknown_fields missing (expected for schema_version \"x07schema.specrows@0.1.0\")"
            )
        })?,
        SchemaVersion::SpecRows020 => {
            let mut allow = defaults.allow_unknown_fields.unwrap_or(false);
            if let Some(c) = &defaults.canon_v1 {
                if let Some(map_key_order) = &c.map_key_order {
                    if map_key_order.trim() != "lex_u8_v1" {
                        anyhow::bail!(
                            "defaults.canon_v1.map_key_order unsupported: expected \"lex_u8_v1\" got {:?}",
                            map_key_order
                        );
                    }
                }
                if let Some(mode) = &c.mode {
                    match mode.trim() {
                        "strict_reject_v1" | "accept_and_canonize_v1" => {}
                        other => anyhow::bail!(
                            "defaults.canon_v1.mode unsupported: expected \"strict_reject_v1\" or \"accept_and_canonize_v1\" got {other:?}"
                        ),
                    }
                }
                if let Some(unknown_fields) = &c.unknown_fields {
                    allow = match unknown_fields.trim() {
                        "reject_v1" => false,
                        "allow_v1" => true,
                        other => anyhow::bail!(
                            "defaults.canon_v1.unknown_fields unsupported: expected \"reject_v1\" or \"allow_v1\" got {other:?}"
                        ),
                    };
                }
            }
            if let Some(explicit) = defaults.allow_unknown_fields {
                if explicit != allow {
                    anyhow::bail!("defaults.allow_unknown_fields conflicts with defaults.canon_v1.unknown_fields");
                }
            }
            allow
        }
    };

    Ok(ResolvedDefaults {
        max_doc_bytes,
        max_map_entries,
        max_number_bytes,
        max_string_bytes,
        allow_unknown_fields,
        max_depth,
        max_seq_items,
        number_style_default_v1,
    })
}

fn types_from_rows(schema: &X07SchemaFile) -> Result<Vec<SchemaType>> {
    #[derive(Debug, Clone, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct RowBudgets {
        #[serde(default)]
        max_doc_bytes: Option<i32>,
        #[serde(default)]
        max_depth: Option<i32>,
        #[serde(default)]
        max_map_entries: Option<i32>,
        #[serde(default)]
        max_seq_items: Option<i32>,
        #[serde(default)]
        max_string_bytes: Option<i32>,
        #[serde(default)]
        max_number_bytes: Option<i32>,
    }

    #[derive(Debug, Clone, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct TypeRowOpts {
        err_base: i32,
        brand: String,
        #[serde(default)]
        allow_unknown_fields: Option<bool>,
        #[serde(default)]
        codec: Option<String>,
        #[serde(default)]
        budgets: Option<RowBudgets>,
    }

    #[derive(Debug, Clone, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct FieldRowOpts {
        required: bool,
        #[serde(default)]
        max_string_bytes: Option<i32>,
        #[serde(default)]
        max_number_bytes: Option<i32>,
        #[serde(default)]
        max_seq_items: Option<i32>,
        #[serde(default)]
        number_style_v1: Option<String>,
    }

    fn as_i32(v: &Value, msg: &str) -> Result<i32> {
        let Some(n) = v.as_i64() else {
            anyhow::bail!("{msg}");
        };
        i32::try_from(n).map_err(|_| anyhow::anyhow!("{msg}"))
    }

    fn as_str<'a>(v: &'a Value, msg: &str) -> Result<&'a str> {
        v.as_str().ok_or_else(|| anyhow::anyhow!("{msg}"))
    }

    #[derive(Debug, Clone)]
    struct Builder {
        ty: SchemaType,
        seen_field_ids: BTreeSet<i32>,
        seen_field_names: BTreeSet<String>,
        seen_variant_ids: BTreeSet<i32>,
        seen_variant_names: BTreeSet<String>,
        seen_example_names: BTreeSet<String>,
    }

    let schema_version = parse_schema_version(&schema.schema_version)?;

    let mut types_by_id: BTreeMap<String, Builder> = BTreeMap::new();

    for (ridx, row) in schema.rows.iter().enumerate() {
        if row.is_empty() {
            anyhow::bail!("rows[{ridx}] must be a non-empty array");
        }
        let tag = row[0]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("rows[{ridx}][0] must be a string tag"))?;
        if tag != "type" {
            continue;
        }
        if row.len() != 5 && row.len() != 6 {
            anyhow::bail!("rows[{ridx}] \"type\" row must have 5 or 6 columns");
        }
        let raw_type_id = as_str(&row[1], "type_id must be a string")?.trim();
        if raw_type_id.is_empty() {
            anyhow::bail!("rows[{ridx}] type_id must be non-empty");
        }
        let version = as_i32(&row[2], "version must be an integer")?;
        let kind = as_str(&row[3], "kind must be a string")?.trim().to_string();
        let opts: TypeRowOpts =
            serde_json::from_value(row[4].clone()).context("parse type opts")?;
        if opts.err_base <= 0 {
            anyhow::bail!("rows[{ridx}] type opts.err_base must be >= 1");
        }
        if opts.brand.trim().is_empty() {
            anyhow::bail!("rows[{ridx}] type opts.brand must be non-empty");
        }
        if let Some(codec) = &opts.codec {
            if codec.trim() != "ext.data_model.doc_v1" {
                anyhow::bail!(
                    "rows[{ridx}] type opts.codec unsupported: expected \"ext.data_model.doc_v1\" got {:?}",
                    codec
                );
            }
        }

        let mut budgets = SchemaBudgets {
            max_doc_bytes: opts.budgets.as_ref().and_then(|b| b.max_doc_bytes),
            max_map_entries: opts.budgets.as_ref().and_then(|b| b.max_map_entries),
            max_number_bytes: opts.budgets.as_ref().and_then(|b| b.max_number_bytes),
            max_string_bytes: opts.budgets.as_ref().and_then(|b| b.max_string_bytes),
            allow_unknown_fields: opts.allow_unknown_fields,
            max_depth: opts.budgets.as_ref().and_then(|b| b.max_depth),
            max_seq_items: opts.budgets.as_ref().and_then(|b| b.max_seq_items),
        };
        if budgets.max_doc_bytes.is_none()
            && budgets.max_map_entries.is_none()
            && budgets.max_number_bytes.is_none()
            && budgets.max_string_bytes.is_none()
            && budgets.allow_unknown_fields.is_none()
            && budgets.max_depth.is_none()
            && budgets.max_seq_items.is_none()
        {
            budgets = SchemaBudgets {
                max_doc_bytes: None,
                max_map_entries: None,
                max_number_bytes: None,
                max_string_bytes: None,
                allow_unknown_fields: None,
                max_depth: None,
                max_seq_items: None,
            };
        }
        let budgets = if budgets.max_doc_bytes.is_none()
            && budgets.max_map_entries.is_none()
            && budgets.max_number_bytes.is_none()
            && budgets.max_string_bytes.is_none()
            && budgets.allow_unknown_fields.is_none()
            && budgets.max_depth.is_none()
            && budgets.max_seq_items.is_none()
        {
            None
        } else {
            Some(budgets)
        };

        let type_id = raw_type_id.to_string();
        if types_by_id.contains_key(&type_id) {
            anyhow::bail!("rows[{ridx}] duplicate type_id: {type_id:?}");
        }
        types_by_id.insert(
            type_id.clone(),
            Builder {
                ty: SchemaType {
                    type_id,
                    version,
                    kind,
                    err_base: opts.err_base,
                    budgets,
                    fields: Vec::new(),
                    variants: Vec::new(),
                    examples: Vec::new(),
                },
                seen_field_ids: BTreeSet::new(),
                seen_field_names: BTreeSet::new(),
                seen_variant_ids: BTreeSet::new(),
                seen_variant_names: BTreeSet::new(),
                seen_example_names: BTreeSet::new(),
            },
        );
    }

    if types_by_id.is_empty() {
        anyhow::bail!("schema.rows must include at least one \"type\" row");
    }

    for (ridx, row) in schema.rows.iter().enumerate() {
        if row.is_empty() {
            anyhow::bail!("rows[{ridx}] must be a non-empty array");
        }
        let tag = row[0]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("rows[{ridx}][0] must be a string tag"))?;
        match tag {
            "type" => {}
            "field" => {
                if row.len() != 6 && row.len() != 7 {
                    anyhow::bail!("rows[{ridx}] \"field\" row must have 6 or 7 columns");
                }
                let type_id = as_str(&row[1], "type_id must be a string")?
                    .trim()
                    .to_string();
                let Some(b) = types_by_id.get_mut(&type_id) else {
                    anyhow::bail!("rows[{ridx}] unknown type_id in field row: {type_id:?}");
                };
                if b.ty.kind.trim() != "struct" {
                    anyhow::bail!(
                        "rows[{ridx}] \"field\" rows require kind=\"struct\"; got {:?}",
                        b.ty.kind
                    );
                }
                let id = as_i32(&row[2], "field_id must be an integer")?;
                if id <= 0 {
                    anyhow::bail!("rows[{ridx}] field_id must be >= 1");
                }
                if !b.seen_field_ids.insert(id) {
                    anyhow::bail!("rows[{ridx}] duplicate field_id for type {type_id:?}: {id}");
                }
                let name = as_str(&row[3], "field name must be a string")?
                    .trim()
                    .to_string();
                if name.is_empty() {
                    anyhow::bail!("rows[{ridx}] field name must be non-empty");
                }
                if !b.seen_field_names.insert(name.clone()) {
                    anyhow::bail!(
                        "rows[{ridx}] duplicate field name for type {type_id:?}: {name:?}"
                    );
                }

                let ty = as_str(&row[4], "field ty must be a string")?
                    .trim()
                    .to_string();
                let opts: FieldRowOpts =
                    serde_json::from_value(row[5].clone()).context("parse field opts")?;

                let mut max_bytes = None;
                let mut max_items = None;
                let mut number_style_v1: Option<String> = None;
                let ty_trim = ty.trim();
                if ty_trim == "bytes" {
                    if opts.max_number_bytes.is_some() || opts.max_seq_items.is_some() {
                        anyhow::bail!("rows[{ridx}] field opts invalid for bytes field");
                    }
                    if opts.number_style_v1.is_some() {
                        anyhow::bail!("rows[{ridx}] field opts invalid for bytes field");
                    }
                    max_bytes = opts.max_string_bytes;
                } else if ty_trim == "number" {
                    if opts.max_string_bytes.is_some() || opts.max_seq_items.is_some() {
                        anyhow::bail!("rows[{ridx}] field opts invalid for number field");
                    }
                    match schema_version {
                        SchemaVersion::SpecRows010 => {
                            if opts.number_style_v1.is_some() {
                                anyhow::bail!(
                                    "rows[{ridx}] field opts.number_style_v1 requires schema_version \"x07schema.specrows@0.2.0\""
                                );
                            }
                        }
                        SchemaVersion::SpecRows020 => {
                            let style = opts.number_style_v1.as_deref().ok_or_else(|| {
                                anyhow::anyhow!(
                                    "rows[{ridx}] number field opts.number_style_v1 missing"
                                )
                            })?;
                            let _ = parse_number_style_v1(style).with_context(|| {
                                format!("rows[{ridx}] field opts.number_style_v1")
                            })?;
                            number_style_v1 = Some(style.trim().to_string());
                        }
                    }
                    max_bytes = opts.max_number_bytes;
                } else if ty_trim == "bool" {
                    if opts.max_string_bytes.is_some()
                        || opts.max_number_bytes.is_some()
                        || opts.max_seq_items.is_some()
                    {
                        anyhow::bail!("rows[{ridx}] field opts invalid for bool field");
                    }
                    if opts.number_style_v1.is_some() {
                        anyhow::bail!("rows[{ridx}] field opts invalid for bool field");
                    }
                } else if let Some(rest) = ty_trim.strip_prefix("struct:") {
                    if rest.trim().is_empty() {
                        anyhow::bail!("rows[{ridx}] field ty invalid: {ty_trim:?}");
                    }
                    if opts.max_string_bytes.is_some()
                        || opts.max_number_bytes.is_some()
                        || opts.max_seq_items.is_some()
                    {
                        anyhow::bail!("rows[{ridx}] field opts invalid for struct field");
                    }
                    if opts.number_style_v1.is_some() {
                        anyhow::bail!("rows[{ridx}] field opts invalid for struct field");
                    }
                } else if let Some(rest) = ty_trim.strip_prefix("seq:") {
                    if rest.trim().is_empty() {
                        anyhow::bail!("rows[{ridx}] field ty invalid: {ty_trim:?}");
                    }
                    max_items = opts.max_seq_items;
                    let elem = rest.trim();
                    if elem == "bytes" {
                        if opts.max_number_bytes.is_some() {
                            anyhow::bail!("rows[{ridx}] field opts invalid for seq:bytes field");
                        }
                        if opts.number_style_v1.is_some() {
                            anyhow::bail!("rows[{ridx}] field opts invalid for seq:bytes field");
                        }
                        max_bytes = opts.max_string_bytes;
                    } else if elem == "number" {
                        if opts.max_string_bytes.is_some() {
                            anyhow::bail!("rows[{ridx}] field opts invalid for seq:number field");
                        }
                        match schema_version {
                            SchemaVersion::SpecRows010 => {
                                if opts.number_style_v1.is_some() {
                                    anyhow::bail!(
                                        "rows[{ridx}] field opts.number_style_v1 requires schema_version \"x07schema.specrows@0.2.0\""
                                    );
                                }
                            }
                            SchemaVersion::SpecRows020 => {
                                let style = opts.number_style_v1.as_deref().ok_or_else(|| {
                                    anyhow::anyhow!(
                                        "rows[{ridx}] seq:number field opts.number_style_v1 missing"
                                    )
                                })?;
                                let _ = parse_number_style_v1(style).with_context(|| {
                                    format!("rows[{ridx}] field opts.number_style_v1")
                                })?;
                                number_style_v1 = Some(style.trim().to_string());
                            }
                        }
                        max_bytes = opts.max_number_bytes;
                    } else {
                        if opts.max_string_bytes.is_some() || opts.max_number_bytes.is_some() {
                            anyhow::bail!("rows[{ridx}] field opts invalid for seq field");
                        }
                        if opts.number_style_v1.is_some() {
                            anyhow::bail!("rows[{ridx}] field opts invalid for seq field");
                        }
                    }
                }

                b.ty.fields.push(SchemaField {
                    id,
                    name,
                    ty,
                    required: opts.required,
                    max_bytes,
                    max_items,
                    number_style_v1,
                });
            }
            "variant" => {
                if row.len() != 5 && row.len() != 6 {
                    anyhow::bail!("rows[{ridx}] \"variant\" row must have 5 or 6 columns");
                }
                let type_id = as_str(&row[1], "type_id must be a string")?
                    .trim()
                    .to_string();
                let Some(b) = types_by_id.get_mut(&type_id) else {
                    anyhow::bail!("rows[{ridx}] unknown type_id in variant row: {type_id:?}");
                };
                if b.ty.kind.trim() != "enum" {
                    anyhow::bail!(
                        "rows[{ridx}] \"variant\" rows require kind=\"enum\"; got {:?}",
                        b.ty.kind
                    );
                }
                let id = as_i32(&row[2], "variant_id must be an integer")?;
                if id <= 0 {
                    anyhow::bail!("rows[{ridx}] variant_id must be >= 1");
                }
                if !b.seen_variant_ids.insert(id) {
                    anyhow::bail!("rows[{ridx}] duplicate variant_id for type {type_id:?}: {id}");
                }
                let name = as_str(&row[3], "variant name must be a string")?
                    .trim()
                    .to_string();
                if name.is_empty() {
                    anyhow::bail!("rows[{ridx}] variant name must be non-empty");
                }
                if !b.seen_variant_names.insert(name.clone()) {
                    anyhow::bail!(
                        "rows[{ridx}] duplicate variant name for type {type_id:?}: {name:?}"
                    );
                }
                let payload_ty = as_str(&row[4], "payload_ty must be a string")?
                    .trim()
                    .to_string();
                b.ty.variants.push(SchemaVariant {
                    id,
                    name,
                    payload_ty,
                });
            }
            "example" => {
                if row.len() != 4 && row.len() != 5 {
                    anyhow::bail!("rows[{ridx}] \"example\" row must have 4 or 5 columns");
                }
                let type_id = as_str(&row[1], "type_id must be a string")?
                    .trim()
                    .to_string();
                let Some(b) = types_by_id.get_mut(&type_id) else {
                    anyhow::bail!("rows[{ridx}] unknown type_id in example row: {type_id:?}");
                };
                let name = as_str(&row[2], "example_id must be a string")?
                    .trim()
                    .to_string();
                if name.is_empty() {
                    anyhow::bail!("rows[{ridx}] example_id must be non-empty");
                }
                if !b.seen_example_names.insert(name.clone()) {
                    anyhow::bail!(
                        "rows[{ridx}] duplicate example_id for type {type_id:?}: {name:?}"
                    );
                }
                b.ty.examples.push(SchemaExample {
                    name,
                    value: row[3].clone(),
                });
            }
            _ => anyhow::bail!("rows[{ridx}] unknown row tag: {tag:?}"),
        }
    }

    Ok(types_by_id.into_values().map(|b| b.ty).collect())
}

fn normalize_schema(schema_version: SchemaVersion, schema: &X07SchemaFile) -> Result<Vec<TypeDef>> {
    if schema.package.name.trim().is_empty() {
        anyhow::bail!("package.name must be non-empty");
    }
    if schema.package.version.trim().is_empty() {
        anyhow::bail!("package.version must be non-empty");
    }
    x07c::validate::validate_module_id(schema.package.name.trim())
        .map_err(|e| anyhow::anyhow!("invalid package.name: {e}"))?;
    let defaults = resolve_defaults(schema_version, &schema.defaults)?;

    let mut out: Vec<TypeDef> = Vec::with_capacity(schema.types.len());
    let mut raw_examples_by_type_id: BTreeMap<String, Vec<SchemaExample>> = BTreeMap::new();
    for (idx, t) in schema.types.iter().enumerate() {
        let kind = match t.kind.trim() {
            "struct" => TypeKind::Struct,
            "enum" => TypeKind::Enum,
            _ => {
                anyhow::bail!(
                    "types[{idx}].kind unsupported: expected \"struct\" or \"enum\" got {:?}",
                    t.kind
                );
            }
        };
        if t.version <= 0 {
            anyhow::bail!("types[{idx}].version must be >= 1");
        }
        if t.err_base <= 0 {
            anyhow::bail!("types[{idx}].err_base must be >= 1");
        }

        let effective_max_doc_bytes = t
            .budgets
            .as_ref()
            .and_then(|b| b.max_doc_bytes)
            .unwrap_or(defaults.max_doc_bytes);
        let effective_max_map_entries = t
            .budgets
            .as_ref()
            .and_then(|b| b.max_map_entries)
            .unwrap_or(defaults.max_map_entries);
        let effective_max_depth = t
            .budgets
            .as_ref()
            .and_then(|b| b.max_depth)
            .unwrap_or(defaults.max_depth);
        let effective_max_seq_items = t
            .budgets
            .as_ref()
            .and_then(|b| b.max_seq_items)
            .unwrap_or(defaults.max_seq_items);
        let allow_unknown_fields = t
            .budgets
            .as_ref()
            .and_then(|b| b.allow_unknown_fields)
            .unwrap_or(defaults.allow_unknown_fields);

        let effective_max_string_bytes = t
            .budgets
            .as_ref()
            .and_then(|b| b.max_string_bytes)
            .unwrap_or(defaults.max_string_bytes);
        let effective_max_number_bytes = t
            .budgets
            .as_ref()
            .and_then(|b| b.max_number_bytes)
            .unwrap_or(defaults.max_number_bytes);

        if effective_max_doc_bytes <= 0 {
            anyhow::bail!("types[{idx}].budgets.max_doc_bytes must be >= 1");
        }
        if effective_max_map_entries <= 0 {
            anyhow::bail!("types[{idx}].budgets.max_map_entries must be >= 1");
        }
        if effective_max_depth <= 0 {
            anyhow::bail!("types[{idx}].budgets.max_depth must be >= 1");
        }
        if effective_max_seq_items <= 0 {
            anyhow::bail!("types[{idx}].budgets.max_seq_items must be >= 1");
        }
        if effective_max_string_bytes <= 0 {
            anyhow::bail!("types[{idx}].budgets.max_string_bytes must be >= 1");
        }
        if effective_max_number_bytes <= 0 {
            anyhow::bail!("types[{idx}].budgets.max_number_bytes must be >= 1");
        }

        let (module_id, tests_module_id) =
            derive_module_ids(schema.package.name.trim(), &t.type_id, t.version)?;

        let mut variants: Vec<VariantDef> = Vec::new();
        let mut fields: Vec<FieldDef> = Vec::new();
        match kind {
            TypeKind::Struct => {
                if !t.variants.is_empty() {
                    anyhow::bail!("types[{idx}].variants must be omitted for struct types");
                }

                let mut seen_field_ids: BTreeSet<i32> = BTreeSet::new();
                let mut seen_field_names: BTreeSet<String> = BTreeSet::new();
                for (fidx, f) in t.fields.iter().enumerate() {
                    if f.id <= 0 {
                        anyhow::bail!("types[{idx}].fields[{fidx}].id must be >= 1");
                    }
                    if !seen_field_ids.insert(f.id) {
                        anyhow::bail!("types[{idx}].fields has duplicate id: {}", f.id);
                    }
                    let name = f.name.trim();
                    if name.is_empty() {
                        anyhow::bail!("types[{idx}].fields[{fidx}].name must be non-empty");
                    }
                    if name.starts_with('$') {
                        anyhow::bail!(
                            "types[{idx}].fields[{fidx}].name invalid: names starting with \"$\" are reserved"
                        );
                    }
                    x07c::validate::validate_local_name(name).map_err(|e| {
                        anyhow::anyhow!("types[{idx}].fields[{fidx}].name invalid: {e}")
                    })?;
                    if !seen_field_names.insert(name.to_string()) {
                        anyhow::bail!("types[{idx}].fields has duplicate name: {name:?}");
                    }

                    let ty = FieldTy::parse(f.ty.trim())
                        .with_context(|| format!("types[{idx}].fields[{fidx}].ty"))?;
                    let (max_bytes, max_items) = match &ty {
                        FieldTy::Bool => {
                            if f.max_bytes.is_some() {
                                anyhow::bail!(
                                    "types[{idx}].fields[{fidx}].max_bytes must be omitted for bool fields"
                                );
                            }
                            if f.max_items.is_some() {
                                anyhow::bail!(
                                    "types[{idx}].fields[{fidx}].max_items must be omitted for bool fields"
                                );
                            }
                            (None, None)
                        }
                        FieldTy::Bytes => {
                            if f.max_items.is_some() {
                                anyhow::bail!(
                                    "types[{idx}].fields[{fidx}].max_items must be omitted for bytes fields"
                                );
                            }
                            (
                                Some(f.max_bytes.unwrap_or(effective_max_string_bytes)),
                                None,
                            )
                        }
                        FieldTy::Number => {
                            if f.max_items.is_some() {
                                anyhow::bail!(
                                    "types[{idx}].fields[{fidx}].max_items must be omitted for number fields"
                                );
                            }
                            (
                                Some(f.max_bytes.unwrap_or(effective_max_number_bytes)),
                                None,
                            )
                        }
                        FieldTy::Struct { .. } => {
                            if f.max_bytes.is_some() {
                                anyhow::bail!(
                                    "types[{idx}].fields[{fidx}].max_bytes must be omitted for struct fields"
                                );
                            }
                            if f.max_items.is_some() {
                                anyhow::bail!(
                                    "types[{idx}].fields[{fidx}].max_items must be omitted for struct fields"
                                );
                            }
                            (None, None)
                        }
                        FieldTy::Seq { elem } => {
                            let max_items = Some(f.max_items.unwrap_or(effective_max_seq_items));
                            let max_bytes = match elem.as_ref() {
                                FieldTy::Bool | FieldTy::Struct { .. } | FieldTy::Seq { .. } => {
                                    if f.max_bytes.is_some() {
                                        anyhow::bail!(
                                            "types[{idx}].fields[{fidx}].max_bytes must be omitted for this seq element type"
                                        );
                                    }
                                    None
                                }
                                FieldTy::Bytes => {
                                    Some(f.max_bytes.unwrap_or(effective_max_string_bytes))
                                }
                                FieldTy::Number => {
                                    Some(f.max_bytes.unwrap_or(effective_max_number_bytes))
                                }
                            };
                            (max_bytes, max_items)
                        }
                    };
                    if let Some(mb) = max_bytes {
                        if mb <= 0 {
                            anyhow::bail!("types[{idx}].fields[{fidx}].max_bytes must be >= 1");
                        }
                    }
                    if let Some(mi) = max_items {
                        if mi <= 0 {
                            anyhow::bail!("types[{idx}].fields[{fidx}].max_items must be >= 1");
                        }
                    }

                    let number_style = match &ty {
                        FieldTy::Number => match schema_version {
                            SchemaVersion::SpecRows010 => {
                                if f.number_style_v1.is_some() {
                                    anyhow::bail!(
                                        "types[{idx}].fields[{fidx}].number_style_v1 requires schema_version \"x07schema.specrows@0.2.0\""
                                    );
                                }
                                None
                            }
                            SchemaVersion::SpecRows020 => {
                                let style = f.number_style_v1.as_deref().ok_or_else(|| {
                                    anyhow::anyhow!(
                                        "types[{idx}].fields[{fidx}].number_style_v1 missing for number field"
                                    )
                                })?;
                                Some(parse_number_style_v1(style).with_context(|| {
                                    format!("types[{idx}].fields[{fidx}].number_style_v1")
                                })?)
                            }
                        },
                        FieldTy::Seq { elem } if matches!(elem.as_ref(), FieldTy::Number) => {
                            match schema_version {
                                SchemaVersion::SpecRows010 => {
                                    if f.number_style_v1.is_some() {
                                        anyhow::bail!(
                                            "types[{idx}].fields[{fidx}].number_style_v1 requires schema_version \"x07schema.specrows@0.2.0\""
                                        );
                                    }
                                    None
                                }
                                SchemaVersion::SpecRows020 => {
                                    let style = f.number_style_v1.as_deref().ok_or_else(|| {
                                        anyhow::anyhow!(
                                            "types[{idx}].fields[{fidx}].number_style_v1 missing for seq:number field"
                                        )
                                    })?;
                                    Some(parse_number_style_v1(style).with_context(|| {
                                        format!("types[{idx}].fields[{fidx}].number_style_v1")
                                    })?)
                                }
                            }
                        }
                        _ => {
                            if f.number_style_v1.is_some() {
                                anyhow::bail!(
                                    "types[{idx}].fields[{fidx}].number_style_v1 is only valid for number and seq:number fields"
                                );
                            }
                            None
                        }
                    };

                    fields.push(FieldDef {
                        id: f.id,
                        name: name.to_string(),
                        ty,
                        required: f.required,
                        max_bytes,
                        max_items,
                        number_style,
                    });
                }
                fields.sort_by(|a, b| a.id.cmp(&b.id));
            }
            TypeKind::Enum => {
                if !t.fields.is_empty() {
                    anyhow::bail!("types[{idx}].fields must be empty for enum types");
                }
                if t.variants.is_empty() {
                    anyhow::bail!("types[{idx}].variants must be non-empty for enum types");
                }

                let mut seen_variant_ids: BTreeSet<i32> = BTreeSet::new();
                let mut seen_variant_names: BTreeSet<String> = BTreeSet::new();
                for (vidx, v) in t.variants.iter().enumerate() {
                    if v.id <= 0 {
                        anyhow::bail!("types[{idx}].variants[{vidx}].id must be >= 1");
                    }
                    if !seen_variant_ids.insert(v.id) {
                        anyhow::bail!("types[{idx}].variants has duplicate id: {}", v.id);
                    }
                    let name = v.name.trim();
                    if name.is_empty() {
                        anyhow::bail!("types[{idx}].variants[{vidx}].name must be non-empty");
                    }
                    if name.starts_with('$') {
                        anyhow::bail!(
                            "types[{idx}].variants[{vidx}].name invalid: names starting with \"$\" are reserved"
                        );
                    }
                    x07c::validate::validate_local_name(name).map_err(|e| {
                        anyhow::anyhow!("types[{idx}].variants[{vidx}].name invalid: {e}")
                    })?;
                    if !seen_variant_names.insert(name.to_string()) {
                        anyhow::bail!("types[{idx}].variants has duplicate name: {name:?}");
                    }

                    let payload_s = v.payload_ty.trim();
                    if payload_s.is_empty() {
                        anyhow::bail!("types[{idx}].variants[{vidx}].payload_ty must be non-empty");
                    }
                    let payload = if payload_s == "unit" {
                        VariantPayloadDef::Unit
                    } else {
                        let ty = FieldTy::parse(payload_s)
                            .with_context(|| format!("types[{idx}].variants[{vidx}].payload_ty"))?;
                        let (max_bytes, max_items) = match &ty {
                            FieldTy::Bool => (None, None),
                            FieldTy::Bytes => (Some(effective_max_string_bytes), None),
                            FieldTy::Number => (Some(effective_max_number_bytes), None),
                            FieldTy::Struct { .. } => (None, None),
                            FieldTy::Seq { elem } => {
                                let max_items = Some(effective_max_seq_items);
                                let max_bytes = match elem.as_ref() {
                                    FieldTy::Bool
                                    | FieldTy::Struct { .. }
                                    | FieldTy::Seq { .. } => None,
                                    FieldTy::Bytes => Some(effective_max_string_bytes),
                                    FieldTy::Number => Some(effective_max_number_bytes),
                                };
                                (max_bytes, max_items)
                            }
                        };
                        VariantPayloadDef::Value {
                            ty,
                            max_bytes,
                            max_items,
                        }
                    };

                    variants.push(VariantDef {
                        id: v.id,
                        name: name.to_string(),
                        payload,
                    });
                }
                variants.sort_by(|a, b| a.id.cmp(&b.id));
            }
        }

        raw_examples_by_type_id.insert(t.type_id.trim().to_string(), t.examples.clone());

        out.push(TypeDef {
            module_id,
            tests_module_id,
            type_id: t.type_id.trim().to_string(),
            version: t.version,
            kind,
            err_base: t.err_base,
            max_doc_bytes: effective_max_doc_bytes,
            max_map_entries: effective_max_map_entries,
            max_depth: effective_max_depth,
            max_seq_items: effective_max_seq_items,
            allow_unknown_fields,
            schema_version,
            number_style_default_v1: defaults.number_style_default_v1,
            variants,
            fields,
            examples: Vec::new(),
        });
    }

    out.sort_by(|a, b| {
        let c = a.type_id.cmp(&b.type_id);
        if c != std::cmp::Ordering::Equal {
            return c;
        }
        a.version.cmp(&b.version)
    });

    let mut seen_type_ids: BTreeSet<String> = BTreeSet::new();
    let mut struct_fields_by_id: BTreeMap<String, Vec<FieldDef>> = BTreeMap::new();
    for td in &out {
        if !seen_type_ids.insert(td.type_id.clone()) {
            anyhow::bail!(
                "schema contains multiple versions of type_id {:?}; examples are ambiguous",
                td.type_id
            );
        }
        if td.kind == TypeKind::Struct {
            struct_fields_by_id.insert(td.type_id.clone(), td.fields.clone());
        }
    }

    for (tidx, td) in out.iter_mut().enumerate() {
        let raw_examples = raw_examples_by_type_id
            .get(&td.type_id)
            .cloned()
            .unwrap_or_default();
        let mut examples: Vec<ExampleDef> = Vec::with_capacity(raw_examples.len());

        for (eidx, ex) in raw_examples.iter().enumerate() {
            let ex_name = ex.name.trim();
            if ex_name.is_empty() {
                anyhow::bail!("types[{tidx}].examples[{eidx}].name must be non-empty");
            }
            x07c::validate::validate_local_name(ex_name)
                .map_err(|e| anyhow::anyhow!("types[{tidx}].examples[{eidx}].name invalid: {e}"))?;
            match td.kind {
                TypeKind::Struct => {
                    let obj = ex.value.as_object().ok_or_else(|| {
                        anyhow::anyhow!("types[{tidx}].examples[{eidx}].value must be an object")
                    })?;

                    let mut values: BTreeMap<String, ExampleValue> = BTreeMap::new();
                    for (k, v) in obj {
                        let Some(field) = td.fields.iter().find(|f| f.name == *k) else {
                            anyhow::bail!(
                                "types[{tidx}].examples[{eidx}] has unknown field: {k:?}"
                            );
                        };
                        values.insert(
                            k.to_string(),
                            parse_example_value_typed(&field.ty, v, &struct_fields_by_id)?,
                        );
                    }

                    for f in &td.fields {
                        if f.required && !values.contains_key(&f.name) {
                            anyhow::bail!(
                                "types[{tidx}].examples[{eidx}] missing required field: {:?}",
                                f.name
                            );
                        }
                    }

                    examples.push(ExampleDef {
                        name: ex_name.to_string(),
                        kind: ExampleKind::Struct { values },
                    });
                }
                TypeKind::Enum => {
                    let arr = ex.value.as_array().ok_or_else(|| {
                        anyhow::anyhow!("types[{tidx}].examples[{eidx}].value must be an array")
                    })?;
                    if arr.len() != 1 && arr.len() != 2 {
                        anyhow::bail!(
                            "types[{tidx}].examples[{eidx}].value must have 1 or 2 elements"
                        );
                    }
                    let variant_name = arr[0]
                        .as_str()
                        .ok_or_else(|| {
                            anyhow::anyhow!("enum example variant name must be a string")
                        })?
                        .trim()
                        .to_string();
                    if variant_name.is_empty() {
                        anyhow::bail!("types[{tidx}].examples[{eidx}].value[0] must be non-empty");
                    }
                    let Some(variant) = td.variants.iter().find(|v| v.name == variant_name) else {
                        anyhow::bail!(
                            "types[{tidx}].examples[{eidx}] has unknown variant: {variant_name:?}"
                        );
                    };

                    let payload = match (&variant.payload, arr.get(1)) {
                        (VariantPayloadDef::Unit, None) => None,
                        (VariantPayloadDef::Unit, Some(Value::Null)) => None,
                        (VariantPayloadDef::Unit, Some(_)) => anyhow::bail!(
                            "types[{tidx}].examples[{eidx}] unit variant payload must be null"
                        ),
                        (VariantPayloadDef::Value { ty, .. }, Some(v)) => {
                            Some(parse_example_value_typed(ty, v, &struct_fields_by_id)?)
                        }
                        (VariantPayloadDef::Value { .. }, None) => {
                            anyhow::bail!("types[{tidx}].examples[{eidx}] missing enum payload")
                        }
                    };

                    examples.push(ExampleDef {
                        name: ex_name.to_string(),
                        kind: ExampleKind::Enum {
                            variant: variant_name,
                            payload,
                        },
                    });
                }
            }
        }

        td.examples = examples;
    }

    Ok(out)
}

fn build_type_index(types: &[TypeDef]) -> Result<TypeIndex> {
    let mut idx: TypeIndex = BTreeMap::new();
    for td in types {
        if idx
            .insert(
                td.type_id.clone(),
                TypeInfo {
                    module_id: td.module_id.clone(),
                    kind: td.kind,
                    variants: td.variants.clone(),
                    fields: td.fields.clone(),
                },
            )
            .is_some()
        {
            anyhow::bail!(
                "schema contains multiple versions of type_id {:?}; references are ambiguous",
                td.type_id
            );
        }
    }

    fn collect_type_id_deps(ty: &FieldTy, out: &mut BTreeSet<String>) {
        match ty {
            FieldTy::Bool | FieldTy::Number | FieldTy::Bytes => {}
            FieldTy::Struct { type_id } => {
                out.insert(type_id.clone());
            }
            FieldTy::Seq { elem } => collect_type_id_deps(elem, out),
        }
    }

    let mut deps: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for td in types {
        let mut out: BTreeSet<String> = BTreeSet::new();
        for f in &td.fields {
            collect_type_id_deps(&f.ty, &mut out);
        }
        for v in &td.variants {
            if let VariantPayloadDef::Value { ty, .. } = &v.payload {
                collect_type_id_deps(ty, &mut out);
            }
        }
        deps.insert(td.type_id.clone(), out);
    }

    // Reject cycles (including indirect), otherwise generated modules may have cyclic imports.
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Mark {
        Visiting,
        Visited,
    }

    let mut marks: BTreeMap<String, Mark> = BTreeMap::new();
    let mut stack: Vec<String> = Vec::new();

    fn visit(
        node: &str,
        deps: &BTreeMap<String, BTreeSet<String>>,
        marks: &mut BTreeMap<String, Mark>,
        stack: &mut Vec<String>,
    ) -> Result<()> {
        if matches!(marks.get(node), Some(Mark::Visited)) {
            return Ok(());
        }
        if matches!(marks.get(node), Some(Mark::Visiting)) {
            if let Some(pos) = stack.iter().position(|s| s == node) {
                let cycle = stack[pos..].join(" -> ");
                anyhow::bail!("schema contains recursive type references: {cycle}");
            }
            anyhow::bail!("schema contains recursive type references: {node}");
        }

        marks.insert(node.to_string(), Mark::Visiting);
        stack.push(node.to_string());

        let Some(children) = deps.get(node) else {
            anyhow::bail!("schema references unknown type_id: {node:?}");
        };
        for child in children {
            visit(child, deps, marks, stack)?;
        }

        stack.pop();
        marks.insert(node.to_string(), Mark::Visited);
        Ok(())
    }

    for td in types {
        visit(&td.type_id, &deps, &mut marks, &mut stack)?;
    }

    // Disallow referencing enum types as struct:<type_id>.
    fn validate_struct_refs(ty: &FieldTy, idx: &TypeIndex) -> Result<()> {
        match ty {
            FieldTy::Bool | FieldTy::Number | FieldTy::Bytes => Ok(()),
            FieldTy::Seq { elem } => validate_struct_refs(elem, idx),
            FieldTy::Struct { type_id } => {
                let Some(info) = idx.get(type_id) else {
                    anyhow::bail!("schema references unknown type_id: {type_id:?}");
                };
                if info.kind != TypeKind::Struct {
                    anyhow::bail!("struct:{type_id:?} must reference a struct type; got enum");
                }
                Ok(())
            }
        }
    }

    for td in types {
        for f in &td.fields {
            validate_struct_refs(&f.ty, &idx)?;
        }
        for v in &td.variants {
            if let VariantPayloadDef::Value { ty, .. } = &v.payload {
                validate_struct_refs(ty, &idx)?;
            }
        }
    }

    Ok(idx)
}

fn parse_example_value_typed(
    ty: &FieldTy,
    v: &Value,
    type_fields_by_id: &BTreeMap<String, Vec<FieldDef>>,
) -> Result<ExampleValue> {
    match ty {
        FieldTy::Bool => v
            .as_bool()
            .map(ExampleValue::Bool)
            .ok_or_else(|| anyhow::anyhow!("example value must be bool")),
        FieldTy::Bytes | FieldTy::Number => match v {
            Value::String(s) => Ok(ExampleValue::Bytes(s.to_string())),
            Value::Number(_) => {
                anyhow::bail!("example numbers must be encoded as strings (decimal bytes)")
            }
            _ => anyhow::bail!("example value must be string"),
        },
        FieldTy::Struct { type_id } => {
            let obj = v
                .as_object()
                .ok_or_else(|| anyhow::anyhow!("example value must be an object"))?;
            let Some(fields) = type_fields_by_id.get(type_id) else {
                anyhow::bail!("example references unknown type_id: {type_id:?}");
            };

            let mut values: BTreeMap<String, ExampleValue> = BTreeMap::new();
            for (k, vv) in obj {
                let Some(field) = fields.iter().find(|f| f.name == *k) else {
                    anyhow::bail!("example has unknown field: {k:?}");
                };
                values.insert(
                    k.to_string(),
                    parse_example_value_typed(&field.ty, vv, type_fields_by_id)?,
                );
            }
            for f in fields {
                if f.required && !values.contains_key(&f.name) {
                    anyhow::bail!("example missing required field: {:?}", f.name);
                }
            }
            Ok(ExampleValue::Struct(values))
        }
        FieldTy::Seq { elem } => {
            let arr = v
                .as_array()
                .ok_or_else(|| anyhow::anyhow!("example value must be an array"))?;
            let mut out: Vec<ExampleValue> = Vec::with_capacity(arr.len());
            for vv in arr {
                out.push(parse_example_value_typed(
                    elem.as_ref(),
                    vv,
                    type_fields_by_id,
                )?);
            }
            Ok(ExampleValue::Seq(out))
        }
    }
}

fn derive_module_ids(pkg: &str, type_id: &str, version: i32) -> Result<(String, String)> {
    let type_id = type_id.trim();
    if type_id.is_empty() {
        anyhow::bail!("type_id must be non-empty");
    }
    let type_mod = if let Some((prefix, last)) = type_id.rsplit_once('.') {
        format!("{prefix}.{last}_v{version}")
    } else {
        format!("{type_id}_v{version}")
    };
    let module_id = format!("{pkg}.schema.{type_mod}");
    x07c::validate::validate_module_id(&module_id)
        .map_err(|e| anyhow::anyhow!("invalid derived module_id {module_id:?}: {e}"))?;

    let tests_module_id = format!("{module_id}.tests");
    x07c::validate::validate_module_id(&tests_module_id)
        .map_err(|e| anyhow::anyhow!("invalid derived tests module_id {tests_module_id:?}: {e}"))?;

    Ok((module_id, tests_module_id))
}

fn generate_type_outputs(
    pkg: &str,
    type_index: &TypeIndex,
    td: &TypeDef,
) -> Result<Vec<GeneratedOutput>> {
    let paths = derive_type_paths(pkg, &td.type_id, td.version)?;
    let runtime_bytes = generate_runtime_module(type_index, td)?;
    let tests_bytes = generate_tests_module(type_index, td)?;
    Ok(vec![
        GeneratedOutput {
            rel_path: paths.runtime_rel,
            kind: "module",
            bytes: runtime_bytes,
        },
        GeneratedOutput {
            rel_path: paths.tests_rel,
            kind: "test_module",
            bytes: tests_bytes,
        },
    ])
}

struct TypePaths {
    runtime_rel: PathBuf,
    tests_rel: PathBuf,
}

fn derive_type_paths(pkg: &str, type_id: &str, version: i32) -> Result<TypePaths> {
    let mut parts: Vec<&str> = type_id.split('.').collect();
    if parts.is_empty() {
        anyhow::bail!("invalid type_id: {type_id:?}");
    }
    let last = parts.pop().unwrap();
    for (idx, seg) in parts.iter().enumerate() {
        if seg.trim().is_empty() {
            anyhow::bail!("invalid type_id segment at index {idx}: {type_id:?}");
        }
        x07c::validate::validate_local_name(seg)
            .map_err(|e| anyhow::anyhow!("invalid type_id segment {:?}: {e}", seg))?;
    }
    x07c::validate::validate_local_name(last)
        .map_err(|e| anyhow::anyhow!("invalid type_id segment {:?}: {e}", last))?;

    let file_stem = format!("{last}_v{version}");

    let mut base = PathBuf::new();
    base.push("modules");
    base.push(pkg);
    base.push("schema");
    for seg in parts {
        base.push(seg);
    }

    Ok(TypePaths {
        runtime_rel: base.join(format!("{file_stem}.x07.json")),
        tests_rel: base.join(file_stem).join("tests.x07.json"),
    })
}

fn generate_tests_manifest(_pkg: &str, types: &[TypeDef]) -> Result<GeneratedOutput> {
    #[derive(Serialize)]
    struct TestManifest<'a> {
        schema_version: &'a str,
        tests: Vec<TestManifestEntry>,
    }

    #[derive(Serialize)]
    struct TestManifestEntry {
        id: String,
        entry: String,
        world: String,
        expect: String,
    }

    let mut tests: Vec<TestManifestEntry> = Vec::new();
    for td in types {
        let id = format!("{}_v{}", td.type_id, td.version);
        let entry = format!("{}.test_vectors_v1", td.tests_module_id);
        tests.push(TestManifestEntry {
            id,
            entry,
            world: "solve-pure".to_string(),
            expect: "pass".to_string(),
        });
    }

    let doc = TestManifest {
        schema_version: "x07.tests_manifest@0.1.0",
        tests,
    };
    let mut bytes = serde_json::to_vec_pretty(&doc)?;
    if bytes.last() != Some(&b'\n') {
        bytes.push(b'\n');
    }

    Ok(GeneratedOutput {
        rel_path: PathBuf::from("tests/tests.json"),
        kind: "tests_manifest",
        bytes,
    })
}

fn collect_type_deps(type_index: &TypeIndex, td: &TypeDef) -> Result<BTreeSet<String>> {
    fn walk(
        type_index: &TypeIndex,
        td: &TypeDef,
        ty: &FieldTy,
        out: &mut BTreeSet<String>,
    ) -> Result<()> {
        match ty {
            FieldTy::Bool | FieldTy::Number | FieldTy::Bytes => Ok(()),
            FieldTy::Struct { type_id } => {
                let Some(dep) = type_index.get(type_id) else {
                    anyhow::bail!(
                        "{}: unresolved struct field type reference: {:?}",
                        td.module_id,
                        type_id
                    );
                };
                if dep.module_id == td.module_id {
                    anyhow::bail!(
                        "{}: self-recursive struct field type reference: {:?}",
                        td.module_id,
                        type_id
                    );
                }
                out.insert(dep.module_id.clone());
                Ok(())
            }
            FieldTy::Seq { elem } => walk(type_index, td, elem, out),
        }
    }

    let mut out: BTreeSet<String> = BTreeSet::new();
    for f in &td.fields {
        walk(type_index, td, &f.ty, &mut out)?;
    }
    for v in &td.variants {
        if let VariantPayloadDef::Value { ty, .. } = &v.payload {
            walk(type_index, td, ty, &mut out)?;
        }
    }
    Ok(out)
}

fn collect_transitive_type_deps(type_index: &TypeIndex, td: &TypeDef) -> Result<BTreeSet<String>> {
    fn collect_type_ids(ty: &FieldTy, out: &mut BTreeSet<String>) {
        match ty {
            FieldTy::Bool | FieldTy::Number | FieldTy::Bytes => {}
            FieldTy::Struct { type_id } => {
                out.insert(type_id.clone());
            }
            FieldTy::Seq { elem } => collect_type_ids(elem, out),
        }
    }

    let mut pending: Vec<String> = Vec::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut out: BTreeSet<String> = BTreeSet::new();

    for f in &td.fields {
        let mut deps: BTreeSet<String> = BTreeSet::new();
        collect_type_ids(&f.ty, &mut deps);
        pending.extend(deps);
    }
    for v in &td.variants {
        if let VariantPayloadDef::Value { ty, .. } = &v.payload {
            let mut deps: BTreeSet<String> = BTreeSet::new();
            collect_type_ids(ty, &mut deps);
            pending.extend(deps);
        }
    }

    while let Some(type_id) = pending.pop() {
        if !seen.insert(type_id.clone()) {
            continue;
        }
        let Some(info) = type_index.get(&type_id) else {
            anyhow::bail!(
                "{}: unresolved struct field type reference: {:?}",
                td.module_id,
                type_id
            );
        };
        if info.module_id == td.module_id {
            anyhow::bail!(
                "{}: self-recursive struct field type reference: {:?}",
                td.module_id,
                type_id
            );
        }
        out.insert(info.module_id.clone());

        for f in &info.fields {
            let mut deps: BTreeSet<String> = BTreeSet::new();
            collect_type_ids(&f.ty, &mut deps);
            for dep in deps {
                if !seen.contains(&dep) {
                    pending.push(dep);
                }
            }
        }
        for v in &info.variants {
            if let VariantPayloadDef::Value { ty, .. } = &v.payload {
                let mut deps: BTreeSet<String> = BTreeSet::new();
                collect_type_ids(ty, &mut deps);
                for dep in deps {
                    if !seen.contains(&dep) {
                        pending.push(dep);
                    }
                }
            }
        }
    }

    Ok(out)
}

fn generate_runtime_module(type_index: &TypeIndex, td: &TypeDef) -> Result<Vec<u8>> {
    match td.kind {
        TypeKind::Struct => generate_runtime_module_struct(type_index, td),
        TypeKind::Enum => generate_runtime_module_enum(type_index, td),
    }
}

fn generate_runtime_module_struct(type_index: &TypeIndex, td: &TypeDef) -> Result<Vec<u8>> {
    let mut imports: BTreeSet<String> = BTreeSet::new();
    imports.insert("ext.data_model".to_string());
    for dep in collect_type_deps(type_index, td)? {
        imports.insert(dep);
    }

    let mut exports: BTreeSet<String> = BTreeSet::new();

    let mut functions: Vec<FunctionDef> = Vec::new();

    let shape_note = format!(
        "x07schema:{}@v{};encoding=ext.data_model;struct_map_keys_sorted=1;max_depth={};max_seq_items={}",
        td.type_id, td.version, td.max_depth, td.max_seq_items
    );
    add_export_defn(
        td,
        "shape_note_v1",
        &mut exports,
        &mut functions,
        vec![],
        Ty::Bytes,
        e_bytes_lit(&shape_note),
    );

    add_export_defn(
        td,
        "err_base_v1",
        &mut exports,
        &mut functions,
        vec![],
        Ty::I32,
        e_int(td.err_base),
    );

    let code_doc_invalid = td.err_base + 1;
    let code_root_kind = td.err_base + 2;
    let code_doc_too_large = td.err_base + 3;
    let code_unknown_field = td.err_base + 30;
    let code_noncanonical_map = td.err_base + 31;
    let code_dup_field = td.err_base + 32;
    let code_map_too_many_entries = td.err_base + 33;

    for (name, code) in [
        ("code_doc_invalid_v1", code_doc_invalid),
        ("code_root_kind_v1", code_root_kind),
        ("code_doc_too_large_v1", code_doc_too_large),
        ("code_unknown_field_v1", code_unknown_field),
        ("code_noncanonical_map_v1", code_noncanonical_map),
        ("code_dup_field_v1", code_dup_field),
        ("code_map_too_many_entries_v1", code_map_too_many_entries),
    ] {
        add_export_defn(
            td,
            name,
            &mut exports,
            &mut functions,
            vec![],
            Ty::I32,
            e_int(code),
        );
    }

    for f in &td.fields {
        add_field_code_exports(td, f, &mut exports, &mut functions);
    }

    functions.push(gen_cmp_bytes_range(td)?);
    functions.push(gen_add_entry(td)?);
    if td.fields.iter().any(|f| f.number_style.is_some()) {
        functions.push(gen_is_canon_int_ascii_v1(td)?);
        functions.push(gen_is_canon_uint_ascii_v1(td)?);
    }
    add_export_name(
        td,
        &format!("{}.validate_doc_v1", td.module_id),
        &mut exports,
    );
    functions.push(gen_validate_doc(td)?);
    add_export_name(
        td,
        &format!("{}.validate_value_v1", td.module_id),
        &mut exports,
    );
    functions.push(gen_validate_value(type_index, td)?);
    add_export_name(td, &format!("{}.encode_doc_v1", td.module_id), &mut exports);
    functions.push(gen_encode_doc(td)?);
    add_export_name(
        td,
        &format!("{}.encode_value_v1", td.module_id),
        &mut exports,
    );
    functions.push(gen_encode_value(td)?);
    for f in &td.fields {
        let getter = match &f.ty {
            FieldTy::Bool => format!("{}.get_{}_v1", td.module_id, f.name),
            FieldTy::Bytes | FieldTy::Number => {
                format!("{}.get_{}_view_v1", td.module_id, f.name)
            }
            FieldTy::Struct { .. } | FieldTy::Seq { .. } => {
                format!("{}.get_{}_value_view_v1", td.module_id, f.name)
            }
        };
        add_export_name(td, &getter, &mut exports);
        if !f.required {
            add_export_name(
                td,
                &format!("{}.has_{}_v1", td.module_id, f.name),
                &mut exports,
            );
        }
        functions.extend(gen_field_accessors(td, f)?);
    }

    let mut file = X07AstFile {
        kind: X07AstKind::Module,
        module_id: td.module_id.clone(),
        imports,
        exports,
        functions,
        async_functions: Vec::new(),
        extern_functions: Vec::new(),
        solve: None,
        meta: BTreeMap::new(),
    };

    x07c::x07ast::canonicalize_x07ast_file(&mut file);
    let mut v = x07c::x07ast::x07ast_file_to_value(&file);
    x07c::x07ast::canon_value_jcs(&mut v);
    let mut bytes = serde_json::to_string(&v)?.into_bytes();
    if bytes.last() != Some(&b'\n') {
        bytes.push(b'\n');
    }
    Ok(bytes)
}

fn generate_runtime_module_enum(type_index: &TypeIndex, td: &TypeDef) -> Result<Vec<u8>> {
    let mut imports: BTreeSet<String> = BTreeSet::new();
    imports.insert("ext.data_model".to_string());
    for dep in collect_type_deps(type_index, td)? {
        imports.insert(dep);
    }

    let mut exports: BTreeSet<String> = BTreeSet::new();
    let mut functions: Vec<FunctionDef> = Vec::new();

    let shape_note = format!(
        "x07schema:{}@v{};encoding=ext.data_model;enum_seq_tag_v1=1;max_depth={};max_seq_items={}",
        td.type_id, td.version, td.max_depth, td.max_seq_items
    );
    add_export_defn(
        td,
        "shape_note_v1",
        &mut exports,
        &mut functions,
        vec![],
        Ty::Bytes,
        e_bytes_lit(&shape_note),
    );

    add_export_defn(
        td,
        "err_base_v1",
        &mut exports,
        &mut functions,
        vec![],
        Ty::I32,
        e_int(td.err_base),
    );

    let code_doc_invalid = td.err_base + 1;
    let code_root_kind = td.err_base + 2;
    let code_doc_too_large = td.err_base + 3;
    let code_enum_tag_invalid = td.err_base + 20;
    let code_enum_payload_invalid = td.err_base + 21;

    for (name, code) in [
        ("code_doc_invalid_v1", code_doc_invalid),
        ("code_root_kind_v1", code_root_kind),
        ("code_doc_too_large_v1", code_doc_too_large),
        ("code_enum_tag_invalid_v1", code_enum_tag_invalid),
        ("code_enum_payload_invalid_v1", code_enum_payload_invalid),
    ] {
        add_export_defn(
            td,
            name,
            &mut exports,
            &mut functions,
            vec![],
            Ty::I32,
            e_int(code),
        );
    }

    functions.push(gen_cmp_bytes_range(td)?);

    if td.schema_version == SchemaVersion::SpecRows020
        && td.variants.iter().any(|v| match &v.payload {
            VariantPayloadDef::Unit => false,
            VariantPayloadDef::Value { ty, .. } => match ty {
                FieldTy::Number => true,
                FieldTy::Seq { elem } => matches!(elem.as_ref(), FieldTy::Number),
                FieldTy::Bool | FieldTy::Bytes | FieldTy::Struct { .. } => false,
            },
        })
    {
        let Some(style) = td.number_style_default_v1 else {
            anyhow::bail!("internal error: specrows@0.2.0 requires number_style_default_v1");
        };
        match style {
            NumberStyleV1::IntAsciiV1 => functions.push(gen_is_canon_int_ascii_v1(td)?),
            NumberStyleV1::UIntAsciiV1 => functions.push(gen_is_canon_uint_ascii_v1(td)?),
        }
    }

    functions.push(gen_enum_tag_view_at(td)?);
    functions.push(gen_enum_payload_value_view_at(td)?);

    add_export_name(
        td,
        &format!("{}.get_tag_view_v1", td.module_id),
        &mut exports,
    );
    functions.push(gen_enum_get_tag_view(td)?);

    add_export_name(
        td,
        &format!("{}.get_payload_value_view_v1", td.module_id),
        &mut exports,
    );
    functions.push(gen_enum_get_payload_value_view(td)?);

    add_export_name(
        td,
        &format!("{}.validate_doc_v1", td.module_id),
        &mut exports,
    );
    functions.push(gen_enum_validate_doc(type_index, td)?);

    add_export_name(
        td,
        &format!("{}.validate_value_v1", td.module_id),
        &mut exports,
    );
    functions.push(gen_enum_validate_value(type_index, td)?);

    add_export_name(td, &format!("{}.encode_doc_v1", td.module_id), &mut exports);
    functions.push(gen_enum_encode_doc(td)?);

    add_export_name(
        td,
        &format!("{}.encode_value_v1", td.module_id),
        &mut exports,
    );
    functions.push(gen_enum_encode_value(td)?);

    let mut file = X07AstFile {
        kind: X07AstKind::Module,
        module_id: td.module_id.clone(),
        imports,
        exports,
        functions,
        async_functions: Vec::new(),
        extern_functions: Vec::new(),
        solve: None,
        meta: BTreeMap::new(),
    };

    x07c::x07ast::canonicalize_x07ast_file(&mut file);
    let mut v = x07c::x07ast::x07ast_file_to_value(&file);
    x07c::x07ast::canon_value_jcs(&mut v);
    let mut bytes = serde_json::to_string(&v)?.into_bytes();
    if bytes.last() != Some(&b'\n') {
        bytes.push(b'\n');
    }
    Ok(bytes)
}

fn generate_tests_module(type_index: &TypeIndex, td: &TypeDef) -> Result<Vec<u8>> {
    match td.kind {
        TypeKind::Struct => generate_tests_module_struct(type_index, td),
        TypeKind::Enum => generate_tests_module_enum(type_index, td),
    }
}

fn generate_tests_module_struct(type_index: &TypeIndex, td: &TypeDef) -> Result<Vec<u8>> {
    let mut imports: BTreeSet<String> = BTreeSet::new();
    imports.insert("std.test".to_string());
    imports.insert("ext.data_model".to_string());
    imports.insert(td.module_id.clone());
    for dep in collect_transitive_type_deps(type_index, td)? {
        imports.insert(dep);
    }

    let mut exports: BTreeSet<String> = BTreeSet::new();
    let mut functions: Vec<FunctionDef> = Vec::new();

    add_export_name(
        td,
        &format!("{}.test_vectors_v1", td.tests_module_id),
        &mut exports,
    );
    add_export_name(
        td,
        &format!("{}.test_negative_v1", td.tests_module_id),
        &mut exports,
    );

    for ex in &td.examples {
        functions.push(gen_golden_doc(type_index, td, ex)?);
    }
    functions.push(gen_test_negative(type_index, td)?);
    functions.push(gen_test_vectors(type_index, td)?);

    let mut file = X07AstFile {
        kind: X07AstKind::Module,
        module_id: td.tests_module_id.clone(),
        imports,
        exports,
        functions,
        async_functions: Vec::new(),
        extern_functions: Vec::new(),
        solve: None,
        meta: BTreeMap::new(),
    };

    x07c::x07ast::canonicalize_x07ast_file(&mut file);
    let mut v = x07c::x07ast::x07ast_file_to_value(&file);
    x07c::x07ast::canon_value_jcs(&mut v);
    let mut bytes = serde_json::to_string(&v)?.into_bytes();
    if bytes.last() != Some(&b'\n') {
        bytes.push(b'\n');
    }
    Ok(bytes)
}

fn generate_tests_module_enum(type_index: &TypeIndex, td: &TypeDef) -> Result<Vec<u8>> {
    let mut imports: BTreeSet<String> = BTreeSet::new();
    imports.insert("std.test".to_string());
    imports.insert("ext.data_model".to_string());
    imports.insert(td.module_id.clone());
    for dep in collect_transitive_type_deps(type_index, td)? {
        imports.insert(dep);
    }

    let mut exports: BTreeSet<String> = BTreeSet::new();
    let mut functions: Vec<FunctionDef> = Vec::new();

    add_export_name(
        td,
        &format!("{}.test_vectors_v1", td.tests_module_id),
        &mut exports,
    );
    add_export_name(
        td,
        &format!("{}.test_negative_v1", td.tests_module_id),
        &mut exports,
    );

    for ex in &td.examples {
        functions.push(gen_golden_doc_enum(type_index, td, ex)?);
    }
    functions.push(gen_test_negative_enum(td)?);
    functions.push(gen_test_vectors_enum(type_index, td)?);

    let mut file = X07AstFile {
        kind: X07AstKind::Module,
        module_id: td.tests_module_id.clone(),
        imports,
        exports,
        functions,
        async_functions: Vec::new(),
        extern_functions: Vec::new(),
        solve: None,
        meta: BTreeMap::new(),
    };

    x07c::x07ast::canonicalize_x07ast_file(&mut file);
    let mut v = x07c::x07ast::x07ast_file_to_value(&file);
    x07c::x07ast::canon_value_jcs(&mut v);
    let mut bytes = serde_json::to_string(&v)?.into_bytes();
    if bytes.last() != Some(&b'\n') {
        bytes.push(b'\n');
    }
    Ok(bytes)
}

fn add_export_defn(
    td: &TypeDef,
    name_suffix: &str,
    exports: &mut BTreeSet<String>,
    functions: &mut Vec<FunctionDef>,
    params: Vec<FunctionParam>,
    ret_ty: Ty,
    body: Expr,
) {
    let name = format!("{}.{}", td.module_id, name_suffix);
    add_export_name(td, &name, exports);
    functions.push(FunctionDef {
        name,
        params,
        ret_ty,
        body,
    });
}

fn add_export_name(_td: &TypeDef, name: &str, exports: &mut BTreeSet<String>) {
    exports.insert(name.to_string());
}

fn add_field_code_exports(
    td: &TypeDef,
    f: &FieldDef,
    exports: &mut BTreeSet<String>,
    functions: &mut Vec<FunctionDef>,
) {
    let base = td.err_base + f.id * 100;
    let code_missing = base + 10;
    let code_kind = base + 11;
    let code_too_long = base + 12;
    let code_bool_value = base + 13;
    let code_noncanonical_number = base + 14;

    add_export_defn(
        td,
        &format!("code_kind_{}_v1", f.name),
        exports,
        functions,
        vec![],
        Ty::I32,
        e_int(code_kind),
    );

    add_export_defn(
        td,
        &format!("code_too_long_{}_v1", f.name),
        exports,
        functions,
        vec![],
        Ty::I32,
        e_int(code_too_long),
    );

    if f.required {
        add_export_defn(
            td,
            &format!("code_missing_{}_v1", f.name),
            exports,
            functions,
            vec![],
            Ty::I32,
            e_int(code_missing),
        );
    }
    if matches!(f.ty, FieldTy::Bool)
        || matches!(f.ty, FieldTy::Seq { ref elem } if matches!(elem.as_ref(), FieldTy::Bool))
    {
        add_export_defn(
            td,
            &format!("code_bool_value_{}_v1", f.name),
            exports,
            functions,
            vec![],
            Ty::I32,
            e_int(code_bool_value),
        );
    }
    if f.number_style.is_some() {
        add_export_defn(
            td,
            &format!("code_noncanonical_number_{}_v1", f.name),
            exports,
            functions,
            vec![],
            Ty::I32,
            e_int(code_noncanonical_number),
        );
    }
}

fn gen_cmp_bytes_range(td: &TypeDef) -> Result<FunctionDef> {
    let name = format!("{}._cmp_bytes_range_v1", td.module_id);
    let params = vec![
        FunctionParam {
            name: "a".to_string(),
            ty: Ty::BytesView,
        },
        FunctionParam {
            name: "a_start".to_string(),
            ty: Ty::I32,
        },
        FunctionParam {
            name: "a_len".to_string(),
            ty: Ty::I32,
        },
        FunctionParam {
            name: "b".to_string(),
            ty: Ty::BytesView,
        },
        FunctionParam {
            name: "b_start".to_string(),
            ty: Ty::I32,
        },
        FunctionParam {
            name: "b_len".to_string(),
            ty: Ty::I32,
        },
    ];

    let body = e_begin(vec![
        e_let(
            "min_len",
            e_if(
                e_call("<u", vec![e_ident("a_len"), e_ident("b_len")]),
                e_ident("a_len"),
                e_ident("b_len"),
            ),
        ),
        e_for(
            "i",
            e_int(0),
            e_ident("min_len"),
            e_begin(vec![
                e_let(
                    "ac",
                    e_call(
                        "view.get_u8",
                        vec![
                            e_ident("a"),
                            e_call("+", vec![e_ident("a_start"), e_ident("i")]),
                        ],
                    ),
                ),
                e_let(
                    "bc",
                    e_call(
                        "view.get_u8",
                        vec![
                            e_ident("b"),
                            e_call("+", vec![e_ident("b_start"), e_ident("i")]),
                        ],
                    ),
                ),
                e_if(
                    e_call("<", vec![e_ident("ac"), e_ident("bc")]),
                    e_return(e_int(-1)),
                    e_int(0),
                ),
                e_if(
                    e_call(">", vec![e_ident("ac"), e_ident("bc")]),
                    e_return(e_int(1)),
                    e_int(0),
                ),
                e_int(0),
            ]),
        ),
        e_if(
            e_call("<", vec![e_ident("a_len"), e_ident("b_len")]),
            e_int(-1),
            e_if(
                e_call(">", vec![e_ident("a_len"), e_ident("b_len")]),
                e_int(1),
                e_int(0),
            ),
        ),
    ]);

    Ok(FunctionDef {
        name,
        params,
        ret_ty: Ty::I32,
        body,
    })
}

fn gen_is_canon_uint_ascii_v1(td: &TypeDef) -> Result<FunctionDef> {
    Ok(FunctionDef {
        name: format!("{}._is_canon_uint_ascii_v1", td.module_id),
        params: vec![
            FunctionParam {
                name: "v".to_string(),
                ty: Ty::BytesView,
            },
            FunctionParam {
                name: "max_bytes".to_string(),
                ty: Ty::I32,
            },
        ],
        ret_ty: Ty::I32,
        body: e_begin(vec![
            e_let("n", e_call("view.len", vec![e_ident("v")])),
            e_if(
                e_call("<", vec![e_ident("n"), e_int(1)]),
                e_return(e_int(0)),
                e_int(0),
            ),
            e_if(
                e_call(">", vec![e_ident("n"), e_ident("max_bytes")]),
                e_return(e_int(0)),
                e_int(0),
            ),
            e_let("first", e_call("view.get_u8", vec![e_ident("v"), e_int(0)])),
            e_if(
                e_call("<", vec![e_ident("first"), e_int(48)]),
                e_return(e_int(0)),
                e_int(0),
            ),
            e_if(
                e_call(">", vec![e_ident("first"), e_int(57)]),
                e_return(e_int(0)),
                e_int(0),
            ),
            e_if(
                e_call("=", vec![e_ident("first"), e_int(48)]),
                e_if(
                    e_call("!=", vec![e_ident("n"), e_int(1)]),
                    e_return(e_int(0)),
                    e_int(0),
                ),
                e_int(0),
            ),
            e_for(
                "i",
                e_int(0),
                e_ident("n"),
                e_begin(vec![
                    e_let("c", e_call("view.get_u8", vec![e_ident("v"), e_ident("i")])),
                    e_if(
                        e_call("<", vec![e_ident("c"), e_int(48)]),
                        e_return(e_int(0)),
                        e_int(0),
                    ),
                    e_if(
                        e_call(">", vec![e_ident("c"), e_int(57)]),
                        e_return(e_int(0)),
                        e_int(0),
                    ),
                    e_int(0),
                ]),
            ),
            e_int(1),
        ]),
    })
}

fn gen_is_canon_int_ascii_v1(td: &TypeDef) -> Result<FunctionDef> {
    Ok(FunctionDef {
        name: format!("{}._is_canon_int_ascii_v1", td.module_id),
        params: vec![
            FunctionParam {
                name: "v".to_string(),
                ty: Ty::BytesView,
            },
            FunctionParam {
                name: "max_bytes".to_string(),
                ty: Ty::I32,
            },
        ],
        ret_ty: Ty::I32,
        body: e_begin(vec![
            e_let("n", e_call("view.len", vec![e_ident("v")])),
            e_if(
                e_call("<", vec![e_ident("n"), e_int(1)]),
                e_return(e_int(0)),
                e_int(0),
            ),
            e_if(
                e_call(">", vec![e_ident("n"), e_ident("max_bytes")]),
                e_return(e_int(0)),
                e_int(0),
            ),
            e_let("i0", e_int(0)),
            e_let("has_minus", e_int(0)),
            e_let("c0", e_call("view.get_u8", vec![e_ident("v"), e_int(0)])),
            e_if(
                e_call("=", vec![e_ident("c0"), e_int(45)]),
                e_begin(vec![
                    e_if(
                        e_call("<", vec![e_ident("n"), e_int(2)]),
                        e_return(e_int(0)),
                        e_int(0),
                    ),
                    e_set("i0", e_int(1)),
                    e_set("has_minus", e_int(1)),
                    e_int(0),
                ]),
                e_int(0),
            ),
            e_let(
                "first",
                e_call("view.get_u8", vec![e_ident("v"), e_ident("i0")]),
            ),
            e_if(
                e_call("<", vec![e_ident("first"), e_int(48)]),
                e_return(e_int(0)),
                e_int(0),
            ),
            e_if(
                e_call(">", vec![e_ident("first"), e_int(57)]),
                e_return(e_int(0)),
                e_int(0),
            ),
            e_if(
                e_call("=", vec![e_ident("first"), e_int(48)]),
                e_begin(vec![
                    e_if(
                        e_call("=", vec![e_ident("has_minus"), e_int(1)]),
                        e_return(e_int(0)),
                        e_int(0),
                    ),
                    e_if(
                        e_call("!=", vec![e_ident("n"), e_int(1)]),
                        e_return(e_int(0)),
                        e_int(0),
                    ),
                    e_return(e_int(1)),
                ]),
                e_int(0),
            ),
            e_for(
                "i",
                e_ident("i0"),
                e_ident("n"),
                e_begin(vec![
                    e_let("c", e_call("view.get_u8", vec![e_ident("v"), e_ident("i")])),
                    e_if(
                        e_call("<", vec![e_ident("c"), e_int(48)]),
                        e_return(e_int(0)),
                        e_int(0),
                    ),
                    e_if(
                        e_call(">", vec![e_ident("c"), e_int(57)]),
                        e_return(e_int(0)),
                        e_int(0),
                    ),
                    e_int(0),
                ]),
            ),
            e_int(1),
        ]),
    })
}

fn gen_add_entry(td: &TypeDef) -> Result<FunctionDef> {
    let name = format!("{}._add_entry_v1", td.module_id);
    let params = vec![
        FunctionParam {
            name: "entries".to_string(),
            ty: Ty::VecU8,
        },
        FunctionParam {
            name: "key".to_string(),
            ty: Ty::Bytes,
        },
        FunctionParam {
            name: "val".to_string(),
            ty: Ty::Bytes,
        },
    ];

    let body = e_begin(vec![
        e_let("out", e_ident("entries")),
        e_set(
            "out",
            e_call(
                "vec_u8.extend_bytes",
                vec![
                    e_ident("out"),
                    e_call(
                        "codec.write_u32_le",
                        vec![e_call("bytes.len", vec![e_ident("key")])],
                    ),
                ],
            ),
        ),
        e_set(
            "out",
            e_call("vec_u8.extend_bytes", vec![e_ident("out"), e_ident("key")]),
        ),
        e_set(
            "out",
            e_call(
                "vec_u8.extend_bytes",
                vec![
                    e_ident("out"),
                    e_call(
                        "codec.write_u32_le",
                        vec![e_call("bytes.len", vec![e_ident("val")])],
                    ),
                ],
            ),
        ),
        e_set(
            "out",
            e_call("vec_u8.extend_bytes", vec![e_ident("out"), e_ident("val")]),
        ),
        e_ident("out"),
    ]);

    Ok(FunctionDef {
        name,
        params,
        ret_ty: Ty::VecU8,
        body,
    })
}

fn gen_validate_doc(td: &TypeDef) -> Result<FunctionDef> {
    let name = format!("{}.validate_doc_v1", td.module_id);
    let params = vec![FunctionParam {
        name: "doc".to_string(),
        ty: Ty::BytesView,
    }];

    let code_doc_invalid = td.err_base + 1;
    let code_doc_too_large = td.err_base + 3;
    let mut stmts: Vec<Expr> = Vec::new();
    stmts.push(e_let("n", e_call("view.len", vec![e_ident("doc")])));
    stmts.push(e_if(
        e_call(">", vec![e_ident("n"), e_int(td.max_doc_bytes)]),
        e_return(e_call("result_i32.err", vec![e_int(code_doc_too_large)])),
        e_int(0),
    ));
    stmts.push(e_if(
        e_call("ext.data_model.doc_is_err", vec![e_ident("doc")]),
        e_return(e_call("result_i32.err", vec![e_int(code_doc_invalid)])),
        e_int(0),
    ));
    stmts.push(e_let(
        "value",
        e_call(
            "view.slice",
            vec![
                e_ident("doc"),
                e_int(1),
                e_call("-", vec![e_ident("n"), e_int(1)]),
            ],
        ),
    ));
    stmts.push(e_call(
        &format!("{}.validate_value_v1", td.module_id),
        vec![e_ident("value")],
    ));

    Ok(FunctionDef {
        name,
        params,
        ret_ty: Ty::ResultI32,
        body: e_begin(stmts),
    })
}

fn gen_validate_value(type_index: &TypeIndex, td: &TypeDef) -> Result<FunctionDef> {
    let name = format!("{}.validate_value_v1", td.module_id);
    let params = vec![FunctionParam {
        name: "value".to_string(),
        ty: Ty::BytesView,
    }];

    let code_root_kind = td.err_base + 2;
    let code_doc_too_large = td.err_base + 3;
    let code_unknown_field = td.err_base + 30;
    let code_noncanonical_map = td.err_base + 31;
    let code_dup_field = td.err_base + 32;
    let code_map_too_many_entries = td.err_base + 33;

    let mut stmts: Vec<Expr> = vec![e_let("n", e_call("view.len", vec![e_ident("value")]))];
    stmts.push(e_if(
        e_call(
            ">u",
            vec![
                e_call("+", vec![e_ident("n"), e_int(1)]),
                e_int(td.max_doc_bytes),
            ],
        ),
        e_return(e_call("result_i32.err", vec![e_int(code_doc_too_large)])),
        e_int(0),
    ));

    stmts.push(e_let("root_off", e_int(0)));
    stmts.push(e_if(
        e_call(
            "!=",
            vec![
                e_call(
                    "ext.data_model.kind_at",
                    vec![e_ident("value"), e_ident("root_off")],
                ),
                e_int(5),
            ],
        ),
        e_return(e_call("result_i32.err", vec![e_int(code_root_kind)])),
        e_int(0),
    ));
    stmts.push(e_let(
        "map_len",
        e_call(
            "ext.data_model.map_len",
            vec![e_ident("value"), e_ident("root_off")],
        ),
    ));
    stmts.push(e_if(
        e_call("<", vec![e_ident("map_len"), e_int(0)]),
        e_return(e_call("result_i32.err", vec![e_int(code_root_kind)])),
        e_int(0),
    ));
    stmts.push(e_if(
        e_call(">", vec![e_ident("map_len"), e_int(td.max_map_entries)]),
        e_return(e_call(
            "result_i32.err",
            vec![e_int(code_map_too_many_entries)],
        )),
        e_int(0),
    ));

    for f in &td.fields {
        stmts.push(e_let(&format!("off_{}", f.name), e_int(-1)));
    }
    for f in &td.fields {
        stmts.push(e_let(&format!("k_lit_{}", f.name), e_bytes_lit(&f.name)));
    }
    stmts.push(e_let(
        "pos",
        e_call("+", vec![e_ident("root_off"), e_int(5)]),
    ));
    stmts.push(e_let("prev_start", e_int(0)));
    stmts.push(e_let("prev_len", e_int(0)));

    let mut for_body: Vec<Expr> = Vec::new();
    for_body.push(e_if(
        e_call(
            ">=u",
            vec![
                e_call("+", vec![e_ident("pos"), e_int(4)]),
                e_call("+", vec![e_ident("n"), e_int(1)]),
            ],
        ),
        e_return(e_call("result_i32.err", vec![e_int(code_root_kind)])),
        e_int(0),
    ));
    for_body.push(e_let(
        "k_len",
        e_call("codec.read_u32_le", vec![e_ident("value"), e_ident("pos")]),
    ));
    for_body.push(e_if(
        e_call("<", vec![e_ident("k_len"), e_int(0)]),
        e_return(e_call("result_i32.err", vec![e_int(code_root_kind)])),
        e_int(0),
    ));
    for_body.push(e_let(
        "k_start",
        e_call("+", vec![e_ident("pos"), e_int(4)]),
    ));
    for_body.push(e_let(
        "k_end",
        e_call("+", vec![e_ident("k_start"), e_ident("k_len")]),
    ));
    for_body.push(e_if(
        e_call(
            ">=u",
            vec![e_ident("k_end"), e_call("+", vec![e_ident("n"), e_int(1)])],
        ),
        e_return(e_call("result_i32.err", vec![e_int(code_root_kind)])),
        e_int(0),
    ));
    for_body.push(e_if(
        e_call(">", vec![e_ident("i"), e_int(0)]),
        e_begin(vec![
            e_let(
                "cmp",
                e_call(
                    &format!("{}._cmp_bytes_range_v1", td.module_id),
                    vec![
                        e_ident("value"),
                        e_ident("prev_start"),
                        e_ident("prev_len"),
                        e_ident("value"),
                        e_ident("k_start"),
                        e_ident("k_len"),
                    ],
                ),
            ),
            e_if(
                e_call(">", vec![e_ident("cmp"), e_int(0)]),
                e_return(e_call("result_i32.err", vec![e_int(code_noncanonical_map)])),
                e_int(0),
            ),
            e_if(
                e_call("=", vec![e_ident("cmp"), e_int(0)]),
                e_return(e_call("result_i32.err", vec![e_int(code_dup_field)])),
                e_int(0),
            ),
            e_int(0),
        ]),
        e_int(0),
    ));
    for_body.push(e_set("prev_start", e_ident("k_start")));
    for_body.push(e_set("prev_len", e_ident("k_len")));
    for_body.push(e_let(
        "k_view",
        e_call(
            "view.slice",
            vec![e_ident("value"), e_ident("k_start"), e_ident("k_len")],
        ),
    ));

    let unknown_else = if td.allow_unknown_fields {
        e_int(0)
    } else {
        e_return(e_call("result_i32.err", vec![e_int(code_unknown_field)]))
    };
    let key_chain = build_key_match_chain(td, &td.fields, e_int(code_dup_field), unknown_else)?;
    for_body.push(key_chain);

    for_body.push(e_set(
        "pos",
        e_call(
            "ext.data_model.skip_value",
            vec![e_ident("value"), e_ident("k_end")],
        ),
    ));
    for_body.push(e_if(
        e_call("<", vec![e_ident("pos"), e_int(0)]),
        e_return(e_call("result_i32.err", vec![e_int(code_root_kind)])),
        e_int(0),
    ));
    for_body.push(e_int(0));

    stmts.push(e_for("i", e_int(0), e_ident("map_len"), e_begin(for_body)));

    for f in &td.fields {
        if f.required {
            let code_missing = td.err_base + f.id * 100 + 10;
            stmts.push(e_if(
                e_call("<", vec![e_ident(format!("off_{}", f.name)), e_int(0)]),
                e_return(e_call("result_i32.err", vec![e_int(code_missing)])),
                e_int(0),
            ));
        }
    }

    for f in &td.fields {
        stmts.extend(gen_validate_field(type_index, td, f, "value")?);
    }

    stmts.push(e_call("result_i32.ok", vec![e_int(0)]));

    Ok(FunctionDef {
        name,
        params,
        ret_ty: Ty::ResultI32,
        body: e_begin(stmts),
    })
}

fn gen_validate_field(
    type_index: &TypeIndex,
    td: &TypeDef,
    f: &FieldDef,
    view_name: &str,
) -> Result<Vec<Expr>> {
    let off = e_ident(format!("off_{}", f.name));
    let base = td.err_base + f.id * 100;
    let code_kind = base + 11;
    let code_too_long = base + 12;
    let code_bool_value = base + 13;
    let code_noncanonical_number = base + 14;

    let mut stmts: Vec<Expr> = Vec::new();

    match &f.ty {
        FieldTy::Bool => {
            let block = e_begin(vec![
                e_if(
                    e_call(
                        "!=",
                        vec![
                            e_call(
                                "ext.data_model.kind_at",
                                vec![e_ident(view_name), off.clone()],
                            ),
                            e_int(1),
                        ],
                    ),
                    e_return(e_call("result_i32.err", vec![e_int(code_kind)])),
                    e_int(0),
                ),
                e_if(
                    e_call(
                        ">=u",
                        vec![
                            e_call("+", vec![off.clone(), e_int(2)]),
                            e_call("+", vec![e_ident("n"), e_int(1)]),
                        ],
                    ),
                    e_return(e_call("result_i32.err", vec![e_int(code_kind)])),
                    e_int(0),
                ),
                e_let(
                    &format!("b_{}", f.name),
                    e_call(
                        "view.get_u8",
                        vec![e_ident(view_name), e_call("+", vec![off.clone(), e_int(1)])],
                    ),
                ),
                e_if(
                    e_if(
                        e_call("=", vec![e_ident(format!("b_{}", f.name)), e_int(0)]),
                        e_int(1),
                        e_call("=", vec![e_ident(format!("b_{}", f.name)), e_int(1)]),
                    ),
                    e_int(0),
                    e_return(e_call("result_i32.err", vec![e_int(code_bool_value)])),
                ),
                e_int(0),
            ]);

            if f.required {
                stmts.push(block);
            } else {
                stmts.push(e_if(e_call(">=", vec![off, e_int(0)]), block, e_int(0)));
            }
        }
        FieldTy::Bytes | FieldTy::Number => {
            let kind = f.ty.kind_byte();
            let max_bytes = f.max_bytes.unwrap_or(0);

            let len_var = format!("len_{}", f.name);
            let start_var = format!("start_{}", f.name);

            let mut inner: Vec<Expr> = vec![e_if(
                e_call(
                    "!=",
                    vec![
                        e_call(
                            "ext.data_model.kind_at",
                            vec![e_ident(view_name), off.clone()],
                        ),
                        e_int(kind),
                    ],
                ),
                e_return(e_call("result_i32.err", vec![e_int(code_kind)])),
                e_int(0),
            )];
            inner.push(e_if(
                e_call(
                    ">=u",
                    vec![
                        e_call("+", vec![off.clone(), e_int(5)]),
                        e_call("+", vec![e_ident("n"), e_int(1)]),
                    ],
                ),
                e_return(e_call("result_i32.err", vec![e_int(code_kind)])),
                e_int(0),
            ));
            inner.push(e_let(
                &len_var,
                e_call(
                    "codec.read_u32_le",
                    vec![e_ident(view_name), e_call("+", vec![off.clone(), e_int(1)])],
                ),
            ));
            inner.push(e_if(
                e_call("<", vec![e_ident(&len_var), e_int(0)]),
                e_return(e_call("result_i32.err", vec![e_int(code_kind)])),
                e_int(0),
            ));
            inner.push(e_let(&start_var, e_call("+", vec![off.clone(), e_int(5)])));
            inner.push(e_if(
                e_call(
                    ">=u",
                    vec![
                        e_call("+", vec![e_ident(&start_var), e_ident(&len_var)]),
                        e_call("+", vec![e_ident("n"), e_int(1)]),
                    ],
                ),
                e_return(e_call("result_i32.err", vec![e_int(code_kind)])),
                e_int(0),
            ));
            inner.push(e_if(
                e_call(">", vec![e_ident(&len_var), e_int(max_bytes)]),
                e_return(e_call("result_i32.err", vec![e_int(code_too_long)])),
                e_int(0),
            ));

            if matches!(f.ty, FieldTy::Number) {
                if let Some(style) = f.number_style {
                    let canon_fn = match style {
                        NumberStyleV1::IntAsciiV1 => {
                            format!("{}._is_canon_int_ascii_v1", td.module_id)
                        }
                        NumberStyleV1::UIntAsciiV1 => {
                            format!("{}._is_canon_uint_ascii_v1", td.module_id)
                        }
                    };
                    inner.push(e_let(
                        "num_view",
                        e_call(
                            "view.slice",
                            vec![e_ident(view_name), e_ident(&start_var), e_ident(&len_var)],
                        ),
                    ));
                    inner.push(e_let(
                        "is_canon",
                        e_call(&canon_fn, vec![e_ident("num_view"), e_int(max_bytes)]),
                    ));
                    inner.push(e_if(
                        e_call("=", vec![e_ident("is_canon"), e_int(0)]),
                        e_return(e_call(
                            "result_i32.err",
                            vec![e_int(code_noncanonical_number)],
                        )),
                        e_int(0),
                    ));
                }
            }

            inner.push(e_int(0));
            let block = e_begin(inner);

            if f.required {
                stmts.push(block);
            } else {
                stmts.push(e_if(e_call(">=", vec![off, e_int(0)]), block, e_int(0)));
            }
        }
        FieldTy::Struct { type_id } => {
            let expected_kind = f.ty.kind_byte();
            let Some(dep) = type_index.get(type_id) else {
                anyhow::bail!(
                    "{}: unresolved struct field type reference: {:?}",
                    td.module_id,
                    type_id
                );
            };
            let validate_fn = format!("{}.validate_value_v1", dep.module_id);
            let end_var = format!("end_{}", f.name);
            let slice_var = format!("slice_{}", f.name);

            let block = e_begin(vec![
                e_if(
                    e_call(
                        "!=",
                        vec![
                            e_call(
                                "ext.data_model.kind_at",
                                vec![e_ident(view_name), off.clone()],
                            ),
                            e_int(expected_kind),
                        ],
                    ),
                    e_return(e_call("result_i32.err", vec![e_int(code_kind)])),
                    e_int(0),
                ),
                e_let(
                    &end_var,
                    e_call(
                        "ext.data_model.skip_value",
                        vec![e_ident(view_name), off.clone()],
                    ),
                ),
                e_if(
                    e_call("<", vec![e_ident(&end_var), e_int(0)]),
                    e_return(e_call("result_i32.err", vec![e_int(code_kind)])),
                    e_int(0),
                ),
                e_let(
                    &slice_var,
                    e_call(
                        "view.slice",
                        vec![
                            e_ident(view_name),
                            off.clone(),
                            e_call("-", vec![e_ident(&end_var), off.clone()]),
                        ],
                    ),
                ),
                e_call("try", vec![e_call(&validate_fn, vec![e_ident(&slice_var)])]),
                e_int(0),
            ]);
            if f.required {
                stmts.push(block);
            } else {
                stmts.push(e_if(e_call(">=", vec![off, e_int(0)]), block, e_int(0)));
            }
        }
        FieldTy::Seq { elem } => {
            let expected_kind = f.ty.kind_byte();
            let max_items = f.max_items.unwrap_or(td.max_seq_items);
            let seq_len_var = format!("seq_len_{}", f.name);
            let idx_var = format!("j_{}", f.name);
            let elem_off_var = format!("eo_{}", f.name);

            let mut elem_stmts: Vec<Expr> = Vec::new();
            elem_stmts.push(e_let(
                &elem_off_var,
                e_call(
                    "ext.data_model.seq_get",
                    vec![e_ident(view_name), off.clone(), e_ident(&idx_var)],
                ),
            ));
            elem_stmts.push(e_if(
                e_call("<", vec![e_ident(&elem_off_var), e_int(0)]),
                e_return(e_call("result_i32.err", vec![e_int(code_kind)])),
                e_int(0),
            ));

            match elem.as_ref() {
                FieldTy::Bool => {
                    let b_var = format!("b_{}_elem", f.name);
                    elem_stmts.push(e_if(
                        e_call(
                            "!=",
                            vec![
                                e_call(
                                    "ext.data_model.kind_at",
                                    vec![e_ident(view_name), e_ident(&elem_off_var)],
                                ),
                                e_int(1),
                            ],
                        ),
                        e_return(e_call("result_i32.err", vec![e_int(code_kind)])),
                        e_int(0),
                    ));
                    elem_stmts.push(e_if(
                        e_call(
                            ">=u",
                            vec![
                                e_call("+", vec![e_ident(&elem_off_var), e_int(2)]),
                                e_call("+", vec![e_ident("n"), e_int(1)]),
                            ],
                        ),
                        e_return(e_call("result_i32.err", vec![e_int(code_kind)])),
                        e_int(0),
                    ));
                    elem_stmts.push(e_let(
                        &b_var,
                        e_call(
                            "view.get_u8",
                            vec![
                                e_ident(view_name),
                                e_call("+", vec![e_ident(&elem_off_var), e_int(1)]),
                            ],
                        ),
                    ));
                    elem_stmts.push(e_if(
                        e_if(
                            e_call("=", vec![e_ident(&b_var), e_int(0)]),
                            e_int(1),
                            e_call("=", vec![e_ident(&b_var), e_int(1)]),
                        ),
                        e_int(0),
                        e_return(e_call("result_i32.err", vec![e_int(code_bool_value)])),
                    ));
                }
                FieldTy::Bytes | FieldTy::Number => {
                    let kind = elem.kind_byte();
                    let max_bytes = f.max_bytes.unwrap_or(0);
                    let len_var = format!("len_{}_elem", f.name);
                    let start_var = format!("start_{}_elem", f.name);
                    elem_stmts.push(e_if(
                        e_call(
                            "!=",
                            vec![
                                e_call(
                                    "ext.data_model.kind_at",
                                    vec![e_ident(view_name), e_ident(&elem_off_var)],
                                ),
                                e_int(kind),
                            ],
                        ),
                        e_return(e_call("result_i32.err", vec![e_int(code_kind)])),
                        e_int(0),
                    ));
                    elem_stmts.push(e_if(
                        e_call(
                            ">=u",
                            vec![
                                e_call("+", vec![e_ident(&elem_off_var), e_int(5)]),
                                e_call("+", vec![e_ident("n"), e_int(1)]),
                            ],
                        ),
                        e_return(e_call("result_i32.err", vec![e_int(code_kind)])),
                        e_int(0),
                    ));
                    elem_stmts.push(e_let(
                        &len_var,
                        e_call(
                            "codec.read_u32_le",
                            vec![
                                e_ident(view_name),
                                e_call("+", vec![e_ident(&elem_off_var), e_int(1)]),
                            ],
                        ),
                    ));
                    elem_stmts.push(e_if(
                        e_call("<", vec![e_ident(&len_var), e_int(0)]),
                        e_return(e_call("result_i32.err", vec![e_int(code_kind)])),
                        e_int(0),
                    ));
                    elem_stmts.push(e_let(
                        &start_var,
                        e_call("+", vec![e_ident(&elem_off_var), e_int(5)]),
                    ));
                    elem_stmts.push(e_if(
                        e_call(
                            ">=u",
                            vec![
                                e_call("+", vec![e_ident(&start_var), e_ident(&len_var)]),
                                e_call("+", vec![e_ident("n"), e_int(1)]),
                            ],
                        ),
                        e_return(e_call("result_i32.err", vec![e_int(code_kind)])),
                        e_int(0),
                    ));
                    elem_stmts.push(e_if(
                        e_call(">", vec![e_ident(&len_var), e_int(max_bytes)]),
                        e_return(e_call("result_i32.err", vec![e_int(code_too_long)])),
                        e_int(0),
                    ));
                    if matches!(elem.as_ref(), FieldTy::Number) {
                        if let Some(style) = f.number_style {
                            let canon_fn = match style {
                                NumberStyleV1::IntAsciiV1 => {
                                    format!("{}._is_canon_int_ascii_v1", td.module_id)
                                }
                                NumberStyleV1::UIntAsciiV1 => {
                                    format!("{}._is_canon_uint_ascii_v1", td.module_id)
                                }
                            };
                            elem_stmts.push(e_let(
                                "num_view",
                                e_call(
                                    "view.slice",
                                    vec![
                                        e_ident(view_name),
                                        e_ident(&start_var),
                                        e_ident(&len_var),
                                    ],
                                ),
                            ));
                            elem_stmts.push(e_let(
                                "is_canon",
                                e_call(&canon_fn, vec![e_ident("num_view"), e_int(max_bytes)]),
                            ));
                            elem_stmts.push(e_if(
                                e_call("=", vec![e_ident("is_canon"), e_int(0)]),
                                e_return(e_call(
                                    "result_i32.err",
                                    vec![e_int(code_noncanonical_number)],
                                )),
                                e_int(0),
                            ));
                        }
                    }
                }
                FieldTy::Struct { type_id } => {
                    let Some(dep) = type_index.get(type_id) else {
                        anyhow::bail!(
                            "{}: unresolved struct field type reference: {:?}",
                            td.module_id,
                            type_id
                        );
                    };
                    let validate_fn = format!("{}.validate_value_v1", dep.module_id);
                    let end_var = format!("end_{}_elem", f.name);
                    let slice_var = format!("slice_{}_elem", f.name);
                    elem_stmts.push(e_if(
                        e_call(
                            "!=",
                            vec![
                                e_call(
                                    "ext.data_model.kind_at",
                                    vec![e_ident(view_name), e_ident(&elem_off_var)],
                                ),
                                e_int(5),
                            ],
                        ),
                        e_return(e_call("result_i32.err", vec![e_int(code_kind)])),
                        e_int(0),
                    ));
                    elem_stmts.push(e_let(
                        &end_var,
                        e_call(
                            "ext.data_model.skip_value",
                            vec![e_ident(view_name), e_ident(&elem_off_var)],
                        ),
                    ));
                    elem_stmts.push(e_if(
                        e_call("<", vec![e_ident(&end_var), e_int(0)]),
                        e_return(e_call("result_i32.err", vec![e_int(code_kind)])),
                        e_int(0),
                    ));
                    elem_stmts.push(e_let(
                        &slice_var,
                        e_call(
                            "view.slice",
                            vec![
                                e_ident(view_name),
                                e_ident(&elem_off_var),
                                e_call("-", vec![e_ident(&end_var), e_ident(&elem_off_var)]),
                            ],
                        ),
                    ));
                    elem_stmts.push(e_call(
                        "try",
                        vec![e_call(&validate_fn, vec![e_ident(&slice_var)])],
                    ));
                }
                FieldTy::Seq { .. } => anyhow::bail!(
                    "{}: unsupported seq elem ty for field {:?}",
                    td.module_id,
                    f.name
                ),
            }
            elem_stmts.push(e_int(0));

            let block = e_begin(vec![
                e_if(
                    e_call(
                        "!=",
                        vec![
                            e_call(
                                "ext.data_model.kind_at",
                                vec![e_ident(view_name), off.clone()],
                            ),
                            e_int(expected_kind),
                        ],
                    ),
                    e_return(e_call("result_i32.err", vec![e_int(code_kind)])),
                    e_int(0),
                ),
                e_let(
                    &seq_len_var,
                    e_call(
                        "ext.data_model.seq_len",
                        vec![e_ident(view_name), off.clone()],
                    ),
                ),
                e_if(
                    e_call("<", vec![e_ident(&seq_len_var), e_int(0)]),
                    e_return(e_call("result_i32.err", vec![e_int(code_kind)])),
                    e_int(0),
                ),
                e_if(
                    e_call(">", vec![e_ident(&seq_len_var), e_int(max_items)]),
                    e_return(e_call("result_i32.err", vec![e_int(code_too_long)])),
                    e_int(0),
                ),
                e_for(
                    &idx_var,
                    e_int(0),
                    e_ident(&seq_len_var),
                    e_begin(elem_stmts),
                ),
                e_int(0),
            ]);
            if f.required {
                stmts.push(block);
            } else {
                stmts.push(e_if(e_call(">=", vec![off, e_int(0)]), block, e_int(0)));
            }
        }
    }

    Ok(stmts)
}

fn gen_encode_doc(td: &TypeDef) -> Result<FunctionDef> {
    let name = format!("{}.encode_doc_v1", td.module_id);

    let mut params: Vec<FunctionParam> = Vec::new();
    for f in &td.fields {
        let (param_name, ty) = match (f.required, &f.ty) {
            (true, FieldTy::Bool) => (f.name.clone(), Ty::I32),
            (true, FieldTy::Bytes) | (true, FieldTy::Number) => (f.name.clone(), Ty::BytesView),
            (true, FieldTy::Struct { .. }) | (true, FieldTy::Seq { .. }) => {
                (f.name.clone(), Ty::BytesView)
            }
            (false, FieldTy::Bool) => (format!("{}_opt", f.name), Ty::OptionI32),
            (false, FieldTy::Bytes) | (false, FieldTy::Number) => {
                (format!("{}_opt", f.name), Ty::OptionBytes)
            }
            (false, FieldTy::Struct { .. }) | (false, FieldTy::Seq { .. }) => {
                (format!("{}_opt", f.name), Ty::OptionBytes)
            }
        };
        params.push(FunctionParam {
            name: param_name,
            ty,
        });
    }

    let mut stmts: Vec<Expr> = Vec::new();
    let mut args: Vec<Expr> = Vec::new();
    for f in &td.fields {
        match (f.required, &f.ty) {
            (true, FieldTy::Bool) => args.push(e_ident(f.name.clone())),
            (true, FieldTy::Bytes) | (true, FieldTy::Number) => args.push(e_ident(f.name.clone())),
            (true, FieldTy::Struct { .. }) | (true, FieldTy::Seq { .. }) => {
                args.push(e_ident(f.name.clone()))
            }
            (false, FieldTy::Bool) => args.push(e_ident(format!("{}_opt", f.name))),
            (false, FieldTy::Bytes) | (false, FieldTy::Number) => {
                args.push(e_ident(format!("{}_opt", f.name)))
            }
            (false, FieldTy::Struct { .. }) | (false, FieldTy::Seq { .. }) => {
                args.push(e_ident(format!("{}_opt", f.name)))
            }
        }
    }
    stmts.push(e_let(
        "value",
        e_call(
            "try",
            vec![e_call(&format!("{}.encode_value_v1", td.module_id), args)],
        ),
    ));
    stmts.push(e_call(
        "result_bytes.ok",
        vec![e_call(
            "ext.data_model.doc_ok",
            vec![e_call("bytes.view", vec![e_ident("value")])],
        )],
    ));

    Ok(FunctionDef {
        name,
        params,
        ret_ty: Ty::ResultBytes,
        body: e_begin(stmts),
    })
}

fn gen_encode_value(td: &TypeDef) -> Result<FunctionDef> {
    let name = format!("{}.encode_value_v1", td.module_id);

    let mut params: Vec<FunctionParam> = Vec::new();
    for f in &td.fields {
        let (param_name, ty) = match (f.required, &f.ty) {
            (true, FieldTy::Bool) => (f.name.clone(), Ty::I32),
            (true, FieldTy::Bytes) | (true, FieldTy::Number) => (f.name.clone(), Ty::BytesView),
            (true, FieldTy::Struct { .. }) | (true, FieldTy::Seq { .. }) => {
                (f.name.clone(), Ty::BytesView)
            }
            (false, FieldTy::Bool) => (format!("{}_opt", f.name), Ty::OptionI32),
            (false, FieldTy::Bytes) | (false, FieldTy::Number) => {
                (format!("{}_opt", f.name), Ty::OptionBytes)
            }
            (false, FieldTy::Struct { .. }) | (false, FieldTy::Seq { .. }) => {
                (format!("{}_opt", f.name), Ty::OptionBytes)
            }
        };
        params.push(FunctionParam {
            name: param_name,
            ty,
        });
    }

    let mut stmts: Vec<Expr> = vec![
        e_let("empty", e_call("bytes.alloc", vec![e_int(0)])),
        e_let("count", e_int(0)),
        e_let("entries", e_call("vec_u8.with_capacity", vec![e_int(0)])),
        e_set(
            "entries",
            e_call(
                "vec_u8.extend_bytes",
                vec![
                    e_ident("entries"),
                    e_call("codec.write_u32_le", vec![e_int(0)]),
                ],
            ),
        ),
    ];

    for f in &td.fields {
        stmts.extend(gen_encode_field(td, f)?);
    }

    stmts.push(e_let(
        "count_b",
        e_call("codec.write_u32_le", vec![e_ident("count")]),
    ));
    for i in 0..4 {
        stmts.push(e_set(
            "entries",
            e_call(
                "vec_u8.set",
                vec![
                    e_ident("entries"),
                    e_int(i),
                    e_call("bytes.get_u8", vec![e_ident("count_b"), e_int(i)]),
                ],
            ),
        ));
    }

    let code_root_kind = td.err_base + 2;
    let code_doc_too_large = td.err_base + 3;

    stmts.push(e_let(
        "entries_b",
        e_call("vec_u8.into_bytes", vec![e_ident("entries")]),
    ));
    stmts.push(e_let(
        "map_val",
        e_call(
            "ext.data_model.value_map_from_entries",
            vec![e_call("bytes.view", vec![e_ident("entries_b")])],
        ),
    ));
    stmts.push(e_if(
        e_call(
            "=",
            vec![e_call("bytes.len", vec![e_ident("map_val")]), e_int(0)],
        ),
        e_return(e_call("result_bytes.err", vec![e_int(code_root_kind)])),
        e_int(0),
    ));
    stmts.push(e_if(
        e_call(
            ">u",
            vec![
                e_call(
                    "+",
                    vec![e_call("bytes.len", vec![e_ident("map_val")]), e_int(1)],
                ),
                e_int(td.max_doc_bytes),
            ],
        ),
        e_return(e_call("result_bytes.err", vec![e_int(code_doc_too_large)])),
        e_int(0),
    ));
    stmts.push(e_call("result_bytes.ok", vec![e_ident("map_val")]));

    Ok(FunctionDef {
        name,
        params,
        ret_ty: Ty::ResultBytes,
        body: e_begin(stmts),
    })
}

fn gen_encode_field(td: &TypeDef, f: &FieldDef) -> Result<Vec<Expr>> {
    let mut stmts: Vec<Expr> = Vec::new();
    let base = td.err_base + f.id * 100;
    let code_kind = base + 11;
    let code_too_long = base + 12;
    let code_bool_value = base + 13;
    let code_noncanonical_number = base + 14;

    let key_var = format!("k_{}", f.name);
    let val_var = format!("v_{}", f.name);

    match (f.required, &f.ty) {
        (true, FieldTy::Bool) => {
            stmts.push(e_let(&key_var, e_bytes_lit(&f.name)));
            stmts.push(e_let(
                &val_var,
                e_call(f.ty.value_ctor().expect("scalar"), vec![e_ident(&f.name)]),
            ));
            stmts.push(e_set(
                "entries",
                e_call(
                    &format!("{}._add_entry_v1", td.module_id),
                    vec![e_ident("entries"), e_ident(&key_var), e_ident(&val_var)],
                ),
            ));
            stmts.push(e_set(
                "count",
                e_call("+", vec![e_ident("count"), e_int(1)]),
            ));
        }
        (true, FieldTy::Bytes) | (true, FieldTy::Number) => {
            let max_bytes = f.max_bytes.unwrap_or(0);
            stmts.push(e_if(
                e_call(
                    ">",
                    vec![e_call("view.len", vec![e_ident(&f.name)]), e_int(max_bytes)],
                ),
                e_return(e_call("result_bytes.err", vec![e_int(code_too_long)])),
                e_int(0),
            ));
            if matches!(f.ty, FieldTy::Number) {
                if let Some(style) = f.number_style {
                    let canon_fn = match style {
                        NumberStyleV1::IntAsciiV1 => {
                            format!("{}._is_canon_int_ascii_v1", td.module_id)
                        }
                        NumberStyleV1::UIntAsciiV1 => {
                            format!("{}._is_canon_uint_ascii_v1", td.module_id)
                        }
                    };
                    stmts.push(e_if(
                        e_call(
                            "=",
                            vec![
                                e_call(&canon_fn, vec![e_ident(&f.name), e_int(max_bytes)]),
                                e_int(0),
                            ],
                        ),
                        e_return(e_call(
                            "result_bytes.err",
                            vec![e_int(code_noncanonical_number)],
                        )),
                        e_int(0),
                    ));
                }
            }
            stmts.push(e_let(&key_var, e_bytes_lit(&f.name)));
            stmts.push(e_let(
                &val_var,
                e_call(f.ty.value_ctor().expect("scalar"), vec![e_ident(&f.name)]),
            ));
            stmts.push(e_set(
                "entries",
                e_call(
                    &format!("{}._add_entry_v1", td.module_id),
                    vec![e_ident("entries"), e_ident(&key_var), e_ident(&val_var)],
                ),
            ));
            stmts.push(e_set(
                "count",
                e_call("+", vec![e_ident("count"), e_int(1)]),
            ));
        }
        (true, FieldTy::Struct { .. }) | (true, FieldTy::Seq { .. }) => {
            let expected_kind = f.ty.kind_byte();
            stmts.push(e_if(
                e_call(
                    "<",
                    vec![e_call("view.len", vec![e_ident(&f.name)]), e_int(1)],
                ),
                e_return(e_call("result_bytes.err", vec![e_int(code_kind)])),
                e_int(0),
            ));
            stmts.push(e_if(
                e_call(
                    "!=",
                    vec![
                        e_call("ext.data_model.kind_at", vec![e_ident(&f.name), e_int(0)]),
                        e_int(expected_kind),
                    ],
                ),
                e_return(e_call("result_bytes.err", vec![e_int(code_kind)])),
                e_int(0),
            ));
            if let FieldTy::Seq { elem } = &f.ty {
                let max_items = f.max_items.unwrap_or(td.max_seq_items);
                let seq_len_var = format!("seq_len_{}", f.name);
                let pn_var = format!("pn_{}", f.name);
                let idx_var = format!("j_{}", f.name);
                let elem_off_var = format!("eo_{}", f.name);
                stmts.push(e_let(
                    &seq_len_var,
                    e_call("ext.data_model.seq_len", vec![e_ident(&f.name), e_int(0)]),
                ));
                stmts.push(e_if(
                    e_call("<", vec![e_ident(&seq_len_var), e_int(0)]),
                    e_return(e_call("result_bytes.err", vec![e_int(code_kind)])),
                    e_int(0),
                ));
                stmts.push(e_if(
                    e_call(">", vec![e_ident(&seq_len_var), e_int(max_items)]),
                    e_return(e_call("result_bytes.err", vec![e_int(code_too_long)])),
                    e_int(0),
                ));

                if matches!(
                    elem.as_ref(),
                    FieldTy::Bool | FieldTy::Bytes | FieldTy::Number
                ) {
                    let max_bytes = f.max_bytes.unwrap_or(0);
                    stmts.push(e_let(&pn_var, e_call("view.len", vec![e_ident(&f.name)])));

                    let mut elem_stmts: Vec<Expr> = Vec::new();
                    elem_stmts.push(e_let(
                        &elem_off_var,
                        e_call(
                            "ext.data_model.seq_get",
                            vec![e_ident(&f.name), e_int(0), e_ident(&idx_var)],
                        ),
                    ));
                    elem_stmts.push(e_if(
                        e_call("<", vec![e_ident(&elem_off_var), e_int(0)]),
                        e_return(e_call("result_bytes.err", vec![e_int(code_kind)])),
                        e_int(0),
                    ));

                    match elem.as_ref() {
                        FieldTy::Bool => {
                            let b_var = format!("b_{}_elem", f.name);
                            elem_stmts.push(e_if(
                                e_call(
                                    "!=",
                                    vec![
                                        e_call(
                                            "ext.data_model.kind_at",
                                            vec![e_ident(&f.name), e_ident(&elem_off_var)],
                                        ),
                                        e_int(1),
                                    ],
                                ),
                                e_return(e_call("result_bytes.err", vec![e_int(code_kind)])),
                                e_int(0),
                            ));
                            elem_stmts.push(e_if(
                                e_call(
                                    ">=u",
                                    vec![
                                        e_call("+", vec![e_ident(&elem_off_var), e_int(2)]),
                                        e_call("+", vec![e_ident(&pn_var), e_int(1)]),
                                    ],
                                ),
                                e_return(e_call("result_bytes.err", vec![e_int(code_kind)])),
                                e_int(0),
                            ));
                            elem_stmts.push(e_let(
                                &b_var,
                                e_call(
                                    "view.get_u8",
                                    vec![
                                        e_ident(&f.name),
                                        e_call("+", vec![e_ident(&elem_off_var), e_int(1)]),
                                    ],
                                ),
                            ));
                            elem_stmts.push(e_if(
                                e_if(
                                    e_call("=", vec![e_ident(&b_var), e_int(0)]),
                                    e_int(1),
                                    e_call("=", vec![e_ident(&b_var), e_int(1)]),
                                ),
                                e_int(0),
                                e_return(e_call("result_bytes.err", vec![e_int(code_bool_value)])),
                            ));
                        }
                        FieldTy::Bytes | FieldTy::Number => {
                            let kind = elem.kind_byte();
                            let len_var = format!("len_{}_elem", f.name);
                            let start_var = format!("start_{}_elem", f.name);
                            elem_stmts.push(e_if(
                                e_call(
                                    "!=",
                                    vec![
                                        e_call(
                                            "ext.data_model.kind_at",
                                            vec![e_ident(&f.name), e_ident(&elem_off_var)],
                                        ),
                                        e_int(kind),
                                    ],
                                ),
                                e_return(e_call("result_bytes.err", vec![e_int(code_kind)])),
                                e_int(0),
                            ));
                            elem_stmts.push(e_if(
                                e_call(
                                    ">=u",
                                    vec![
                                        e_call("+", vec![e_ident(&elem_off_var), e_int(5)]),
                                        e_call("+", vec![e_ident(&pn_var), e_int(1)]),
                                    ],
                                ),
                                e_return(e_call("result_bytes.err", vec![e_int(code_kind)])),
                                e_int(0),
                            ));
                            elem_stmts.push(e_let(
                                &len_var,
                                e_call(
                                    "codec.read_u32_le",
                                    vec![
                                        e_ident(&f.name),
                                        e_call("+", vec![e_ident(&elem_off_var), e_int(1)]),
                                    ],
                                ),
                            ));
                            elem_stmts.push(e_if(
                                e_call("<", vec![e_ident(&len_var), e_int(0)]),
                                e_return(e_call("result_bytes.err", vec![e_int(code_kind)])),
                                e_int(0),
                            ));
                            elem_stmts.push(e_let(
                                &start_var,
                                e_call("+", vec![e_ident(&elem_off_var), e_int(5)]),
                            ));
                            elem_stmts.push(e_if(
                                e_call(
                                    ">=u",
                                    vec![
                                        e_call("+", vec![e_ident(&start_var), e_ident(&len_var)]),
                                        e_call("+", vec![e_ident(&pn_var), e_int(1)]),
                                    ],
                                ),
                                e_return(e_call("result_bytes.err", vec![e_int(code_kind)])),
                                e_int(0),
                            ));
                            elem_stmts.push(e_if(
                                e_call(">", vec![e_ident(&len_var), e_int(max_bytes)]),
                                e_return(e_call("result_bytes.err", vec![e_int(code_too_long)])),
                                e_int(0),
                            ));

                            if matches!(elem.as_ref(), FieldTy::Number) {
                                if let Some(style) = f.number_style {
                                    let canon_fn = match style {
                                        NumberStyleV1::IntAsciiV1 => {
                                            format!("{}._is_canon_int_ascii_v1", td.module_id)
                                        }
                                        NumberStyleV1::UIntAsciiV1 => {
                                            format!("{}._is_canon_uint_ascii_v1", td.module_id)
                                        }
                                    };
                                    elem_stmts.push(e_let(
                                        "num_view",
                                        e_call(
                                            "view.slice",
                                            vec![
                                                e_ident(&f.name),
                                                e_ident(&start_var),
                                                e_ident(&len_var),
                                            ],
                                        ),
                                    ));
                                    elem_stmts.push(e_let(
                                        "is_canon",
                                        e_call(
                                            &canon_fn,
                                            vec![e_ident("num_view"), e_int(max_bytes)],
                                        ),
                                    ));
                                    elem_stmts.push(e_if(
                                        e_call("=", vec![e_ident("is_canon"), e_int(0)]),
                                        e_return(e_call(
                                            "result_bytes.err",
                                            vec![e_int(code_noncanonical_number)],
                                        )),
                                        e_int(0),
                                    ));
                                }
                            }
                        }
                        _ => {}
                    }
                    elem_stmts.push(e_int(0));

                    stmts.push(e_for(
                        &idx_var,
                        e_int(0),
                        e_ident(&seq_len_var),
                        e_begin(elem_stmts),
                    ));
                }
            }
            stmts.push(e_let(&key_var, e_bytes_lit(&f.name)));
            stmts.push(e_let(
                &val_var,
                e_call("view.to_bytes", vec![e_ident(&f.name)]),
            ));
            stmts.push(e_set(
                "entries",
                e_call(
                    &format!("{}._add_entry_v1", td.module_id),
                    vec![e_ident("entries"), e_ident(&key_var), e_ident(&val_var)],
                ),
            ));
            stmts.push(e_set(
                "count",
                e_call("+", vec![e_ident("count"), e_int(1)]),
            ));
        }
        (false, FieldTy::Bool) => {
            let opt = format!("{}_opt", f.name);
            stmts.push(e_let(&key_var, e_bytes_lit(&f.name)));
            stmts.push(e_if(
                e_call("option_i32.is_some", vec![e_ident(&opt)]),
                e_begin(vec![
                    e_let(
                        &f.name,
                        e_call("option_i32.unwrap_or", vec![e_ident(&opt), e_int(0)]),
                    ),
                    e_let(
                        &val_var,
                        e_call(f.ty.value_ctor().expect("scalar"), vec![e_ident(&f.name)]),
                    ),
                    e_set(
                        "entries",
                        e_call(
                            &format!("{}._add_entry_v1", td.module_id),
                            vec![e_ident("entries"), e_ident(&key_var), e_ident(&val_var)],
                        ),
                    ),
                    e_set("count", e_call("+", vec![e_ident("count"), e_int(1)])),
                    e_int(0),
                ]),
                e_int(0),
            ));
        }
        (false, FieldTy::Bytes) | (false, FieldTy::Number) => {
            let opt = format!("{}_opt", f.name);
            let max_bytes = f.max_bytes.unwrap_or(0);
            let b_var = format!("{}_b", f.name);
            stmts.push(e_let(&key_var, e_bytes_lit(&f.name)));
            stmts.push(e_if(
                e_call("option_bytes.is_some", vec![e_ident(&opt)]),
                e_begin(vec![
                    e_let(
                        &b_var,
                        e_call(
                            "option_bytes.unwrap_or",
                            vec![e_ident(&opt), e_ident("empty")],
                        ),
                    ),
                    e_if(
                        e_call(
                            ">",
                            vec![
                                e_call("bytes.len", vec![e_ident(b_var.clone())]),
                                e_int(max_bytes),
                            ],
                        ),
                        e_return(e_call("result_bytes.err", vec![e_int(code_too_long)])),
                        e_int(0),
                    ),
                    if matches!(f.ty, FieldTy::Number) {
                        if let Some(style) = f.number_style {
                            let canon_fn = match style {
                                NumberStyleV1::IntAsciiV1 => {
                                    format!("{}._is_canon_int_ascii_v1", td.module_id)
                                }
                                NumberStyleV1::UIntAsciiV1 => {
                                    format!("{}._is_canon_uint_ascii_v1", td.module_id)
                                }
                            };
                            e_if(
                                e_call(
                                    "=",
                                    vec![
                                        e_call(
                                            &canon_fn,
                                            vec![
                                                e_call("bytes.view", vec![e_ident(b_var.clone())]),
                                                e_int(max_bytes),
                                            ],
                                        ),
                                        e_int(0),
                                    ],
                                ),
                                e_return(e_call(
                                    "result_bytes.err",
                                    vec![e_int(code_noncanonical_number)],
                                )),
                                e_int(0),
                            )
                        } else {
                            e_int(0)
                        }
                    } else {
                        e_int(0)
                    },
                    e_let(
                        &val_var,
                        e_call(
                            f.ty.value_ctor().expect("scalar"),
                            vec![e_call("bytes.view", vec![e_ident(b_var.clone())])],
                        ),
                    ),
                    e_set(
                        "entries",
                        e_call(
                            &format!("{}._add_entry_v1", td.module_id),
                            vec![e_ident("entries"), e_ident(&key_var), e_ident(&val_var)],
                        ),
                    ),
                    e_set("count", e_call("+", vec![e_ident("count"), e_int(1)])),
                    e_int(0),
                ]),
                e_int(0),
            ));
        }
        (false, FieldTy::Struct { .. }) | (false, FieldTy::Seq { .. }) => {
            let opt = format!("{}_opt", f.name);
            let expected_kind = f.ty.kind_byte();
            let max_items = f.max_items.unwrap_or(td.max_seq_items);
            let b_var = format!("{}_b", f.name);
            let view_var = format!("v_{}", f.name);
            let pn_var = format!("pn_{}", f.name);
            let seq_len_var = format!("seq_len_{}", f.name);
            let idx_var = format!("j_{}", f.name);
            let elem_off_var = format!("eo_{}", f.name);

            stmts.push(e_let(&key_var, e_bytes_lit(&f.name)));

            let mut inner: Vec<Expr> = vec![e_let(
                &b_var,
                e_call(
                    "option_bytes.unwrap_or",
                    vec![e_ident(&opt), e_ident("empty")],
                ),
            )];
            inner.push(e_if(
                e_call(
                    "<",
                    vec![e_call("bytes.len", vec![e_ident(&b_var)]), e_int(1)],
                ),
                e_return(e_call("result_bytes.err", vec![e_int(code_kind)])),
                e_int(0),
            ));
            inner.push(e_let(
                &view_var,
                e_call("bytes.view", vec![e_ident(&b_var)]),
            ));
            inner.push(e_let(&pn_var, e_call("view.len", vec![e_ident(&view_var)])));
            inner.push(e_if(
                e_call(
                    "!=",
                    vec![
                        e_call("ext.data_model.kind_at", vec![e_ident(&view_var), e_int(0)]),
                        e_int(expected_kind),
                    ],
                ),
                e_return(e_call("result_bytes.err", vec![e_int(code_kind)])),
                e_int(0),
            ));
            if let FieldTy::Seq { elem } = &f.ty {
                inner.push(e_let(
                    &seq_len_var,
                    e_call("ext.data_model.seq_len", vec![e_ident(&view_var), e_int(0)]),
                ));
                inner.push(e_if(
                    e_call("<", vec![e_ident(&seq_len_var), e_int(0)]),
                    e_return(e_call("result_bytes.err", vec![e_int(code_kind)])),
                    e_int(0),
                ));
                inner.push(e_if(
                    e_call(">", vec![e_ident(&seq_len_var), e_int(max_items)]),
                    e_return(e_call("result_bytes.err", vec![e_int(code_too_long)])),
                    e_int(0),
                ));

                if matches!(
                    elem.as_ref(),
                    FieldTy::Bool | FieldTy::Bytes | FieldTy::Number
                ) {
                    let max_bytes = f.max_bytes.unwrap_or(0);

                    let mut elem_stmts: Vec<Expr> = Vec::new();
                    elem_stmts.push(e_let(
                        &elem_off_var,
                        e_call(
                            "ext.data_model.seq_get",
                            vec![e_ident(&view_var), e_int(0), e_ident(&idx_var)],
                        ),
                    ));
                    elem_stmts.push(e_if(
                        e_call("<", vec![e_ident(&elem_off_var), e_int(0)]),
                        e_return(e_call("result_bytes.err", vec![e_int(code_kind)])),
                        e_int(0),
                    ));

                    match elem.as_ref() {
                        FieldTy::Bool => {
                            let b_var = format!("b_{}_elem", f.name);
                            elem_stmts.push(e_if(
                                e_call(
                                    "!=",
                                    vec![
                                        e_call(
                                            "ext.data_model.kind_at",
                                            vec![e_ident(&view_var), e_ident(&elem_off_var)],
                                        ),
                                        e_int(1),
                                    ],
                                ),
                                e_return(e_call("result_bytes.err", vec![e_int(code_kind)])),
                                e_int(0),
                            ));
                            elem_stmts.push(e_if(
                                e_call(
                                    ">=u",
                                    vec![
                                        e_call("+", vec![e_ident(&elem_off_var), e_int(2)]),
                                        e_call("+", vec![e_ident(&pn_var), e_int(1)]),
                                    ],
                                ),
                                e_return(e_call("result_bytes.err", vec![e_int(code_kind)])),
                                e_int(0),
                            ));
                            elem_stmts.push(e_let(
                                &b_var,
                                e_call(
                                    "view.get_u8",
                                    vec![
                                        e_ident(&view_var),
                                        e_call("+", vec![e_ident(&elem_off_var), e_int(1)]),
                                    ],
                                ),
                            ));
                            elem_stmts.push(e_if(
                                e_if(
                                    e_call("=", vec![e_ident(&b_var), e_int(0)]),
                                    e_int(1),
                                    e_call("=", vec![e_ident(&b_var), e_int(1)]),
                                ),
                                e_int(0),
                                e_return(e_call("result_bytes.err", vec![e_int(code_bool_value)])),
                            ));
                        }
                        FieldTy::Bytes | FieldTy::Number => {
                            let kind = elem.kind_byte();
                            let len_var = format!("len_{}_elem", f.name);
                            let start_var = format!("start_{}_elem", f.name);
                            elem_stmts.push(e_if(
                                e_call(
                                    "!=",
                                    vec![
                                        e_call(
                                            "ext.data_model.kind_at",
                                            vec![e_ident(&view_var), e_ident(&elem_off_var)],
                                        ),
                                        e_int(kind),
                                    ],
                                ),
                                e_return(e_call("result_bytes.err", vec![e_int(code_kind)])),
                                e_int(0),
                            ));
                            elem_stmts.push(e_if(
                                e_call(
                                    ">=u",
                                    vec![
                                        e_call("+", vec![e_ident(&elem_off_var), e_int(5)]),
                                        e_call("+", vec![e_ident(&pn_var), e_int(1)]),
                                    ],
                                ),
                                e_return(e_call("result_bytes.err", vec![e_int(code_kind)])),
                                e_int(0),
                            ));
                            elem_stmts.push(e_let(
                                &len_var,
                                e_call(
                                    "codec.read_u32_le",
                                    vec![
                                        e_ident(&view_var),
                                        e_call("+", vec![e_ident(&elem_off_var), e_int(1)]),
                                    ],
                                ),
                            ));
                            elem_stmts.push(e_if(
                                e_call("<", vec![e_ident(&len_var), e_int(0)]),
                                e_return(e_call("result_bytes.err", vec![e_int(code_kind)])),
                                e_int(0),
                            ));
                            elem_stmts.push(e_let(
                                &start_var,
                                e_call("+", vec![e_ident(&elem_off_var), e_int(5)]),
                            ));
                            elem_stmts.push(e_if(
                                e_call(
                                    ">=u",
                                    vec![
                                        e_call("+", vec![e_ident(&start_var), e_ident(&len_var)]),
                                        e_call("+", vec![e_ident(&pn_var), e_int(1)]),
                                    ],
                                ),
                                e_return(e_call("result_bytes.err", vec![e_int(code_kind)])),
                                e_int(0),
                            ));
                            elem_stmts.push(e_if(
                                e_call(">", vec![e_ident(&len_var), e_int(max_bytes)]),
                                e_return(e_call("result_bytes.err", vec![e_int(code_too_long)])),
                                e_int(0),
                            ));

                            if matches!(elem.as_ref(), FieldTy::Number) {
                                if let Some(style) = f.number_style {
                                    let canon_fn = match style {
                                        NumberStyleV1::IntAsciiV1 => {
                                            format!("{}._is_canon_int_ascii_v1", td.module_id)
                                        }
                                        NumberStyleV1::UIntAsciiV1 => {
                                            format!("{}._is_canon_uint_ascii_v1", td.module_id)
                                        }
                                    };
                                    elem_stmts.push(e_let(
                                        "num_view",
                                        e_call(
                                            "view.slice",
                                            vec![
                                                e_ident(&view_var),
                                                e_ident(&start_var),
                                                e_ident(&len_var),
                                            ],
                                        ),
                                    ));
                                    elem_stmts.push(e_let(
                                        "is_canon",
                                        e_call(
                                            &canon_fn,
                                            vec![e_ident("num_view"), e_int(max_bytes)],
                                        ),
                                    ));
                                    elem_stmts.push(e_if(
                                        e_call("=", vec![e_ident("is_canon"), e_int(0)]),
                                        e_return(e_call(
                                            "result_bytes.err",
                                            vec![e_int(code_noncanonical_number)],
                                        )),
                                        e_int(0),
                                    ));
                                }
                            }
                        }
                        _ => {}
                    }
                    elem_stmts.push(e_int(0));

                    inner.push(e_for(
                        &idx_var,
                        e_int(0),
                        e_ident(&seq_len_var),
                        e_begin(elem_stmts),
                    ));
                }
            }

            inner.push(e_let(&val_var, e_ident(&b_var)));
            inner.push(e_set(
                "entries",
                e_call(
                    &format!("{}._add_entry_v1", td.module_id),
                    vec![e_ident("entries"), e_ident(&key_var), e_ident(&val_var)],
                ),
            ));
            inner.push(e_set(
                "count",
                e_call("+", vec![e_ident("count"), e_int(1)]),
            ));
            inner.push(e_int(0));

            stmts.push(e_if(
                e_call("option_bytes.is_some", vec![e_ident(&opt)]),
                e_begin(inner),
                e_int(0),
            ));
        }
    }

    Ok(stmts)
}

fn gen_field_accessors(td: &TypeDef, f: &FieldDef) -> Result<Vec<FunctionDef>> {
    let mut out: Vec<FunctionDef> = Vec::new();

    let (getter_name, getter_ret, getter_body) = match &f.ty {
        FieldTy::Bool => (format!("get_{}_v1", f.name), Ty::I32, gen_get_bool(td, f)?),
        FieldTy::Bytes | FieldTy::Number => (
            format!("get_{}_view_v1", f.name),
            Ty::BytesView,
            gen_get_view(td, f)?,
        ),
        FieldTy::Struct { .. } | FieldTy::Seq { .. } => (
            format!("get_{}_value_view_v1", f.name),
            Ty::BytesView,
            gen_get_value_view(td, f)?,
        ),
    };
    let full_getter_name = format!("{}.{}", td.module_id, getter_name);
    out.push(FunctionDef {
        name: full_getter_name,
        params: vec![FunctionParam {
            name: "doc".to_string(),
            ty: Ty::BytesView,
        }],
        ret_ty: getter_ret,
        body: getter_body,
    });

    if !f.required {
        let has_name = format!("{}.has_{}_v1", td.module_id, f.name);
        out.push(FunctionDef {
            name: has_name,
            params: vec![FunctionParam {
                name: "doc".to_string(),
                ty: Ty::BytesView,
            }],
            ret_ty: Ty::I32,
            body: gen_has(td, f)?,
        });
    }

    Ok(out)
}

fn gen_get_value_view(_td: &TypeDef, f: &FieldDef) -> Result<Expr> {
    let expected_kind = f.ty.kind_byte();
    Ok(e_begin(vec![
        e_let("root_off", e_int(1)),
        e_let("k", e_bytes_lit(&f.name)),
        e_let(
            "v_off",
            e_call(
                "ext.data_model.map_find",
                vec![
                    e_ident("doc"),
                    e_ident("root_off"),
                    e_call("bytes.view", vec![e_ident("k")]),
                ],
            ),
        ),
        e_if(
            e_call("<", vec![e_ident("v_off"), e_int(0)]),
            e_return(e_call(
                "view.slice",
                vec![e_ident("doc"), e_int(0), e_int(0)],
            )),
            e_int(0),
        ),
        e_if(
            e_call(
                "!=",
                vec![
                    e_call(
                        "ext.data_model.kind_at",
                        vec![e_ident("doc"), e_ident("v_off")],
                    ),
                    e_int(expected_kind),
                ],
            ),
            e_return(e_call(
                "view.slice",
                vec![e_ident("doc"), e_int(0), e_int(0)],
            )),
            e_int(0),
        ),
        e_let(
            "end",
            e_call(
                "ext.data_model.skip_value",
                vec![e_ident("doc"), e_ident("v_off")],
            ),
        ),
        e_if(
            e_call("<", vec![e_ident("end"), e_int(0)]),
            e_return(e_call(
                "view.slice",
                vec![e_ident("doc"), e_int(0), e_int(0)],
            )),
            e_int(0),
        ),
        e_call(
            "view.slice",
            vec![
                e_ident("doc"),
                e_ident("v_off"),
                e_call("-", vec![e_ident("end"), e_ident("v_off")]),
            ],
        ),
    ]))
}

fn gen_get_view(_td: &TypeDef, f: &FieldDef) -> Result<Expr> {
    let expected_kind = f.ty.kind_byte();
    let len_var = format!("len_{}", f.name);
    let start_var = format!("start_{}", f.name);

    Ok(e_begin(vec![
        e_let("root_off", e_int(1)),
        e_let("k", e_bytes_lit(&f.name)),
        e_let(
            "v_off",
            e_call(
                "ext.data_model.map_find",
                vec![
                    e_ident("doc"),
                    e_ident("root_off"),
                    e_call("bytes.view", vec![e_ident("k")]),
                ],
            ),
        ),
        e_if(
            e_call("<", vec![e_ident("v_off"), e_int(0)]),
            e_return(e_call(
                "view.slice",
                vec![e_ident("doc"), e_int(0), e_int(0)],
            )),
            e_int(0),
        ),
        e_if(
            e_call(
                "!=",
                vec![
                    e_call(
                        "ext.data_model.kind_at",
                        vec![e_ident("doc"), e_ident("v_off")],
                    ),
                    e_int(expected_kind),
                ],
            ),
            e_return(e_call(
                "view.slice",
                vec![e_ident("doc"), e_int(0), e_int(0)],
            )),
            e_int(0),
        ),
        e_let("n", e_call("view.len", vec![e_ident("doc")])),
        e_if(
            e_call(
                ">=u",
                vec![
                    e_call("+", vec![e_ident("v_off"), e_int(5)]),
                    e_call("+", vec![e_ident("n"), e_int(1)]),
                ],
            ),
            e_return(e_call(
                "view.slice",
                vec![e_ident("doc"), e_int(0), e_int(0)],
            )),
            e_int(0),
        ),
        e_let(
            &len_var,
            e_call(
                "codec.read_u32_le",
                vec![
                    e_ident("doc"),
                    e_call("+", vec![e_ident("v_off"), e_int(1)]),
                ],
            ),
        ),
        e_if(
            e_call("<", vec![e_ident(&len_var), e_int(0)]),
            e_return(e_call(
                "view.slice",
                vec![e_ident("doc"), e_int(0), e_int(0)],
            )),
            e_int(0),
        ),
        e_let(&start_var, e_call("+", vec![e_ident("v_off"), e_int(5)])),
        e_if(
            e_call(
                ">=u",
                vec![
                    e_call("+", vec![e_ident(&start_var), e_ident(&len_var)]),
                    e_call("+", vec![e_ident("n"), e_int(1)]),
                ],
            ),
            e_return(e_call(
                "view.slice",
                vec![e_ident("doc"), e_int(0), e_int(0)],
            )),
            e_int(0),
        ),
        e_call(
            "view.slice",
            vec![e_ident("doc"), e_ident(&start_var), e_ident(&len_var)],
        ),
    ]))
}

fn gen_get_bool(_td: &TypeDef, f: &FieldDef) -> Result<Expr> {
    let k = format!("k_{}", f.name);
    let b_var = format!("b_{}", f.name);
    Ok(e_begin(vec![
        e_let("root_off", e_int(1)),
        e_let(&k, e_bytes_lit(&f.name)),
        e_let(
            "v_off",
            e_call(
                "ext.data_model.map_find",
                vec![
                    e_ident("doc"),
                    e_ident("root_off"),
                    e_call("bytes.view", vec![e_ident(&k)]),
                ],
            ),
        ),
        e_if(
            e_call("<", vec![e_ident("v_off"), e_int(0)]),
            e_int(0),
            e_begin(vec![
                e_let(
                    &b_var,
                    e_call(
                        "ext.data_model.bool_get",
                        vec![e_ident("doc"), e_ident("v_off")],
                    ),
                ),
                e_if(
                    e_call("<", vec![e_ident(b_var.clone()), e_int(0)]),
                    e_int(0),
                    e_if(
                        e_call("=", vec![e_ident(b_var.clone()), e_int(0)]),
                        e_int(0),
                        e_int(1),
                    ),
                ),
            ]),
        ),
    ]))
}

fn gen_has(_td: &TypeDef, f: &FieldDef) -> Result<Expr> {
    let k = format!("k_{}", f.name);
    Ok(e_begin(vec![
        e_let("root_off", e_int(1)),
        e_let(&k, e_bytes_lit(&f.name)),
        e_let(
            "v_off",
            e_call(
                "ext.data_model.map_find",
                vec![
                    e_ident("doc"),
                    e_ident("root_off"),
                    e_call("bytes.view", vec![e_ident(&k)]),
                ],
            ),
        ),
        e_if(
            e_call("<", vec![e_ident("v_off"), e_int(0)]),
            e_int(0),
            e_int(1),
        ),
    ]))
}

fn gen_enum_tag_view_at(td: &TypeDef) -> Result<FunctionDef> {
    let name = format!("{}._enum_tag_view_at_v1", td.module_id);
    let params = vec![
        FunctionParam {
            name: "value".to_string(),
            ty: Ty::BytesView,
        },
        FunctionParam {
            name: "root_off".to_string(),
            ty: Ty::I32,
        },
    ];

    let empty = e_call("view.slice", vec![e_ident("value"), e_int(0), e_int(0)]);

    let body = e_begin(vec![
        e_if(
            e_call(
                "!=",
                vec![
                    e_call(
                        "ext.data_model.kind_at",
                        vec![e_ident("value"), e_ident("root_off")],
                    ),
                    e_int(4),
                ],
            ),
            e_return(empty.clone()),
            e_int(0),
        ),
        e_let(
            "seq_len",
            e_call(
                "ext.data_model.seq_len",
                vec![e_ident("value"), e_ident("root_off")],
            ),
        ),
        e_if(
            e_call("<", vec![e_ident("seq_len"), e_int(1)]),
            e_return(empty.clone()),
            e_int(0),
        ),
        e_let(
            "v_off",
            e_call(
                "ext.data_model.seq_get",
                vec![e_ident("value"), e_ident("root_off"), e_int(0)],
            ),
        ),
        e_if(
            e_call("<", vec![e_ident("v_off"), e_int(0)]),
            e_return(empty.clone()),
            e_int(0),
        ),
        e_if(
            e_call(
                "!=",
                vec![
                    e_call(
                        "ext.data_model.kind_at",
                        vec![e_ident("value"), e_ident("v_off")],
                    ),
                    e_int(2),
                ],
            ),
            e_return(empty.clone()),
            e_int(0),
        ),
        e_let("n", e_call("view.len", vec![e_ident("value")])),
        e_if(
            e_call(
                ">=u",
                vec![
                    e_call("+", vec![e_ident("v_off"), e_int(5)]),
                    e_call("+", vec![e_ident("n"), e_int(1)]),
                ],
            ),
            e_return(empty.clone()),
            e_int(0),
        ),
        e_let(
            "len_tag",
            e_call(
                "codec.read_u32_le",
                vec![
                    e_ident("value"),
                    e_call("+", vec![e_ident("v_off"), e_int(1)]),
                ],
            ),
        ),
        e_if(
            e_call("<", vec![e_ident("len_tag"), e_int(0)]),
            e_return(empty.clone()),
            e_int(0),
        ),
        e_let("start_tag", e_call("+", vec![e_ident("v_off"), e_int(5)])),
        e_if(
            e_call(
                ">=u",
                vec![
                    e_call("+", vec![e_ident("start_tag"), e_ident("len_tag")]),
                    e_call("+", vec![e_ident("n"), e_int(1)]),
                ],
            ),
            e_return(empty),
            e_int(0),
        ),
        e_call(
            "view.slice",
            vec![e_ident("value"), e_ident("start_tag"), e_ident("len_tag")],
        ),
    ]);

    Ok(FunctionDef {
        name,
        params,
        ret_ty: Ty::BytesView,
        body,
    })
}

fn gen_enum_payload_value_view_at(td: &TypeDef) -> Result<FunctionDef> {
    let name = format!("{}._enum_payload_value_view_at_v1", td.module_id);
    let params = vec![
        FunctionParam {
            name: "value".to_string(),
            ty: Ty::BytesView,
        },
        FunctionParam {
            name: "root_off".to_string(),
            ty: Ty::I32,
        },
    ];

    let empty = e_call("view.slice", vec![e_ident("value"), e_int(0), e_int(0)]);

    let body = e_begin(vec![
        e_if(
            e_call(
                "!=",
                vec![
                    e_call(
                        "ext.data_model.kind_at",
                        vec![e_ident("value"), e_ident("root_off")],
                    ),
                    e_int(4),
                ],
            ),
            e_return(empty.clone()),
            e_int(0),
        ),
        e_let(
            "seq_len",
            e_call(
                "ext.data_model.seq_len",
                vec![e_ident("value"), e_ident("root_off")],
            ),
        ),
        e_if(
            e_call("<", vec![e_ident("seq_len"), e_int(2)]),
            e_return(empty.clone()),
            e_int(0),
        ),
        e_let(
            "v_off",
            e_call(
                "ext.data_model.seq_get",
                vec![e_ident("value"), e_ident("root_off"), e_int(1)],
            ),
        ),
        e_if(
            e_call("<", vec![e_ident("v_off"), e_int(0)]),
            e_return(empty.clone()),
            e_int(0),
        ),
        e_let(
            "end",
            e_call(
                "ext.data_model.skip_value",
                vec![e_ident("value"), e_ident("v_off")],
            ),
        ),
        e_if(
            e_call("<", vec![e_ident("end"), e_int(0)]),
            e_return(empty.clone()),
            e_int(0),
        ),
        e_if(
            e_call("<", vec![e_ident("end"), e_ident("v_off")]),
            e_return(empty),
            e_int(0),
        ),
        e_call(
            "view.slice",
            vec![
                e_ident("value"),
                e_ident("v_off"),
                e_call("-", vec![e_ident("end"), e_ident("v_off")]),
            ],
        ),
    ]);

    Ok(FunctionDef {
        name,
        params,
        ret_ty: Ty::BytesView,
        body,
    })
}

fn gen_enum_get_tag_view(td: &TypeDef) -> Result<FunctionDef> {
    Ok(FunctionDef {
        name: format!("{}.get_tag_view_v1", td.module_id),
        params: vec![FunctionParam {
            name: "doc".to_string(),
            ty: Ty::BytesView,
        }],
        ret_ty: Ty::BytesView,
        body: e_call(
            &format!("{}._enum_tag_view_at_v1", td.module_id),
            vec![e_ident("doc"), e_int(1)],
        ),
    })
}

fn gen_enum_get_payload_value_view(td: &TypeDef) -> Result<FunctionDef> {
    Ok(FunctionDef {
        name: format!("{}.get_payload_value_view_v1", td.module_id),
        params: vec![FunctionParam {
            name: "doc".to_string(),
            ty: Ty::BytesView,
        }],
        ret_ty: Ty::BytesView,
        body: e_call(
            &format!("{}._enum_payload_value_view_at_v1", td.module_id),
            vec![e_ident("doc"), e_int(1)],
        ),
    })
}

fn gen_enum_validate_doc(type_index: &TypeIndex, td: &TypeDef) -> Result<FunctionDef> {
    let name = format!("{}.validate_doc_v1", td.module_id);
    let params = vec![FunctionParam {
        name: "doc".to_string(),
        ty: Ty::BytesView,
    }];

    let code_doc_invalid = td.err_base + 1;
    let code_root_kind = td.err_base + 2;
    let code_doc_too_large = td.err_base + 3;
    let code_enum_tag_invalid = td.err_base + 20;
    let code_enum_payload_invalid = td.err_base + 21;

    let mut stmts: Vec<Expr> = Vec::new();
    stmts.push(e_let("n", e_call("view.len", vec![e_ident("doc")])));
    stmts.push(e_if(
        e_call(">", vec![e_ident("n"), e_int(td.max_doc_bytes)]),
        e_return(e_call("result_i32.err", vec![e_int(code_doc_too_large)])),
        e_int(0),
    ));
    stmts.push(e_if(
        e_call("ext.data_model.doc_is_err", vec![e_ident("doc")]),
        e_return(e_call("result_i32.err", vec![e_int(code_doc_invalid)])),
        e_int(0),
    ));

    stmts.push(e_let("root_off", e_int(1)));
    stmts.push(e_if(
        e_call(
            "!=",
            vec![
                e_call(
                    "ext.data_model.kind_at",
                    vec![e_ident("doc"), e_ident("root_off")],
                ),
                e_int(4),
            ],
        ),
        e_return(e_call("result_i32.err", vec![e_int(code_root_kind)])),
        e_int(0),
    ));
    stmts.push(e_let(
        "seq_len",
        e_call(
            "ext.data_model.seq_len",
            vec![e_ident("doc"), e_ident("root_off")],
        ),
    ));
    stmts.push(e_if(
        e_call("<", vec![e_ident("seq_len"), e_int(0)]),
        e_return(e_call("result_i32.err", vec![e_int(code_root_kind)])),
        e_int(0),
    ));
    stmts.push(e_if(
        e_call("!=", vec![e_ident("seq_len"), e_int(2)]),
        e_return(e_call("result_i32.err", vec![e_int(code_root_kind)])),
        e_int(0),
    ));

    stmts.push(e_let(
        "tag_view",
        e_call(
            &format!("{}._enum_tag_view_at_v1", td.module_id),
            vec![e_ident("doc"), e_ident("root_off")],
        ),
    ));
    stmts.push(e_let(
        "tag_len",
        e_call("view.len", vec![e_ident("tag_view")]),
    ));
    stmts.push(e_if(
        e_call("<=", vec![e_ident("tag_len"), e_int(0)]),
        e_return(e_call("result_i32.err", vec![e_int(code_enum_tag_invalid)])),
        e_int(0),
    ));

    stmts.push(e_let("variant_id", e_int(0)));
    for v in &td.variants {
        let digits = v.id.to_string();
        let tag_lit = format!("tag_lit_{}", v.id);
        stmts.push(e_let(&tag_lit, e_bytes_lit(&digits)));
        let cmp = e_call(
            &format!("{}._cmp_bytes_range_v1", td.module_id),
            vec![
                e_ident("tag_view"),
                e_int(0),
                e_ident("tag_len"),
                e_call("bytes.view", vec![e_ident(&tag_lit)]),
                e_int(0),
                e_call("bytes.len", vec![e_ident(&tag_lit)]),
            ],
        );
        stmts.push(e_if(
            e_call("=", vec![cmp, e_int(0)]),
            e_set("variant_id", e_int(v.id)),
            e_int(0),
        ));
    }
    stmts.push(e_if(
        e_call("=", vec![e_ident("variant_id"), e_int(0)]),
        e_return(e_call("result_i32.err", vec![e_int(code_enum_tag_invalid)])),
        e_int(0),
    ));

    stmts.push(e_let(
        "payload_view",
        e_call(
            &format!("{}._enum_payload_value_view_at_v1", td.module_id),
            vec![e_ident("doc"), e_ident("root_off")],
        ),
    ));
    stmts.push(e_if(
        e_call(
            "=",
            vec![e_call("view.len", vec![e_ident("payload_view")]), e_int(0)],
        ),
        e_return(e_call(
            "result_i32.err",
            vec![e_int(code_enum_payload_invalid)],
        )),
        e_int(0),
    ));

    let mut chain: Expr = e_return(e_call("result_i32.err", vec![e_int(code_enum_tag_invalid)]));
    for v in td.variants.iter().rev() {
        let check = e_call("=", vec![e_ident("variant_id"), e_int(v.id)]);
        let mut inner: Vec<Expr> = Vec::new();
        match &v.payload {
            VariantPayloadDef::Unit => {
                inner.push(e_if(
                    e_call(
                        "!=",
                        vec![
                            e_call(
                                "ext.data_model.kind_at",
                                vec![e_ident("payload_view"), e_int(0)],
                            ),
                            e_int(0),
                        ],
                    ),
                    e_return(e_call(
                        "result_i32.err",
                        vec![e_int(code_enum_payload_invalid)],
                    )),
                    e_int(0),
                ));
            }
            VariantPayloadDef::Value {
                ty,
                max_bytes,
                max_items,
            } => {
                let expected_kind = ty.kind_byte();
                inner.push(e_if(
                    e_call(
                        "!=",
                        vec![
                            e_call(
                                "ext.data_model.kind_at",
                                vec![e_ident("payload_view"), e_int(0)],
                            ),
                            e_int(expected_kind),
                        ],
                    ),
                    e_return(e_call(
                        "result_i32.err",
                        vec![e_int(code_enum_payload_invalid)],
                    )),
                    e_int(0),
                ));

                match ty {
                    FieldTy::Bool => {
                        inner.push(e_let(
                            "b",
                            e_call(
                                "ext.data_model.bool_get",
                                vec![e_ident("payload_view"), e_int(0)],
                            ),
                        ));
                        inner.push(e_if(
                            e_if(
                                e_call("<", vec![e_ident("b"), e_int(0)]),
                                e_int(1),
                                e_call(">", vec![e_ident("b"), e_int(1)]),
                            ),
                            e_return(e_call(
                                "result_i32.err",
                                vec![e_int(code_enum_payload_invalid)],
                            )),
                            e_int(0),
                        ));
                    }
                    FieldTy::Bytes => {
                        let maxb = max_bytes.unwrap_or(0);
                        inner.push(e_let(
                            "pn",
                            e_call("view.len", vec![e_ident("payload_view")]),
                        ));
                        inner.push(e_if(
                            e_call("<", vec![e_ident("pn"), e_int(5)]),
                            e_return(e_call(
                                "result_i32.err",
                                vec![e_int(code_enum_payload_invalid)],
                            )),
                            e_int(0),
                        ));
                        inner.push(e_let(
                            "plen",
                            e_call("codec.read_u32_le", vec![e_ident("payload_view"), e_int(1)]),
                        ));
                        inner.push(e_if(
                            e_call("<", vec![e_ident("plen"), e_int(0)]),
                            e_return(e_call(
                                "result_i32.err",
                                vec![e_int(code_enum_payload_invalid)],
                            )),
                            e_int(0),
                        ));
                        inner.push(e_if(
                            e_call(">", vec![e_ident("plen"), e_int(maxb)]),
                            e_return(e_call(
                                "result_i32.err",
                                vec![e_int(code_enum_payload_invalid)],
                            )),
                            e_int(0),
                        ));
                        inner.push(e_if(
                            e_call(
                                ">=u",
                                vec![
                                    e_call("+", vec![e_int(5), e_ident("plen")]),
                                    e_call("+", vec![e_ident("pn"), e_int(1)]),
                                ],
                            ),
                            e_return(e_call(
                                "result_i32.err",
                                vec![e_int(code_enum_payload_invalid)],
                            )),
                            e_int(0),
                        ));
                    }
                    FieldTy::Number => {
                        let maxb = max_bytes.unwrap_or(0);
                        inner.push(e_let(
                            "pn",
                            e_call("view.len", vec![e_ident("payload_view")]),
                        ));
                        inner.push(e_if(
                            e_call("<", vec![e_ident("pn"), e_int(5)]),
                            e_return(e_call(
                                "result_i32.err",
                                vec![e_int(code_enum_payload_invalid)],
                            )),
                            e_int(0),
                        ));
                        inner.push(e_let(
                            "plen",
                            e_call("codec.read_u32_le", vec![e_ident("payload_view"), e_int(1)]),
                        ));
                        inner.push(e_if(
                            e_call("<", vec![e_ident("plen"), e_int(0)]),
                            e_return(e_call(
                                "result_i32.err",
                                vec![e_int(code_enum_payload_invalid)],
                            )),
                            e_int(0),
                        ));
                        inner.push(e_if(
                            e_call(">", vec![e_ident("plen"), e_int(maxb)]),
                            e_return(e_call(
                                "result_i32.err",
                                vec![e_int(code_enum_payload_invalid)],
                            )),
                            e_int(0),
                        ));
                        inner.push(e_if(
                            e_call(
                                ">=u",
                                vec![
                                    e_call("+", vec![e_int(5), e_ident("plen")]),
                                    e_call("+", vec![e_ident("pn"), e_int(1)]),
                                ],
                            ),
                            e_return(e_call(
                                "result_i32.err",
                                vec![e_int(code_enum_payload_invalid)],
                            )),
                            e_int(0),
                        ));
                        if td.schema_version == SchemaVersion::SpecRows020 {
                            let Some(style) = td.number_style_default_v1 else {
                                anyhow::bail!("internal error: specrows@0.2.0 requires number_style_default_v1");
                            };
                            let canon_fn = match style {
                                NumberStyleV1::IntAsciiV1 => {
                                    format!("{}._is_canon_int_ascii_v1", td.module_id)
                                }
                                NumberStyleV1::UIntAsciiV1 => {
                                    format!("{}._is_canon_uint_ascii_v1", td.module_id)
                                }
                            };
                            inner.push(e_let(
                                "num_view",
                                e_call(
                                    "view.slice",
                                    vec![e_ident("payload_view"), e_int(5), e_ident("plen")],
                                ),
                            ));
                            inner.push(e_let(
                                "is_canon",
                                e_call(&canon_fn, vec![e_ident("num_view"), e_int(maxb)]),
                            ));
                            inner.push(e_if(
                                e_call("=", vec![e_ident("is_canon"), e_int(0)]),
                                e_return(e_call(
                                    "result_i32.err",
                                    vec![e_int(code_enum_payload_invalid)],
                                )),
                                e_int(0),
                            ));
                        }
                    }
                    FieldTy::Seq { elem } => {
                        let max_items = max_items.unwrap_or(td.max_seq_items);
                        let maxb = max_bytes.unwrap_or(0);

                        inner.push(e_let(
                            "pn",
                            e_call("view.len", vec![e_ident("payload_view")]),
                        ));
                        inner.push(e_let(
                            "slen",
                            e_call(
                                "ext.data_model.seq_len",
                                vec![e_ident("payload_view"), e_int(0)],
                            ),
                        ));
                        inner.push(e_if(
                            e_call("<", vec![e_ident("slen"), e_int(0)]),
                            e_return(e_call(
                                "result_i32.err",
                                vec![e_int(code_enum_payload_invalid)],
                            )),
                            e_int(0),
                        ));
                        inner.push(e_if(
                            e_call(">", vec![e_ident("slen"), e_int(max_items)]),
                            e_return(e_call(
                                "result_i32.err",
                                vec![e_int(code_enum_payload_invalid)],
                            )),
                            e_int(0),
                        ));

                        let mut elem_stmts: Vec<Expr> = Vec::new();
                        elem_stmts.push(e_let(
                            "eo",
                            e_call(
                                "ext.data_model.seq_get",
                                vec![e_ident("payload_view"), e_int(0), e_ident("j")],
                            ),
                        ));
                        elem_stmts.push(e_if(
                            e_call("<", vec![e_ident("eo"), e_int(0)]),
                            e_return(e_call(
                                "result_i32.err",
                                vec![e_int(code_enum_payload_invalid)],
                            )),
                            e_int(0),
                        ));

                        match elem.as_ref() {
                            FieldTy::Bool => {
                                elem_stmts.push(e_if(
                                    e_call(
                                        "!=",
                                        vec![
                                            e_call(
                                                "ext.data_model.kind_at",
                                                vec![e_ident("payload_view"), e_ident("eo")],
                                            ),
                                            e_int(1),
                                        ],
                                    ),
                                    e_return(e_call(
                                        "result_i32.err",
                                        vec![e_int(code_enum_payload_invalid)],
                                    )),
                                    e_int(0),
                                ));
                                elem_stmts.push(e_if(
                                    e_call(
                                        ">=u",
                                        vec![
                                            e_call("+", vec![e_ident("eo"), e_int(2)]),
                                            e_call("+", vec![e_ident("pn"), e_int(1)]),
                                        ],
                                    ),
                                    e_return(e_call(
                                        "result_i32.err",
                                        vec![e_int(code_enum_payload_invalid)],
                                    )),
                                    e_int(0),
                                ));
                                elem_stmts.push(e_let(
                                    "b",
                                    e_call(
                                        "view.get_u8",
                                        vec![
                                            e_ident("payload_view"),
                                            e_call("+", vec![e_ident("eo"), e_int(1)]),
                                        ],
                                    ),
                                ));
                                elem_stmts.push(e_if(
                                    e_if(
                                        e_call("=", vec![e_ident("b"), e_int(0)]),
                                        e_int(1),
                                        e_call("=", vec![e_ident("b"), e_int(1)]),
                                    ),
                                    e_int(0),
                                    e_return(e_call(
                                        "result_i32.err",
                                        vec![e_int(code_enum_payload_invalid)],
                                    )),
                                ));
                            }
                            FieldTy::Bytes | FieldTy::Number => {
                                let kind = elem.kind_byte();
                                elem_stmts.push(e_if(
                                    e_call(
                                        "!=",
                                        vec![
                                            e_call(
                                                "ext.data_model.kind_at",
                                                vec![e_ident("payload_view"), e_ident("eo")],
                                            ),
                                            e_int(kind),
                                        ],
                                    ),
                                    e_return(e_call(
                                        "result_i32.err",
                                        vec![e_int(code_enum_payload_invalid)],
                                    )),
                                    e_int(0),
                                ));
                                elem_stmts.push(e_if(
                                    e_call(
                                        ">=u",
                                        vec![
                                            e_call("+", vec![e_ident("eo"), e_int(5)]),
                                            e_call("+", vec![e_ident("pn"), e_int(1)]),
                                        ],
                                    ),
                                    e_return(e_call(
                                        "result_i32.err",
                                        vec![e_int(code_enum_payload_invalid)],
                                    )),
                                    e_int(0),
                                ));
                                elem_stmts.push(e_let(
                                    "len",
                                    e_call(
                                        "codec.read_u32_le",
                                        vec![
                                            e_ident("payload_view"),
                                            e_call("+", vec![e_ident("eo"), e_int(1)]),
                                        ],
                                    ),
                                ));
                                elem_stmts.push(e_if(
                                    e_call("<", vec![e_ident("len"), e_int(0)]),
                                    e_return(e_call(
                                        "result_i32.err",
                                        vec![e_int(code_enum_payload_invalid)],
                                    )),
                                    e_int(0),
                                ));
                                elem_stmts.push(e_let(
                                    "start",
                                    e_call("+", vec![e_ident("eo"), e_int(5)]),
                                ));
                                elem_stmts.push(e_if(
                                    e_call(
                                        ">=u",
                                        vec![
                                            e_call("+", vec![e_ident("start"), e_ident("len")]),
                                            e_call("+", vec![e_ident("pn"), e_int(1)]),
                                        ],
                                    ),
                                    e_return(e_call(
                                        "result_i32.err",
                                        vec![e_int(code_enum_payload_invalid)],
                                    )),
                                    e_int(0),
                                ));
                                elem_stmts.push(e_if(
                                    e_call(">", vec![e_ident("len"), e_int(maxb)]),
                                    e_return(e_call(
                                        "result_i32.err",
                                        vec![e_int(code_enum_payload_invalid)],
                                    )),
                                    e_int(0),
                                ));

                                if matches!(elem.as_ref(), FieldTy::Number)
                                    && td.schema_version == SchemaVersion::SpecRows020
                                {
                                    let Some(style) = td.number_style_default_v1 else {
                                        anyhow::bail!("internal error: specrows@0.2.0 requires number_style_default_v1");
                                    };
                                    let canon_fn = match style {
                                        NumberStyleV1::IntAsciiV1 => {
                                            format!("{}._is_canon_int_ascii_v1", td.module_id)
                                        }
                                        NumberStyleV1::UIntAsciiV1 => {
                                            format!("{}._is_canon_uint_ascii_v1", td.module_id)
                                        }
                                    };
                                    elem_stmts.push(e_let(
                                        "num_view",
                                        e_call(
                                            "view.slice",
                                            vec![
                                                e_ident("payload_view"),
                                                e_ident("start"),
                                                e_ident("len"),
                                            ],
                                        ),
                                    ));
                                    elem_stmts.push(e_let(
                                        "is_canon",
                                        e_call(&canon_fn, vec![e_ident("num_view"), e_int(maxb)]),
                                    ));
                                    elem_stmts.push(e_if(
                                        e_call("=", vec![e_ident("is_canon"), e_int(0)]),
                                        e_return(e_call(
                                            "result_i32.err",
                                            vec![e_int(code_enum_payload_invalid)],
                                        )),
                                        e_int(0),
                                    ));
                                }
                            }
                            FieldTy::Struct { type_id } => {
                                let Some(dep) = type_index.get(type_id) else {
                                    anyhow::bail!(
                                        "{}: unresolved seq elem type reference: {:?}",
                                        td.module_id,
                                        type_id
                                    );
                                };
                                let validate_fn = format!("{}.validate_value_v1", dep.module_id);
                                elem_stmts.push(e_if(
                                    e_call(
                                        "!=",
                                        vec![
                                            e_call(
                                                "ext.data_model.kind_at",
                                                vec![e_ident("payload_view"), e_ident("eo")],
                                            ),
                                            e_int(5),
                                        ],
                                    ),
                                    e_return(e_call(
                                        "result_i32.err",
                                        vec![e_int(code_enum_payload_invalid)],
                                    )),
                                    e_int(0),
                                ));
                                elem_stmts.push(e_let(
                                    "end",
                                    e_call(
                                        "ext.data_model.skip_value",
                                        vec![e_ident("payload_view"), e_ident("eo")],
                                    ),
                                ));
                                elem_stmts.push(e_if(
                                    e_call("<", vec![e_ident("end"), e_int(0)]),
                                    e_return(e_call(
                                        "result_i32.err",
                                        vec![e_int(code_enum_payload_invalid)],
                                    )),
                                    e_int(0),
                                ));
                                elem_stmts.push(e_let(
                                    "slice",
                                    e_call(
                                        "view.slice",
                                        vec![
                                            e_ident("payload_view"),
                                            e_ident("eo"),
                                            e_call("-", vec![e_ident("end"), e_ident("eo")]),
                                        ],
                                    ),
                                ));
                                elem_stmts.push(e_call(
                                    "try",
                                    vec![e_call(&validate_fn, vec![e_ident("slice")])],
                                ));
                            }
                            FieldTy::Seq { .. } => anyhow::bail!(
                                "{}: unsupported enum payload seq elem ty",
                                td.module_id
                            ),
                        }
                        elem_stmts.push(e_int(0));

                        inner.push(e_for("j", e_int(0), e_ident("slen"), e_begin(elem_stmts)));
                    }
                    FieldTy::Struct { type_id } => {
                        let Some(dep) = type_index.get(type_id) else {
                            anyhow::bail!(
                                "{}: unresolved enum payload type reference: {:?}",
                                td.module_id,
                                type_id
                            );
                        };
                        let validate_fn = format!("{}.validate_value_v1", dep.module_id);
                        inner.push(e_call(
                            "try",
                            vec![e_call(&validate_fn, vec![e_ident("payload_view")])],
                        ));
                    }
                }
            }
        }
        inner.push(e_return(e_call("result_i32.ok", vec![e_int(0)])));
        chain = e_if(check, e_begin(inner), chain);
    }
    stmts.push(chain);

    Ok(FunctionDef {
        name,
        params,
        ret_ty: Ty::ResultI32,
        body: e_begin(stmts),
    })
}

fn gen_enum_validate_value(type_index: &TypeIndex, td: &TypeDef) -> Result<FunctionDef> {
    let name = format!("{}.validate_value_v1", td.module_id);
    let params = vec![FunctionParam {
        name: "value".to_string(),
        ty: Ty::BytesView,
    }];

    let code_root_kind = td.err_base + 2;
    let code_doc_too_large = td.err_base + 3;
    let code_enum_tag_invalid = td.err_base + 20;
    let code_enum_payload_invalid = td.err_base + 21;

    let mut stmts: Vec<Expr> = Vec::new();
    stmts.push(e_let("n", e_call("view.len", vec![e_ident("value")])));
    stmts.push(e_if(
        e_call(
            ">u",
            vec![
                e_call("+", vec![e_ident("n"), e_int(1)]),
                e_int(td.max_doc_bytes),
            ],
        ),
        e_return(e_call("result_i32.err", vec![e_int(code_doc_too_large)])),
        e_int(0),
    ));

    stmts.push(e_let("root_off", e_int(0)));
    stmts.push(e_if(
        e_call(
            "!=",
            vec![
                e_call(
                    "ext.data_model.kind_at",
                    vec![e_ident("value"), e_ident("root_off")],
                ),
                e_int(4),
            ],
        ),
        e_return(e_call("result_i32.err", vec![e_int(code_root_kind)])),
        e_int(0),
    ));
    stmts.push(e_let(
        "seq_len",
        e_call(
            "ext.data_model.seq_len",
            vec![e_ident("value"), e_ident("root_off")],
        ),
    ));
    stmts.push(e_if(
        e_call("<", vec![e_ident("seq_len"), e_int(0)]),
        e_return(e_call("result_i32.err", vec![e_int(code_root_kind)])),
        e_int(0),
    ));
    stmts.push(e_if(
        e_call("!=", vec![e_ident("seq_len"), e_int(2)]),
        e_return(e_call("result_i32.err", vec![e_int(code_root_kind)])),
        e_int(0),
    ));

    stmts.push(e_let(
        "tag_view",
        e_call(
            &format!("{}._enum_tag_view_at_v1", td.module_id),
            vec![e_ident("value"), e_ident("root_off")],
        ),
    ));
    stmts.push(e_let(
        "tag_len",
        e_call("view.len", vec![e_ident("tag_view")]),
    ));
    stmts.push(e_if(
        e_call("<=", vec![e_ident("tag_len"), e_int(0)]),
        e_return(e_call("result_i32.err", vec![e_int(code_enum_tag_invalid)])),
        e_int(0),
    ));

    stmts.push(e_let("variant_id", e_int(0)));
    for v in &td.variants {
        let digits = v.id.to_string();
        let tag_lit = format!("tag_lit_{}", v.id);
        stmts.push(e_let(&tag_lit, e_bytes_lit(&digits)));
        let cmp = e_call(
            &format!("{}._cmp_bytes_range_v1", td.module_id),
            vec![
                e_ident("tag_view"),
                e_int(0),
                e_ident("tag_len"),
                e_call("bytes.view", vec![e_ident(&tag_lit)]),
                e_int(0),
                e_call("bytes.len", vec![e_ident(&tag_lit)]),
            ],
        );
        stmts.push(e_if(
            e_call("=", vec![cmp, e_int(0)]),
            e_set("variant_id", e_int(v.id)),
            e_int(0),
        ));
    }
    stmts.push(e_if(
        e_call("=", vec![e_ident("variant_id"), e_int(0)]),
        e_return(e_call("result_i32.err", vec![e_int(code_enum_tag_invalid)])),
        e_int(0),
    ));

    stmts.push(e_let(
        "payload_view",
        e_call(
            &format!("{}._enum_payload_value_view_at_v1", td.module_id),
            vec![e_ident("value"), e_ident("root_off")],
        ),
    ));
    stmts.push(e_if(
        e_call(
            "=",
            vec![e_call("view.len", vec![e_ident("payload_view")]), e_int(0)],
        ),
        e_return(e_call(
            "result_i32.err",
            vec![e_int(code_enum_payload_invalid)],
        )),
        e_int(0),
    ));

    let mut chain: Expr = e_return(e_call("result_i32.err", vec![e_int(code_enum_tag_invalid)]));
    for v in td.variants.iter().rev() {
        let check = e_call("=", vec![e_ident("variant_id"), e_int(v.id)]);
        let mut inner: Vec<Expr> = Vec::new();
        match &v.payload {
            VariantPayloadDef::Unit => {
                inner.push(e_if(
                    e_call(
                        "!=",
                        vec![
                            e_call(
                                "ext.data_model.kind_at",
                                vec![e_ident("payload_view"), e_int(0)],
                            ),
                            e_int(0),
                        ],
                    ),
                    e_return(e_call(
                        "result_i32.err",
                        vec![e_int(code_enum_payload_invalid)],
                    )),
                    e_int(0),
                ));
            }
            VariantPayloadDef::Value {
                ty,
                max_bytes,
                max_items,
            } => {
                let expected_kind = ty.kind_byte();
                inner.push(e_if(
                    e_call(
                        "!=",
                        vec![
                            e_call(
                                "ext.data_model.kind_at",
                                vec![e_ident("payload_view"), e_int(0)],
                            ),
                            e_int(expected_kind),
                        ],
                    ),
                    e_return(e_call(
                        "result_i32.err",
                        vec![e_int(code_enum_payload_invalid)],
                    )),
                    e_int(0),
                ));

                match ty {
                    FieldTy::Bool => {
                        inner.push(e_let(
                            "b",
                            e_call(
                                "ext.data_model.bool_get",
                                vec![e_ident("payload_view"), e_int(0)],
                            ),
                        ));
                        inner.push(e_if(
                            e_if(
                                e_call("<", vec![e_ident("b"), e_int(0)]),
                                e_int(1),
                                e_call(">", vec![e_ident("b"), e_int(1)]),
                            ),
                            e_return(e_call(
                                "result_i32.err",
                                vec![e_int(code_enum_payload_invalid)],
                            )),
                            e_int(0),
                        ));
                    }
                    FieldTy::Bytes => {
                        let maxb = max_bytes.unwrap_or(0);
                        inner.push(e_let(
                            "pn",
                            e_call("view.len", vec![e_ident("payload_view")]),
                        ));
                        inner.push(e_if(
                            e_call("<", vec![e_ident("pn"), e_int(5)]),
                            e_return(e_call(
                                "result_i32.err",
                                vec![e_int(code_enum_payload_invalid)],
                            )),
                            e_int(0),
                        ));
                        inner.push(e_let(
                            "plen",
                            e_call("codec.read_u32_le", vec![e_ident("payload_view"), e_int(1)]),
                        ));
                        inner.push(e_if(
                            e_call("<", vec![e_ident("plen"), e_int(0)]),
                            e_return(e_call(
                                "result_i32.err",
                                vec![e_int(code_enum_payload_invalid)],
                            )),
                            e_int(0),
                        ));
                        inner.push(e_if(
                            e_call(">", vec![e_ident("plen"), e_int(maxb)]),
                            e_return(e_call(
                                "result_i32.err",
                                vec![e_int(code_enum_payload_invalid)],
                            )),
                            e_int(0),
                        ));
                        inner.push(e_if(
                            e_call(
                                ">=u",
                                vec![
                                    e_call("+", vec![e_int(5), e_ident("plen")]),
                                    e_call("+", vec![e_ident("pn"), e_int(1)]),
                                ],
                            ),
                            e_return(e_call(
                                "result_i32.err",
                                vec![e_int(code_enum_payload_invalid)],
                            )),
                            e_int(0),
                        ));
                    }
                    FieldTy::Number => {
                        let maxb = max_bytes.unwrap_or(0);
                        inner.push(e_let(
                            "pn",
                            e_call("view.len", vec![e_ident("payload_view")]),
                        ));
                        inner.push(e_if(
                            e_call("<", vec![e_ident("pn"), e_int(5)]),
                            e_return(e_call(
                                "result_i32.err",
                                vec![e_int(code_enum_payload_invalid)],
                            )),
                            e_int(0),
                        ));
                        inner.push(e_let(
                            "plen",
                            e_call("codec.read_u32_le", vec![e_ident("payload_view"), e_int(1)]),
                        ));
                        inner.push(e_if(
                            e_call("<", vec![e_ident("plen"), e_int(0)]),
                            e_return(e_call(
                                "result_i32.err",
                                vec![e_int(code_enum_payload_invalid)],
                            )),
                            e_int(0),
                        ));
                        inner.push(e_if(
                            e_call(">", vec![e_ident("plen"), e_int(maxb)]),
                            e_return(e_call(
                                "result_i32.err",
                                vec![e_int(code_enum_payload_invalid)],
                            )),
                            e_int(0),
                        ));
                        inner.push(e_if(
                            e_call(
                                ">=u",
                                vec![
                                    e_call("+", vec![e_int(5), e_ident("plen")]),
                                    e_call("+", vec![e_ident("pn"), e_int(1)]),
                                ],
                            ),
                            e_return(e_call(
                                "result_i32.err",
                                vec![e_int(code_enum_payload_invalid)],
                            )),
                            e_int(0),
                        ));
                        if td.schema_version == SchemaVersion::SpecRows020 {
                            let Some(style) = td.number_style_default_v1 else {
                                anyhow::bail!("internal error: specrows@0.2.0 requires number_style_default_v1");
                            };
                            let canon_fn = match style {
                                NumberStyleV1::IntAsciiV1 => {
                                    format!("{}._is_canon_int_ascii_v1", td.module_id)
                                }
                                NumberStyleV1::UIntAsciiV1 => {
                                    format!("{}._is_canon_uint_ascii_v1", td.module_id)
                                }
                            };
                            inner.push(e_let(
                                "num_view",
                                e_call(
                                    "view.slice",
                                    vec![e_ident("payload_view"), e_int(5), e_ident("plen")],
                                ),
                            ));
                            inner.push(e_let(
                                "is_canon",
                                e_call(&canon_fn, vec![e_ident("num_view"), e_int(maxb)]),
                            ));
                            inner.push(e_if(
                                e_call("=", vec![e_ident("is_canon"), e_int(0)]),
                                e_return(e_call(
                                    "result_i32.err",
                                    vec![e_int(code_enum_payload_invalid)],
                                )),
                                e_int(0),
                            ));
                        }
                    }
                    FieldTy::Seq { elem } => {
                        let max_items = max_items.unwrap_or(td.max_seq_items);
                        let maxb = max_bytes.unwrap_or(0);

                        inner.push(e_let(
                            "pn",
                            e_call("view.len", vec![e_ident("payload_view")]),
                        ));
                        inner.push(e_let(
                            "slen",
                            e_call(
                                "ext.data_model.seq_len",
                                vec![e_ident("payload_view"), e_int(0)],
                            ),
                        ));
                        inner.push(e_if(
                            e_call("<", vec![e_ident("slen"), e_int(0)]),
                            e_return(e_call(
                                "result_i32.err",
                                vec![e_int(code_enum_payload_invalid)],
                            )),
                            e_int(0),
                        ));
                        inner.push(e_if(
                            e_call(">", vec![e_ident("slen"), e_int(max_items)]),
                            e_return(e_call(
                                "result_i32.err",
                                vec![e_int(code_enum_payload_invalid)],
                            )),
                            e_int(0),
                        ));

                        let mut elem_stmts: Vec<Expr> = Vec::new();
                        elem_stmts.push(e_let(
                            "eo",
                            e_call(
                                "ext.data_model.seq_get",
                                vec![e_ident("payload_view"), e_int(0), e_ident("j")],
                            ),
                        ));
                        elem_stmts.push(e_if(
                            e_call("<", vec![e_ident("eo"), e_int(0)]),
                            e_return(e_call(
                                "result_i32.err",
                                vec![e_int(code_enum_payload_invalid)],
                            )),
                            e_int(0),
                        ));

                        match elem.as_ref() {
                            FieldTy::Bool => {
                                elem_stmts.push(e_if(
                                    e_call(
                                        "!=",
                                        vec![
                                            e_call(
                                                "ext.data_model.kind_at",
                                                vec![e_ident("payload_view"), e_ident("eo")],
                                            ),
                                            e_int(1),
                                        ],
                                    ),
                                    e_return(e_call(
                                        "result_i32.err",
                                        vec![e_int(code_enum_payload_invalid)],
                                    )),
                                    e_int(0),
                                ));
                                elem_stmts.push(e_if(
                                    e_call(
                                        ">=u",
                                        vec![
                                            e_call("+", vec![e_ident("eo"), e_int(2)]),
                                            e_call("+", vec![e_ident("pn"), e_int(1)]),
                                        ],
                                    ),
                                    e_return(e_call(
                                        "result_i32.err",
                                        vec![e_int(code_enum_payload_invalid)],
                                    )),
                                    e_int(0),
                                ));
                                elem_stmts.push(e_let(
                                    "b",
                                    e_call(
                                        "view.get_u8",
                                        vec![
                                            e_ident("payload_view"),
                                            e_call("+", vec![e_ident("eo"), e_int(1)]),
                                        ],
                                    ),
                                ));
                                elem_stmts.push(e_if(
                                    e_if(
                                        e_call("=", vec![e_ident("b"), e_int(0)]),
                                        e_int(1),
                                        e_call("=", vec![e_ident("b"), e_int(1)]),
                                    ),
                                    e_int(0),
                                    e_return(e_call(
                                        "result_i32.err",
                                        vec![e_int(code_enum_payload_invalid)],
                                    )),
                                ));
                            }
                            FieldTy::Bytes | FieldTy::Number => {
                                let kind = elem.kind_byte();
                                elem_stmts.push(e_if(
                                    e_call(
                                        "!=",
                                        vec![
                                            e_call(
                                                "ext.data_model.kind_at",
                                                vec![e_ident("payload_view"), e_ident("eo")],
                                            ),
                                            e_int(kind),
                                        ],
                                    ),
                                    e_return(e_call(
                                        "result_i32.err",
                                        vec![e_int(code_enum_payload_invalid)],
                                    )),
                                    e_int(0),
                                ));
                                elem_stmts.push(e_if(
                                    e_call(
                                        ">=u",
                                        vec![
                                            e_call("+", vec![e_ident("eo"), e_int(5)]),
                                            e_call("+", vec![e_ident("pn"), e_int(1)]),
                                        ],
                                    ),
                                    e_return(e_call(
                                        "result_i32.err",
                                        vec![e_int(code_enum_payload_invalid)],
                                    )),
                                    e_int(0),
                                ));
                                elem_stmts.push(e_let(
                                    "len",
                                    e_call(
                                        "codec.read_u32_le",
                                        vec![
                                            e_ident("payload_view"),
                                            e_call("+", vec![e_ident("eo"), e_int(1)]),
                                        ],
                                    ),
                                ));
                                elem_stmts.push(e_if(
                                    e_call("<", vec![e_ident("len"), e_int(0)]),
                                    e_return(e_call(
                                        "result_i32.err",
                                        vec![e_int(code_enum_payload_invalid)],
                                    )),
                                    e_int(0),
                                ));
                                elem_stmts.push(e_let(
                                    "start",
                                    e_call("+", vec![e_ident("eo"), e_int(5)]),
                                ));
                                elem_stmts.push(e_if(
                                    e_call(
                                        ">=u",
                                        vec![
                                            e_call("+", vec![e_ident("start"), e_ident("len")]),
                                            e_call("+", vec![e_ident("pn"), e_int(1)]),
                                        ],
                                    ),
                                    e_return(e_call(
                                        "result_i32.err",
                                        vec![e_int(code_enum_payload_invalid)],
                                    )),
                                    e_int(0),
                                ));
                                elem_stmts.push(e_if(
                                    e_call(">", vec![e_ident("len"), e_int(maxb)]),
                                    e_return(e_call(
                                        "result_i32.err",
                                        vec![e_int(code_enum_payload_invalid)],
                                    )),
                                    e_int(0),
                                ));

                                if matches!(elem.as_ref(), FieldTy::Number)
                                    && td.schema_version == SchemaVersion::SpecRows020
                                {
                                    let Some(style) = td.number_style_default_v1 else {
                                        anyhow::bail!("internal error: specrows@0.2.0 requires number_style_default_v1");
                                    };
                                    let canon_fn = match style {
                                        NumberStyleV1::IntAsciiV1 => {
                                            format!("{}._is_canon_int_ascii_v1", td.module_id)
                                        }
                                        NumberStyleV1::UIntAsciiV1 => {
                                            format!("{}._is_canon_uint_ascii_v1", td.module_id)
                                        }
                                    };
                                    elem_stmts.push(e_let(
                                        "num_view",
                                        e_call(
                                            "view.slice",
                                            vec![
                                                e_ident("payload_view"),
                                                e_ident("start"),
                                                e_ident("len"),
                                            ],
                                        ),
                                    ));
                                    elem_stmts.push(e_let(
                                        "is_canon",
                                        e_call(&canon_fn, vec![e_ident("num_view"), e_int(maxb)]),
                                    ));
                                    elem_stmts.push(e_if(
                                        e_call("=", vec![e_ident("is_canon"), e_int(0)]),
                                        e_return(e_call(
                                            "result_i32.err",
                                            vec![e_int(code_enum_payload_invalid)],
                                        )),
                                        e_int(0),
                                    ));
                                }
                            }
                            FieldTy::Struct { type_id } => {
                                let Some(dep) = type_index.get(type_id) else {
                                    anyhow::bail!(
                                        "{}: unresolved seq elem type reference: {:?}",
                                        td.module_id,
                                        type_id
                                    );
                                };
                                let validate_fn = format!("{}.validate_value_v1", dep.module_id);
                                elem_stmts.push(e_if(
                                    e_call(
                                        "!=",
                                        vec![
                                            e_call(
                                                "ext.data_model.kind_at",
                                                vec![e_ident("payload_view"), e_ident("eo")],
                                            ),
                                            e_int(5),
                                        ],
                                    ),
                                    e_return(e_call(
                                        "result_i32.err",
                                        vec![e_int(code_enum_payload_invalid)],
                                    )),
                                    e_int(0),
                                ));
                                elem_stmts.push(e_let(
                                    "end",
                                    e_call(
                                        "ext.data_model.skip_value",
                                        vec![e_ident("payload_view"), e_ident("eo")],
                                    ),
                                ));
                                elem_stmts.push(e_if(
                                    e_call("<", vec![e_ident("end"), e_int(0)]),
                                    e_return(e_call(
                                        "result_i32.err",
                                        vec![e_int(code_enum_payload_invalid)],
                                    )),
                                    e_int(0),
                                ));
                                elem_stmts.push(e_let(
                                    "slice",
                                    e_call(
                                        "view.slice",
                                        vec![
                                            e_ident("payload_view"),
                                            e_ident("eo"),
                                            e_call("-", vec![e_ident("end"), e_ident("eo")]),
                                        ],
                                    ),
                                ));
                                elem_stmts.push(e_call(
                                    "try",
                                    vec![e_call(&validate_fn, vec![e_ident("slice")])],
                                ));
                            }
                            FieldTy::Seq { .. } => anyhow::bail!(
                                "{}: unsupported enum payload seq elem ty",
                                td.module_id
                            ),
                        }
                        elem_stmts.push(e_int(0));

                        inner.push(e_for("j", e_int(0), e_ident("slen"), e_begin(elem_stmts)));
                    }
                    FieldTy::Struct { type_id } => {
                        let Some(dep) = type_index.get(type_id) else {
                            anyhow::bail!(
                                "{}: unresolved enum payload type reference: {:?}",
                                td.module_id,
                                type_id
                            );
                        };
                        let validate_fn = format!("{}.validate_value_v1", dep.module_id);
                        inner.push(e_call(
                            "try",
                            vec![e_call(&validate_fn, vec![e_ident("payload_view")])],
                        ));
                    }
                }
            }
        }
        inner.push(e_return(e_call("result_i32.ok", vec![e_int(0)])));
        chain = e_if(check, e_begin(inner), chain);
    }
    stmts.push(chain);

    Ok(FunctionDef {
        name,
        params,
        ret_ty: Ty::ResultI32,
        body: e_begin(stmts),
    })
}

fn gen_enum_encode_doc(td: &TypeDef) -> Result<FunctionDef> {
    let name = format!("{}.encode_doc_v1", td.module_id);
    let params = vec![
        FunctionParam {
            name: "variant_id".to_string(),
            ty: Ty::I32,
        },
        FunctionParam {
            name: "payload".to_string(),
            ty: Ty::BytesView,
        },
    ];

    let mut stmts: Vec<Expr> = Vec::new();
    stmts.push(e_let(
        "value",
        e_call(
            "try",
            vec![e_call(
                &format!("{}.encode_value_v1", td.module_id),
                vec![e_ident("variant_id"), e_ident("payload")],
            )],
        ),
    ));
    stmts.push(e_call(
        "result_bytes.ok",
        vec![e_call(
            "ext.data_model.doc_ok",
            vec![e_call("bytes.view", vec![e_ident("value")])],
        )],
    ));

    Ok(FunctionDef {
        name,
        params,
        ret_ty: Ty::ResultBytes,
        body: e_begin(stmts),
    })
}

fn gen_enum_encode_value(td: &TypeDef) -> Result<FunctionDef> {
    let name = format!("{}.encode_value_v1", td.module_id);
    let params = vec![
        FunctionParam {
            name: "variant_id".to_string(),
            ty: Ty::I32,
        },
        FunctionParam {
            name: "payload".to_string(),
            ty: Ty::BytesView,
        },
    ];

    let code_root_kind = td.err_base + 2;
    let code_doc_too_large = td.err_base + 3;
    let code_enum_tag_invalid = td.err_base + 20;
    let code_enum_payload_invalid = td.err_base + 21;

    let mut chain: Expr = e_return(e_call(
        "result_bytes.err",
        vec![e_int(code_enum_tag_invalid)],
    ));
    for v in td.variants.iter().rev() {
        let check = e_call("=", vec![e_ident("variant_id"), e_int(v.id)]);
        let digits = v.id.to_string();

        let mut inner: Vec<Expr> = Vec::new();
        inner.push(e_let("tag_b", e_bytes_lit(&digits)));
        inner.push(e_let(
            "tag_val",
            e_call(
                "ext.data_model.value_number",
                vec![e_call("bytes.view", vec![e_ident("tag_b")])],
            ),
        ));

        match &v.payload {
            VariantPayloadDef::Unit => {
                inner.push(e_let(
                    "payload_b",
                    e_call("ext.data_model.value_null", vec![]),
                ));
                inner.push(e_let(
                    "payload_view",
                    e_call("bytes.view", vec![e_ident("payload_b")]),
                ));
            }
            VariantPayloadDef::Value {
                ty,
                max_bytes,
                max_items,
            } => {
                let expected_kind = ty.kind_byte();
                inner.push(e_let("pn", e_call("view.len", vec![e_ident("payload")])));
                inner.push(e_if(
                    e_call("<", vec![e_ident("pn"), e_int(1)]),
                    e_return(e_call(
                        "result_bytes.err",
                        vec![e_int(code_enum_payload_invalid)],
                    )),
                    e_int(0),
                ));
                inner.push(e_if(
                    e_call(
                        "!=",
                        vec![
                            e_call("ext.data_model.kind_at", vec![e_ident("payload"), e_int(0)]),
                            e_int(expected_kind),
                        ],
                    ),
                    e_return(e_call(
                        "result_bytes.err",
                        vec![e_int(code_enum_payload_invalid)],
                    )),
                    e_int(0),
                ));

                match ty {
                    FieldTy::Bool => {
                        inner.push(e_if(
                            e_call("<", vec![e_ident("pn"), e_int(2)]),
                            e_return(e_call(
                                "result_bytes.err",
                                vec![e_int(code_enum_payload_invalid)],
                            )),
                            e_int(0),
                        ));
                        inner.push(e_let(
                            "b",
                            e_call(
                                "ext.data_model.bool_get",
                                vec![e_ident("payload"), e_int(0)],
                            ),
                        ));
                        inner.push(e_if(
                            e_if(
                                e_call("<", vec![e_ident("b"), e_int(0)]),
                                e_int(1),
                                e_call(">", vec![e_ident("b"), e_int(1)]),
                            ),
                            e_return(e_call(
                                "result_bytes.err",
                                vec![e_int(code_enum_payload_invalid)],
                            )),
                            e_int(0),
                        ));
                    }
                    FieldTy::Bytes => {
                        let maxb = max_bytes.unwrap_or(0);
                        inner.push(e_if(
                            e_call("<", vec![e_ident("pn"), e_int(5)]),
                            e_return(e_call(
                                "result_bytes.err",
                                vec![e_int(code_enum_payload_invalid)],
                            )),
                            e_int(0),
                        ));
                        inner.push(e_let(
                            "plen",
                            e_call("codec.read_u32_le", vec![e_ident("payload"), e_int(1)]),
                        ));
                        inner.push(e_if(
                            e_call("<", vec![e_ident("plen"), e_int(0)]),
                            e_return(e_call(
                                "result_bytes.err",
                                vec![e_int(code_enum_payload_invalid)],
                            )),
                            e_int(0),
                        ));
                        inner.push(e_if(
                            e_call(">", vec![e_ident("plen"), e_int(maxb)]),
                            e_return(e_call(
                                "result_bytes.err",
                                vec![e_int(code_enum_payload_invalid)],
                            )),
                            e_int(0),
                        ));
                        inner.push(e_if(
                            e_call(
                                ">=u",
                                vec![
                                    e_call("+", vec![e_int(5), e_ident("plen")]),
                                    e_call("+", vec![e_ident("pn"), e_int(1)]),
                                ],
                            ),
                            e_return(e_call(
                                "result_bytes.err",
                                vec![e_int(code_enum_payload_invalid)],
                            )),
                            e_int(0),
                        ));
                    }
                    FieldTy::Number => {
                        let maxb = max_bytes.unwrap_or(0);
                        inner.push(e_if(
                            e_call("<", vec![e_ident("pn"), e_int(5)]),
                            e_return(e_call(
                                "result_bytes.err",
                                vec![e_int(code_enum_payload_invalid)],
                            )),
                            e_int(0),
                        ));
                        inner.push(e_let(
                            "plen",
                            e_call("codec.read_u32_le", vec![e_ident("payload"), e_int(1)]),
                        ));
                        inner.push(e_if(
                            e_call("<", vec![e_ident("plen"), e_int(0)]),
                            e_return(e_call(
                                "result_bytes.err",
                                vec![e_int(code_enum_payload_invalid)],
                            )),
                            e_int(0),
                        ));
                        inner.push(e_if(
                            e_call(">", vec![e_ident("plen"), e_int(maxb)]),
                            e_return(e_call(
                                "result_bytes.err",
                                vec![e_int(code_enum_payload_invalid)],
                            )),
                            e_int(0),
                        ));
                        inner.push(e_if(
                            e_call(
                                ">=u",
                                vec![
                                    e_call("+", vec![e_int(5), e_ident("plen")]),
                                    e_call("+", vec![e_ident("pn"), e_int(1)]),
                                ],
                            ),
                            e_return(e_call(
                                "result_bytes.err",
                                vec![e_int(code_enum_payload_invalid)],
                            )),
                            e_int(0),
                        ));
                        if td.schema_version == SchemaVersion::SpecRows020 {
                            let Some(style) = td.number_style_default_v1 else {
                                anyhow::bail!("internal error: specrows@0.2.0 requires number_style_default_v1");
                            };
                            let canon_fn = match style {
                                NumberStyleV1::IntAsciiV1 => {
                                    format!("{}._is_canon_int_ascii_v1", td.module_id)
                                }
                                NumberStyleV1::UIntAsciiV1 => {
                                    format!("{}._is_canon_uint_ascii_v1", td.module_id)
                                }
                            };
                            inner.push(e_let(
                                "num_view",
                                e_call(
                                    "view.slice",
                                    vec![e_ident("payload"), e_int(5), e_ident("plen")],
                                ),
                            ));
                            inner.push(e_let(
                                "is_canon",
                                e_call(&canon_fn, vec![e_ident("num_view"), e_int(maxb)]),
                            ));
                            inner.push(e_if(
                                e_call("=", vec![e_ident("is_canon"), e_int(0)]),
                                e_return(e_call(
                                    "result_bytes.err",
                                    vec![e_int(code_enum_payload_invalid)],
                                )),
                                e_int(0),
                            ));
                        }
                    }
                    FieldTy::Seq { elem } => {
                        let max_items = max_items.unwrap_or(td.max_seq_items);
                        let maxb = max_bytes.unwrap_or(0);
                        inner.push(e_if(
                            e_call("<", vec![e_ident("pn"), e_int(5)]),
                            e_return(e_call(
                                "result_bytes.err",
                                vec![e_int(code_enum_payload_invalid)],
                            )),
                            e_int(0),
                        ));
                        inner.push(e_let(
                            "slen",
                            e_call("ext.data_model.seq_len", vec![e_ident("payload"), e_int(0)]),
                        ));
                        inner.push(e_if(
                            e_call("<", vec![e_ident("slen"), e_int(0)]),
                            e_return(e_call(
                                "result_bytes.err",
                                vec![e_int(code_enum_payload_invalid)],
                            )),
                            e_int(0),
                        ));
                        inner.push(e_if(
                            e_call(">", vec![e_ident("slen"), e_int(max_items)]),
                            e_return(e_call(
                                "result_bytes.err",
                                vec![e_int(code_enum_payload_invalid)],
                            )),
                            e_int(0),
                        ));

                        if matches!(
                            elem.as_ref(),
                            FieldTy::Bool | FieldTy::Bytes | FieldTy::Number
                        ) {
                            let mut elem_stmts: Vec<Expr> = Vec::new();
                            elem_stmts.push(e_let(
                                "eo",
                                e_call(
                                    "ext.data_model.seq_get",
                                    vec![e_ident("payload"), e_int(0), e_ident("j")],
                                ),
                            ));
                            elem_stmts.push(e_if(
                                e_call("<", vec![e_ident("eo"), e_int(0)]),
                                e_return(e_call(
                                    "result_bytes.err",
                                    vec![e_int(code_enum_payload_invalid)],
                                )),
                                e_int(0),
                            ));

                            match elem.as_ref() {
                                FieldTy::Bool => {
                                    elem_stmts.push(e_if(
                                        e_call(
                                            "!=",
                                            vec![
                                                e_call(
                                                    "ext.data_model.kind_at",
                                                    vec![e_ident("payload"), e_ident("eo")],
                                                ),
                                                e_int(1),
                                            ],
                                        ),
                                        e_return(e_call(
                                            "result_bytes.err",
                                            vec![e_int(code_enum_payload_invalid)],
                                        )),
                                        e_int(0),
                                    ));
                                    elem_stmts.push(e_if(
                                        e_call(
                                            ">=u",
                                            vec![
                                                e_call("+", vec![e_ident("eo"), e_int(2)]),
                                                e_call("+", vec![e_ident("pn"), e_int(1)]),
                                            ],
                                        ),
                                        e_return(e_call(
                                            "result_bytes.err",
                                            vec![e_int(code_enum_payload_invalid)],
                                        )),
                                        e_int(0),
                                    ));
                                    elem_stmts.push(e_let(
                                        "b",
                                        e_call(
                                            "view.get_u8",
                                            vec![
                                                e_ident("payload"),
                                                e_call("+", vec![e_ident("eo"), e_int(1)]),
                                            ],
                                        ),
                                    ));
                                    elem_stmts.push(e_if(
                                        e_if(
                                            e_call("=", vec![e_ident("b"), e_int(0)]),
                                            e_int(1),
                                            e_call("=", vec![e_ident("b"), e_int(1)]),
                                        ),
                                        e_int(0),
                                        e_return(e_call(
                                            "result_bytes.err",
                                            vec![e_int(code_enum_payload_invalid)],
                                        )),
                                    ));
                                }
                                FieldTy::Bytes | FieldTy::Number => {
                                    let kind = elem.kind_byte();
                                    elem_stmts.push(e_if(
                                        e_call(
                                            "!=",
                                            vec![
                                                e_call(
                                                    "ext.data_model.kind_at",
                                                    vec![e_ident("payload"), e_ident("eo")],
                                                ),
                                                e_int(kind),
                                            ],
                                        ),
                                        e_return(e_call(
                                            "result_bytes.err",
                                            vec![e_int(code_enum_payload_invalid)],
                                        )),
                                        e_int(0),
                                    ));
                                    elem_stmts.push(e_if(
                                        e_call(
                                            ">=u",
                                            vec![
                                                e_call("+", vec![e_ident("eo"), e_int(5)]),
                                                e_call("+", vec![e_ident("pn"), e_int(1)]),
                                            ],
                                        ),
                                        e_return(e_call(
                                            "result_bytes.err",
                                            vec![e_int(code_enum_payload_invalid)],
                                        )),
                                        e_int(0),
                                    ));
                                    elem_stmts.push(e_let(
                                        "len",
                                        e_call(
                                            "codec.read_u32_le",
                                            vec![
                                                e_ident("payload"),
                                                e_call("+", vec![e_ident("eo"), e_int(1)]),
                                            ],
                                        ),
                                    ));
                                    elem_stmts.push(e_if(
                                        e_call("<", vec![e_ident("len"), e_int(0)]),
                                        e_return(e_call(
                                            "result_bytes.err",
                                            vec![e_int(code_enum_payload_invalid)],
                                        )),
                                        e_int(0),
                                    ));
                                    elem_stmts.push(e_let(
                                        "start",
                                        e_call("+", vec![e_ident("eo"), e_int(5)]),
                                    ));
                                    elem_stmts.push(e_if(
                                        e_call(
                                            ">=u",
                                            vec![
                                                e_call("+", vec![e_ident("start"), e_ident("len")]),
                                                e_call("+", vec![e_ident("pn"), e_int(1)]),
                                            ],
                                        ),
                                        e_return(e_call(
                                            "result_bytes.err",
                                            vec![e_int(code_enum_payload_invalid)],
                                        )),
                                        e_int(0),
                                    ));
                                    elem_stmts.push(e_if(
                                        e_call(">", vec![e_ident("len"), e_int(maxb)]),
                                        e_return(e_call(
                                            "result_bytes.err",
                                            vec![e_int(code_enum_payload_invalid)],
                                        )),
                                        e_int(0),
                                    ));

                                    if matches!(elem.as_ref(), FieldTy::Number)
                                        && td.schema_version == SchemaVersion::SpecRows020
                                    {
                                        let Some(style) = td.number_style_default_v1 else {
                                            anyhow::bail!("internal error: specrows@0.2.0 requires number_style_default_v1");
                                        };
                                        let canon_fn = match style {
                                            NumberStyleV1::IntAsciiV1 => {
                                                format!("{}._is_canon_int_ascii_v1", td.module_id)
                                            }
                                            NumberStyleV1::UIntAsciiV1 => {
                                                format!("{}._is_canon_uint_ascii_v1", td.module_id)
                                            }
                                        };
                                        elem_stmts.push(e_let(
                                            "num_view",
                                            e_call(
                                                "view.slice",
                                                vec![
                                                    e_ident("payload"),
                                                    e_ident("start"),
                                                    e_ident("len"),
                                                ],
                                            ),
                                        ));
                                        elem_stmts.push(e_let(
                                            "is_canon",
                                            e_call(
                                                &canon_fn,
                                                vec![e_ident("num_view"), e_int(maxb)],
                                            ),
                                        ));
                                        elem_stmts.push(e_if(
                                            e_call("=", vec![e_ident("is_canon"), e_int(0)]),
                                            e_return(e_call(
                                                "result_bytes.err",
                                                vec![e_int(code_enum_payload_invalid)],
                                            )),
                                            e_int(0),
                                        ));
                                    }
                                }
                                _ => {}
                            }
                            elem_stmts.push(e_int(0));
                            inner.push(e_for("j", e_int(0), e_ident("slen"), e_begin(elem_stmts)));
                        }
                    }
                    FieldTy::Struct { .. } => {}
                }

                inner.push(e_let("payload_view", e_ident("payload")));
            }
        }

        inner.push(e_let(
            "elems",
            e_call("vec_u8.with_capacity", vec![e_int(0)]),
        ));
        inner.push(e_set(
            "elems",
            e_call(
                "vec_u8.extend_bytes",
                vec![
                    e_ident("elems"),
                    e_call("codec.write_u32_le", vec![e_int(2)]),
                ],
            ),
        ));

        inner.push(e_let(
            "tag_len",
            e_call("bytes.len", vec![e_ident("tag_val")]),
        ));
        inner.push(e_set(
            "elems",
            e_call(
                "vec_u8.extend_bytes",
                vec![
                    e_ident("elems"),
                    e_call("codec.write_u32_le", vec![e_ident("tag_len")]),
                ],
            ),
        ));
        inner.push(e_set(
            "elems",
            e_call(
                "vec_u8.extend_bytes",
                vec![e_ident("elems"), e_ident("tag_val")],
            ),
        ));

        inner.push(e_let(
            "payload_len",
            e_call("view.len", vec![e_ident("payload_view")]),
        ));
        inner.push(e_set(
            "elems",
            e_call(
                "vec_u8.extend_bytes",
                vec![
                    e_ident("elems"),
                    e_call("codec.write_u32_le", vec![e_ident("payload_len")]),
                ],
            ),
        ));
        inner.push(e_set(
            "elems",
            e_call(
                "vec_u8.extend_bytes_range",
                vec![
                    e_ident("elems"),
                    e_ident("payload_view"),
                    e_int(0),
                    e_ident("payload_len"),
                ],
            ),
        ));

        inner.push(e_let(
            "elems_b",
            e_call("vec_u8.into_bytes", vec![e_ident("elems")]),
        ));
        inner.push(e_let(
            "seq_val",
            e_call(
                "ext.data_model.value_seq_from_elems",
                vec![e_call("bytes.view", vec![e_ident("elems_b")])],
            ),
        ));
        inner.push(e_if(
            e_call(
                "=",
                vec![e_call("bytes.len", vec![e_ident("seq_val")]), e_int(0)],
            ),
            e_return(e_call("result_bytes.err", vec![e_int(code_root_kind)])),
            e_int(0),
        ));
        inner.push(e_if(
            e_call(
                ">u",
                vec![
                    e_call(
                        "+",
                        vec![e_call("bytes.len", vec![e_ident("seq_val")]), e_int(1)],
                    ),
                    e_int(td.max_doc_bytes),
                ],
            ),
            e_return(e_call("result_bytes.err", vec![e_int(code_doc_too_large)])),
            e_int(0),
        ));
        inner.push(e_return(e_call(
            "result_bytes.ok",
            vec![e_ident("seq_val")],
        )));

        chain = e_if(check, e_begin(inner), chain);
    }

    Ok(FunctionDef {
        name,
        params,
        ret_ty: Ty::ResultBytes,
        body: e_begin(vec![chain]),
    })
}

fn emit_dm_value_bytes(
    type_index: &TypeIndex,
    ty: &FieldTy,
    ev: &ExampleValue,
    out_var: &str,
    stmts: &mut Vec<Expr>,
    prefix: &str,
) -> Result<()> {
    match (ty, ev) {
        (FieldTy::Bool, ExampleValue::Bool(b)) => {
            stmts.push(e_let(
                out_var,
                e_call(
                    "ext.data_model.value_bool",
                    vec![e_int(if *b { 1 } else { 0 })],
                ),
            ));
            Ok(())
        }
        (FieldTy::Bytes, ExampleValue::Bytes(s)) => {
            let bname = format!("{prefix}_b");
            stmts.push(e_let(&bname, e_bytes_lit(s)));
            stmts.push(e_let(
                out_var,
                e_call(
                    "ext.data_model.value_string",
                    vec![e_call("bytes.view", vec![e_ident(&bname)])],
                ),
            ));
            Ok(())
        }
        (FieldTy::Number, ExampleValue::Bytes(s)) => {
            let bname = format!("{prefix}_b");
            stmts.push(e_let(&bname, e_bytes_lit(s)));
            stmts.push(e_let(
                out_var,
                e_call(
                    "ext.data_model.value_number",
                    vec![e_call("bytes.view", vec![e_ident(&bname)])],
                ),
            ));
            Ok(())
        }
        (FieldTy::Struct { type_id }, ExampleValue::Struct(values)) => {
            emit_struct_value_bytes(type_index, type_id, values, out_var, stmts, prefix)
        }
        (FieldTy::Seq { elem }, ExampleValue::Seq(items)) => {
            emit_seq_value_bytes(type_index, elem, items, out_var, stmts, prefix)
        }
        _ => anyhow::bail!("example type mismatch"),
    }
}

fn emit_struct_value_bytes(
    type_index: &TypeIndex,
    type_id: &str,
    values: &BTreeMap<String, ExampleValue>,
    out_var: &str,
    stmts: &mut Vec<Expr>,
    prefix: &str,
) -> Result<()> {
    let Some(info) = type_index.get(type_id) else {
        anyhow::bail!("unknown type_id: {type_id:?}");
    };

    let mut args: Vec<Expr> = Vec::new();
    for f in &info.fields {
        let ev = values.get(&f.name);
        match (f.required, &f.ty, ev) {
            (true, FieldTy::Bool, Some(ExampleValue::Bool(b))) => {
                args.push(e_int(if *b { 1 } else { 0 }));
            }
            (false, FieldTy::Bool, Some(ExampleValue::Bool(b))) => {
                args.push(e_call(
                    "option_i32.some",
                    vec![e_int(if *b { 1 } else { 0 })],
                ));
            }
            (false, FieldTy::Bool, None) => {
                args.push(e_call("option_i32.none", vec![]));
            }

            (true, FieldTy::Bytes | FieldTy::Number, Some(ExampleValue::Bytes(s))) => {
                let bname = format!("{prefix}_{}_b", f.name);
                stmts.push(e_let(&bname, e_bytes_lit(s)));
                args.push(e_call("bytes.view", vec![e_ident(&bname)]));
            }
            (false, FieldTy::Bytes | FieldTy::Number, Some(ExampleValue::Bytes(s))) => {
                let bname = format!("{prefix}_{}_b", f.name);
                stmts.push(e_let(&bname, e_bytes_lit(s)));
                args.push(e_call("option_bytes.some", vec![e_ident(&bname)]));
            }
            (false, FieldTy::Bytes | FieldTy::Number, None) => {
                args.push(e_call("option_bytes.none", vec![]));
            }

            (true, FieldTy::Struct { .. } | FieldTy::Seq { .. }, Some(inner)) => {
                let vname = format!("{prefix}_{}_v", f.name);
                emit_dm_value_bytes(type_index, &f.ty, inner, &vname, stmts, &vname)?;
                args.push(e_call("bytes.view", vec![e_ident(&vname)]));
            }
            (false, FieldTy::Struct { .. } | FieldTy::Seq { .. }, Some(inner)) => {
                let vname = format!("{prefix}_{}_v", f.name);
                emit_dm_value_bytes(type_index, &f.ty, inner, &vname, stmts, &vname)?;
                args.push(e_call("option_bytes.some", vec![e_ident(&vname)]));
            }
            (false, FieldTy::Struct { .. } | FieldTy::Seq { .. }, None) => {
                args.push(e_call("option_bytes.none", vec![]));
            }
            (true, _, None) => anyhow::bail!("missing required field {:?}", f.name),
            _ => anyhow::bail!("type mismatch in field {:?}", f.name),
        }
    }

    stmts.push(e_let(
        out_var,
        e_call(
            "try",
            vec![e_call(&format!("{}.encode_value_v1", info.module_id), args)],
        ),
    ));
    Ok(())
}

fn emit_seq_value_bytes(
    type_index: &TypeIndex,
    elem: &FieldTy,
    items: &[ExampleValue],
    out_var: &str,
    stmts: &mut Vec<Expr>,
    prefix: &str,
) -> Result<()> {
    let elems_var = format!("{prefix}_elems");
    stmts.push(e_let(
        &elems_var,
        e_call("vec_u8.with_capacity", vec![e_int(0)]),
    ));
    stmts.push(e_set(
        &elems_var,
        e_call(
            "vec_u8.extend_bytes",
            vec![
                e_ident(&elems_var),
                e_call(
                    "codec.write_u32_le",
                    vec![e_int(i32::try_from(items.len()).unwrap_or(i32::MAX))],
                ),
            ],
        ),
    ));

    for (idx, ev) in items.iter().enumerate() {
        let vname = format!("{prefix}_e{idx}_v");
        emit_dm_value_bytes(type_index, elem, ev, &vname, stmts, &vname)?;

        let len_var = format!("{prefix}_e{idx}_len");
        stmts.push(e_let(&len_var, e_call("bytes.len", vec![e_ident(&vname)])));
        stmts.push(e_set(
            &elems_var,
            e_call(
                "vec_u8.extend_bytes",
                vec![
                    e_ident(&elems_var),
                    e_call("codec.write_u32_le", vec![e_ident(&len_var)]),
                ],
            ),
        ));
        stmts.push(e_set(
            &elems_var,
            e_call(
                "vec_u8.extend_bytes",
                vec![e_ident(&elems_var), e_ident(&vname)],
            ),
        ));
    }

    let elems_b = format!("{prefix}_elems_b");
    stmts.push(e_let(
        &elems_b,
        e_call("vec_u8.into_bytes", vec![e_ident(&elems_var)]),
    ));
    stmts.push(e_let(
        out_var,
        e_call(
            "ext.data_model.value_seq_from_elems",
            vec![e_call("bytes.view", vec![e_ident(&elems_b)])],
        ),
    ));
    Ok(())
}

fn gen_golden_doc(type_index: &TypeIndex, td: &TypeDef, ex: &ExampleDef) -> Result<FunctionDef> {
    let name = format!("{}._golden_{}_doc_v1", td.tests_module_id, ex.name);

    let ExampleKind::Struct { values } = &ex.kind else {
        anyhow::bail!("internal error: struct golden doc for non-struct example");
    };

    let mut stmts: Vec<Expr> = Vec::new();

    for f in &td.fields {
        let Some(ev) = values.get(&f.name) else {
            continue;
        };
        let vname = format!("v_{}", f.name);
        emit_dm_value_bytes(type_index, &f.ty, ev, &vname, &mut stmts, &vname)
            .with_context(|| format!("example {:?} field {:?}", ex.name, f.name))?;
    }

    let present_fields: Vec<&FieldDef> = td
        .fields
        .iter()
        .filter(|f| values.contains_key(&f.name))
        .collect();
    let mut present_sorted = present_fields.clone();
    present_sorted.sort_by(|a, b| a.name.as_bytes().cmp(b.name.as_bytes()));

    stmts.push(e_let("m", e_call("vec_u8.with_capacity", vec![e_int(0)])));
    stmts.push(e_set(
        "m",
        e_call("vec_u8.push", vec![e_ident("m"), e_int(5)]),
    ));
    stmts.push(e_set(
        "m",
        e_call(
            "vec_u8.extend_bytes",
            vec![
                e_ident("m"),
                e_call(
                    "codec.write_u32_le",
                    vec![e_int(
                        i32::try_from(present_sorted.len()).unwrap_or(i32::MAX),
                    )],
                ),
            ],
        ),
    ));

    for f in present_sorted {
        let key_len = i32::try_from(f.name.len()).unwrap_or(i32::MAX);
        stmts.push(e_set(
            "m",
            e_call(
                "vec_u8.extend_bytes",
                vec![
                    e_ident("m"),
                    e_call("codec.write_u32_le", vec![e_int(key_len)]),
                ],
            ),
        ));
        stmts.push(e_set(
            "m",
            e_call(
                "vec_u8.extend_bytes",
                vec![e_ident("m"), e_bytes_lit(&f.name)],
            ),
        ));
        stmts.push(e_set(
            "m",
            e_call(
                "vec_u8.extend_bytes",
                vec![e_ident("m"), e_ident(format!("v_{}", f.name))],
            ),
        ));
    }

    stmts.push(e_let(
        "map_b",
        e_call("vec_u8.into_bytes", vec![e_ident("m")]),
    ));
    stmts.push(e_call(
        "result_bytes.ok",
        vec![e_call(
            "ext.data_model.doc_ok",
            vec![e_call("bytes.view", vec![e_ident("map_b")])],
        )],
    ));

    Ok(FunctionDef {
        name,
        params: Vec::new(),
        ret_ty: Ty::ResultBytes,
        body: e_begin(stmts),
    })
}

fn gen_golden_doc_enum(
    type_index: &TypeIndex,
    td: &TypeDef,
    ex: &ExampleDef,
) -> Result<FunctionDef> {
    let name = format!("{}._golden_{}_doc_v1", td.tests_module_id, ex.name);

    let ExampleKind::Enum { variant, payload } = &ex.kind else {
        anyhow::bail!("internal error: enum golden doc for non-enum example");
    };
    let v = td
        .variants
        .iter()
        .find(|v| v.name == *variant)
        .ok_or_else(|| {
            anyhow::anyhow!("example {:?} unknown enum variant {:?}", ex.name, variant)
        })?;

    let mut stmts: Vec<Expr> = Vec::new();

    let tag_digits = v.id.to_string();
    stmts.push(e_let("tag_b", e_bytes_lit(&tag_digits)));
    stmts.push(e_let(
        "tag_val",
        e_call(
            "ext.data_model.value_number",
            vec![e_call("bytes.view", vec![e_ident("tag_b")])],
        ),
    ));

    match (&v.payload, payload) {
        (VariantPayloadDef::Unit, None) => {
            stmts.push(e_let(
                "payload_val",
                e_call("ext.data_model.value_null", vec![]),
            ));
        }
        (VariantPayloadDef::Unit, Some(_)) => {
            anyhow::bail!(
                "example {:?} enum variant {:?} does not take a payload",
                ex.name,
                variant
            )
        }
        (VariantPayloadDef::Value { ty, .. }, Some(ev)) => {
            emit_dm_value_bytes(
                type_index,
                ty,
                ev,
                "payload_val",
                &mut stmts,
                &format!("enum_payload_{}", ex.name),
            )
            .with_context(|| format!("example {:?} enum payload", ex.name))?;
        }
        (VariantPayloadDef::Value { .. }, None) => {
            anyhow::bail!("example {:?} missing enum payload", ex.name)
        }
    }

    stmts.push(e_let(
        "elems",
        e_call("vec_u8.with_capacity", vec![e_int(0)]),
    ));
    stmts.push(e_set(
        "elems",
        e_call(
            "vec_u8.extend_bytes",
            vec![
                e_ident("elems"),
                e_call("codec.write_u32_le", vec![e_int(2)]),
            ],
        ),
    ));

    stmts.push(e_let(
        "tag_len",
        e_call("bytes.len", vec![e_ident("tag_val")]),
    ));
    stmts.push(e_set(
        "elems",
        e_call(
            "vec_u8.extend_bytes",
            vec![
                e_ident("elems"),
                e_call("codec.write_u32_le", vec![e_ident("tag_len")]),
            ],
        ),
    ));
    stmts.push(e_set(
        "elems",
        e_call(
            "vec_u8.extend_bytes",
            vec![e_ident("elems"), e_ident("tag_val")],
        ),
    ));

    stmts.push(e_let(
        "payload_len",
        e_call("bytes.len", vec![e_ident("payload_val")]),
    ));
    stmts.push(e_set(
        "elems",
        e_call(
            "vec_u8.extend_bytes",
            vec![
                e_ident("elems"),
                e_call("codec.write_u32_le", vec![e_ident("payload_len")]),
            ],
        ),
    ));
    stmts.push(e_set(
        "elems",
        e_call(
            "vec_u8.extend_bytes",
            vec![e_ident("elems"), e_ident("payload_val")],
        ),
    ));

    stmts.push(e_let(
        "elems_b",
        e_call("vec_u8.into_bytes", vec![e_ident("elems")]),
    ));
    stmts.push(e_let(
        "seq_val",
        e_call(
            "ext.data_model.value_seq_from_elems",
            vec![e_call("bytes.view", vec![e_ident("elems_b")])],
        ),
    ));

    stmts.push(e_call(
        "result_bytes.ok",
        vec![e_call(
            "ext.data_model.doc_ok",
            vec![e_call("bytes.view", vec![e_ident("seq_val")])],
        )],
    ));

    Ok(FunctionDef {
        name,
        params: Vec::new(),
        ret_ty: Ty::ResultBytes,
        body: e_begin(stmts),
    })
}

fn gen_test_negative(type_index: &TypeIndex, td: &TypeDef) -> Result<FunctionDef> {
    let name = format!("{}.test_negative_v1", td.tests_module_id);

    let mut stmts: Vec<Expr> = Vec::new();

    // Build one minimal valid argument set (no optional fields).
    for f in &td.fields {
        match (f.required, &f.ty) {
            (true, FieldTy::Bool) => {
                stmts.push(e_let(&format!("i_{}", f.name), e_int(0)));
            }
            (true, FieldTy::Bytes) => {
                stmts.push(e_let(&format!("b_{}", f.name), e_bytes_lit("a")));
            }
            (true, FieldTy::Number) => {
                stmts.push(e_let(&format!("b_{}", f.name), e_bytes_lit("0")));
            }
            (true, FieldTy::Struct { .. }) => {
                let entries_var = format!("b_{}_entries", f.name);
                stmts.push(e_let(
                    &entries_var,
                    e_call("codec.write_u32_le", vec![e_int(0)]),
                ));
                stmts.push(e_let(
                    &format!("b_{}", f.name),
                    e_call(
                        "ext.data_model.value_map_from_entries",
                        vec![e_call("bytes.view", vec![e_ident(&entries_var)])],
                    ),
                ));
            }
            (true, FieldTy::Seq { .. }) => {
                let elems_var = format!("b_{}_elems", f.name);
                stmts.push(e_let(
                    &elems_var,
                    e_call("codec.write_u32_le", vec![e_int(0)]),
                ));
                stmts.push(e_let(
                    &format!("b_{}", f.name),
                    e_call(
                        "ext.data_model.value_seq_from_elems",
                        vec![e_call("bytes.view", vec![e_ident(&elems_var)])],
                    ),
                ));
            }
            _ => {}
        }
    }

    let mut base_args: Vec<Expr> = Vec::new();
    for f in &td.fields {
        match (f.required, &f.ty) {
            (true, FieldTy::Bool) => base_args.push(e_ident(format!("i_{}", f.name))),
            (true, FieldTy::Bytes | FieldTy::Number) => {
                base_args.push(e_call("bytes.view", vec![e_ident(format!("b_{}", f.name))]))
            }
            (true, FieldTy::Struct { .. }) | (true, FieldTy::Seq { .. }) => {
                base_args.push(e_call("bytes.view", vec![e_ident(format!("b_{}", f.name))]))
            }
            (false, FieldTy::Bool) => base_args.push(e_call("option_i32.none", vec![])),
            (false, FieldTy::Bytes | FieldTy::Number) => {
                base_args.push(e_call("option_bytes.none", vec![]))
            }
            (false, FieldTy::Struct { .. }) | (false, FieldTy::Seq { .. }) => {
                base_args.push(e_call("option_bytes.none", vec![]))
            }
        }
    }

    // Corrupt the doc wrapper tag: doc_is_err should trip with code_doc_invalid_v1.
    stmts.push(e_let(
        "doc0",
        e_call(
            "try",
            vec![e_call(
                &format!("{}.encode_doc_v1", td.module_id),
                base_args.clone(),
            )],
        ),
    ));
    stmts.push(e_let(
        "bad0",
        e_call("bytes.set_u8", vec![e_ident("doc0"), e_int(0), e_int(0)]),
    ));
    stmts.push(e_let(
        "r0",
        e_call(
            &format!("{}.validate_doc_v1", td.module_id),
            vec![e_call("bytes.view", vec![e_ident("bad0")])],
        ),
    ));
    stmts.push(e_call(
        "try",
        vec![e_call(
            "std.test.assert_i32_eq",
            vec![
                e_call("result_i32.err_code", vec![e_ident("r0")]),
                e_call(&format!("{}.code_doc_invalid_v1", td.module_id), vec![]),
                e_call("std.test.code_assert_i32_eq", vec![]),
            ],
        )],
    ));

    // Corrupt root kind: validate_value_v1 should trip with code_root_kind_v1.
    stmts.push(e_let(
        "doc1",
        e_call(
            "try",
            vec![e_call(
                &format!("{}.encode_doc_v1", td.module_id),
                base_args.clone(),
            )],
        ),
    ));
    stmts.push(e_let(
        "bad1",
        e_call("bytes.set_u8", vec![e_ident("doc1"), e_int(1), e_int(4)]),
    ));
    stmts.push(e_let(
        "r1",
        e_call(
            &format!("{}.validate_doc_v1", td.module_id),
            vec![e_call("bytes.view", vec![e_ident("bad1")])],
        ),
    ));
    stmts.push(e_call(
        "try",
        vec![e_call(
            "std.test.assert_i32_eq",
            vec![
                e_call("result_i32.err_code", vec![e_ident("r1")]),
                e_call(&format!("{}.code_root_kind_v1", td.module_id), vec![]),
                e_call("std.test.code_assert_i32_eq", vec![]),
            ],
        )],
    ));

    // Noncanonical map ordering / duplicate keys should be rejected deterministically.
    let mut names_sorted: Vec<&str> = td.fields.iter().map(|f| f.name.as_str()).collect();
    names_sorted.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));
    if names_sorted.len() >= 2 {
        let a = names_sorted[0];
        let b = names_sorted[1];
        stmts.push(e_let("v_null", e_call("ext.data_model.value_null", vec![])));
        stmts.push(e_let(
            "m_bad_order",
            e_call("vec_u8.with_capacity", vec![e_int(0)]),
        ));
        stmts.push(e_set(
            "m_bad_order",
            e_call("vec_u8.push", vec![e_ident("m_bad_order"), e_int(5)]),
        ));
        stmts.push(e_set(
            "m_bad_order",
            e_call(
                "vec_u8.extend_bytes",
                vec![
                    e_ident("m_bad_order"),
                    e_call("codec.write_u32_le", vec![e_int(2)]),
                ],
            ),
        ));
        stmts.push(e_set(
            "m_bad_order",
            e_call(
                "vec_u8.extend_bytes",
                vec![
                    e_ident("m_bad_order"),
                    e_call(
                        "codec.write_u32_le",
                        vec![e_int(i32::try_from(b.len()).unwrap_or(i32::MAX))],
                    ),
                ],
            ),
        ));
        stmts.push(e_set(
            "m_bad_order",
            e_call(
                "vec_u8.extend_bytes",
                vec![e_ident("m_bad_order"), e_bytes_lit(b)],
            ),
        ));
        stmts.push(e_set(
            "m_bad_order",
            e_call(
                "vec_u8.extend_bytes",
                vec![e_ident("m_bad_order"), e_ident("v_null")],
            ),
        ));
        stmts.push(e_set(
            "m_bad_order",
            e_call(
                "vec_u8.extend_bytes",
                vec![
                    e_ident("m_bad_order"),
                    e_call(
                        "codec.write_u32_le",
                        vec![e_int(i32::try_from(a.len()).unwrap_or(i32::MAX))],
                    ),
                ],
            ),
        ));
        stmts.push(e_set(
            "m_bad_order",
            e_call(
                "vec_u8.extend_bytes",
                vec![e_ident("m_bad_order"), e_bytes_lit(a)],
            ),
        ));
        stmts.push(e_set(
            "m_bad_order",
            e_call(
                "vec_u8.extend_bytes",
                vec![e_ident("m_bad_order"), e_ident("v_null")],
            ),
        ));
        stmts.push(e_let(
            "m_bad_order_b",
            e_call("vec_u8.into_bytes", vec![e_ident("m_bad_order")]),
        ));
        stmts.push(e_let(
            "doc_bad_order",
            e_call(
                "ext.data_model.doc_ok",
                vec![e_call("bytes.view", vec![e_ident("m_bad_order_b")])],
            ),
        ));
        stmts.push(e_let(
            "r_bad_order",
            e_call(
                &format!("{}.validate_doc_v1", td.module_id),
                vec![e_call("bytes.view", vec![e_ident("doc_bad_order")])],
            ),
        ));
        stmts.push(e_call(
            "try",
            vec![e_call(
                "std.test.assert_i32_eq",
                vec![
                    e_call("result_i32.err_code", vec![e_ident("r_bad_order")]),
                    e_call(
                        &format!("{}.code_noncanonical_map_v1", td.module_id),
                        vec![],
                    ),
                    e_call("std.test.code_assert_i32_eq", vec![]),
                ],
            )],
        ));

        stmts.push(e_let(
            "m_dup_key",
            e_call("vec_u8.with_capacity", vec![e_int(0)]),
        ));
        stmts.push(e_set(
            "m_dup_key",
            e_call("vec_u8.push", vec![e_ident("m_dup_key"), e_int(5)]),
        ));
        stmts.push(e_set(
            "m_dup_key",
            e_call(
                "vec_u8.extend_bytes",
                vec![
                    e_ident("m_dup_key"),
                    e_call("codec.write_u32_le", vec![e_int(2)]),
                ],
            ),
        ));
        let a_len = i32::try_from(a.len()).unwrap_or(i32::MAX);
        for _ in 0..2 {
            stmts.push(e_set(
                "m_dup_key",
                e_call(
                    "vec_u8.extend_bytes",
                    vec![
                        e_ident("m_dup_key"),
                        e_call("codec.write_u32_le", vec![e_int(a_len)]),
                    ],
                ),
            ));
            stmts.push(e_set(
                "m_dup_key",
                e_call(
                    "vec_u8.extend_bytes",
                    vec![e_ident("m_dup_key"), e_bytes_lit(a)],
                ),
            ));
            stmts.push(e_set(
                "m_dup_key",
                e_call(
                    "vec_u8.extend_bytes",
                    vec![e_ident("m_dup_key"), e_ident("v_null")],
                ),
            ));
        }
        stmts.push(e_let(
            "m_dup_key_b",
            e_call("vec_u8.into_bytes", vec![e_ident("m_dup_key")]),
        ));
        stmts.push(e_let(
            "doc_dup_key",
            e_call(
                "ext.data_model.doc_ok",
                vec![e_call("bytes.view", vec![e_ident("m_dup_key_b")])],
            ),
        ));
        stmts.push(e_let(
            "r_dup_key",
            e_call(
                &format!("{}.validate_doc_v1", td.module_id),
                vec![e_call("bytes.view", vec![e_ident("doc_dup_key")])],
            ),
        ));
        stmts.push(e_call(
            "try",
            vec![e_call(
                "std.test.assert_i32_eq",
                vec![
                    e_call("result_i32.err_code", vec![e_ident("r_dup_key")]),
                    e_call(&format!("{}.code_dup_field_v1", td.module_id), vec![]),
                    e_call("std.test.code_assert_i32_eq", vec![]),
                ],
            )],
        ));
    }

    // Noncanonical numbers must be rejected deterministically (specrows@0.2.0).
    if td.schema_version == SchemaVersion::SpecRows020 {
        fn ident_safe(s: &str) -> String {
            s.chars()
                .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
                .collect()
        }

        fn pick_bad_number(style: NumberStyleV1, max_bytes: Option<i32>) -> Option<&'static str> {
            let max = max_bytes.unwrap_or(i32::MAX);
            match style {
                NumberStyleV1::UIntAsciiV1 => {
                    if max >= 5 {
                        Some("00042")
                    } else if max >= 2 {
                        Some("00")
                    } else {
                        None
                    }
                }
                NumberStyleV1::IntAsciiV1 => {
                    if max >= 2 {
                        Some("-0")
                    } else {
                        None
                    }
                }
            }
        }

        for f in &td.fields {
            let Some(style) = f.number_style else {
                continue;
            };
            let Some(bad_num) = pick_bad_number(style, f.max_bytes) else {
                continue;
            };

            let Some((ex_name, ex_values)) = td.examples.iter().find_map(|ex| match &ex.kind {
                ExampleKind::Struct { values } if values.contains_key(&f.name) => {
                    Some((ex.name.as_str(), values))
                }
                _ => None,
            }) else {
                continue;
            };

            let case = format!("badnum_{}_{}", ident_safe(&f.name), ident_safe(ex_name));

            let mut values = ex_values.clone();
            match &f.ty {
                FieldTy::Number => {
                    values.insert(f.name.clone(), ExampleValue::Bytes(bad_num.to_string()));
                }
                FieldTy::Seq { elem } if matches!(elem.as_ref(), FieldTy::Number) => {
                    let Some(ExampleValue::Seq(items)) = values.get_mut(&f.name) else {
                        anyhow::bail!(
                            "internal error: example {:?} field {:?} expected seq",
                            ex_name,
                            f.name
                        );
                    };
                    if items.is_empty() {
                        if f.max_items == Some(0) {
                            continue;
                        }
                        items.push(ExampleValue::Bytes(bad_num.to_string()));
                    } else {
                        items[0] = ExampleValue::Bytes(bad_num.to_string());
                    }
                }
                _ => continue,
            }

            for rf in &td.fields {
                if rf.required && !values.contains_key(&rf.name) {
                    anyhow::bail!(
                        "internal error: example {:?} missing required field {:?}",
                        ex_name,
                        rf.name
                    );
                }
            }

            // Build a doc with one noncanonical number and validate it fails with the field code.
            let mut vvars: BTreeMap<String, String> = BTreeMap::new();
            for ff in &td.fields {
                let Some(ev) = values.get(&ff.name) else {
                    continue;
                };
                let vvar = format!("v_{}_{}", ident_safe(&ff.name), &case);
                emit_dm_value_bytes(type_index, &ff.ty, ev, &vvar, &mut stmts, &vvar)?;
                vvars.insert(ff.name.clone(), vvar);
            }

            let present_fields: Vec<&FieldDef> = td
                .fields
                .iter()
                .filter(|ff| values.contains_key(&ff.name))
                .collect();
            let mut present_sorted = present_fields.clone();
            present_sorted.sort_by(|a, b| a.name.as_bytes().cmp(b.name.as_bytes()));

            let m_var = format!("m_{}", &case);
            stmts.push(e_let(
                &m_var,
                e_call("vec_u8.with_capacity", vec![e_int(0)]),
            ));
            stmts.push(e_set(
                &m_var,
                e_call("vec_u8.push", vec![e_ident(&m_var), e_int(5)]),
            ));
            stmts.push(e_set(
                &m_var,
                e_call(
                    "vec_u8.extend_bytes",
                    vec![
                        e_ident(&m_var),
                        e_call(
                            "codec.write_u32_le",
                            vec![e_int(
                                i32::try_from(present_sorted.len()).unwrap_or(i32::MAX),
                            )],
                        ),
                    ],
                ),
            ));

            for ff in present_sorted {
                let key_len = i32::try_from(ff.name.len()).unwrap_or(i32::MAX);
                stmts.push(e_set(
                    &m_var,
                    e_call(
                        "vec_u8.extend_bytes",
                        vec![
                            e_ident(&m_var),
                            e_call("codec.write_u32_le", vec![e_int(key_len)]),
                        ],
                    ),
                ));
                stmts.push(e_set(
                    &m_var,
                    e_call(
                        "vec_u8.extend_bytes",
                        vec![e_ident(&m_var), e_bytes_lit(&ff.name)],
                    ),
                ));
                let Some(vvar) = vvars.get(&ff.name) else {
                    anyhow::bail!("internal error: missing vvar for field {:?}", ff.name);
                };
                stmts.push(e_set(
                    &m_var,
                    e_call("vec_u8.extend_bytes", vec![e_ident(&m_var), e_ident(vvar)]),
                ));
            }

            let map_b = format!("map_b_{}", &case);
            stmts.push(e_let(
                &map_b,
                e_call("vec_u8.into_bytes", vec![e_ident(&m_var)]),
            ));
            let doc_var = format!("doc_{}", &case);
            stmts.push(e_let(
                &doc_var,
                e_call(
                    "ext.data_model.doc_ok",
                    vec![e_call("bytes.view", vec![e_ident(&map_b)])],
                ),
            ));
            let r_var = format!("r_{}", &case);
            stmts.push(e_let(
                &r_var,
                e_call(
                    &format!("{}.validate_doc_v1", td.module_id),
                    vec![e_call("bytes.view", vec![e_ident(&doc_var)])],
                ),
            ));
            let code_fn = format!("{}.code_noncanonical_number_{}_v1", td.module_id, f.name);
            stmts.push(e_call(
                "try",
                vec![e_call(
                    "std.test.assert_i32_eq",
                    vec![
                        e_call("result_i32.err_code", vec![e_ident(&r_var)]),
                        e_call(&code_fn, vec![]),
                        e_call("std.test.code_assert_i32_eq", vec![]),
                    ],
                )],
            ));

            // Encoder should reject the noncanonical number input with the same code.
            let mut args: Vec<Expr> = Vec::new();
            for ff in &td.fields {
                let ev = values.get(&ff.name);
                match (ff.required, &ff.ty, ev) {
                    (true, FieldTy::Bool, Some(ExampleValue::Bool(b))) => {
                        args.push(e_int(if *b { 1 } else { 0 }));
                    }
                    (false, FieldTy::Bool, Some(ExampleValue::Bool(b))) => {
                        args.push(e_call(
                            "option_i32.some",
                            vec![e_int(if *b { 1 } else { 0 })],
                        ));
                    }
                    (false, FieldTy::Bool, None) => {
                        args.push(e_call("option_i32.none", vec![]));
                    }

                    (true, FieldTy::Bytes | FieldTy::Number, Some(ExampleValue::Bytes(s))) => {
                        let bname = format!("b_{}_{}", ident_safe(&ff.name), &case);
                        stmts.push(e_let(&bname, e_bytes_lit(s)));
                        args.push(e_call("bytes.view", vec![e_ident(&bname)]));
                    }
                    (false, FieldTy::Bytes | FieldTy::Number, Some(ExampleValue::Bytes(s))) => {
                        let bname = format!("b_{}_{}", ident_safe(&ff.name), &case);
                        stmts.push(e_let(&bname, e_bytes_lit(s)));
                        args.push(e_call("option_bytes.some", vec![e_ident(&bname)]));
                    }
                    (false, FieldTy::Bytes | FieldTy::Number, None) => {
                        args.push(e_call("option_bytes.none", vec![]));
                    }

                    (true, FieldTy::Struct { .. } | FieldTy::Seq { .. }, Some(_)) => {
                        let Some(vvar) = vvars.get(&ff.name) else {
                            anyhow::bail!("internal error: missing vvar for field {:?}", ff.name);
                        };
                        args.push(e_call("bytes.view", vec![e_ident(vvar)]));
                    }
                    (false, FieldTy::Struct { .. } | FieldTy::Seq { .. }, Some(_)) => {
                        let Some(vvar) = vvars.get(&ff.name) else {
                            anyhow::bail!("internal error: missing vvar for field {:?}", ff.name);
                        };
                        args.push(e_call("option_bytes.some", vec![e_ident(vvar)]));
                    }
                    (false, FieldTy::Struct { .. } | FieldTy::Seq { .. }, None) => {
                        args.push(e_call("option_bytes.none", vec![]));
                    }

                    (true, _, None) => anyhow::bail!(
                        "internal error: example {:?} missing required field {:?}",
                        ex_name,
                        ff.name
                    ),
                    _ => anyhow::bail!(
                        "internal error: example {:?} field {:?} type mismatch",
                        ex_name,
                        ff.name
                    ),
                }
            }

            let enc_var = format!("enc_{}", &case);
            stmts.push(e_let(
                &enc_var,
                e_call(&format!("{}.encode_doc_v1", td.module_id), args),
            ));
            stmts.push(e_call(
                "try",
                vec![e_call(
                    "std.test.assert_i32_eq",
                    vec![
                        e_call("result_bytes.err_code", vec![e_ident(&enc_var)]),
                        e_call(&code_fn, vec![]),
                        e_call("std.test.code_assert_i32_eq", vec![]),
                    ],
                )],
            ));
        }
    }

    // Encoder should reject an overlong field with the field-specific too_long code.
    if let Some((field_name, max_bytes, is_required)) = td
        .fields
        .iter()
        .filter_map(|f| match &f.ty {
            FieldTy::Bytes | FieldTy::Number => {
                f.max_bytes.map(|mb| (f.name.as_str(), mb, f.required))
            }
            FieldTy::Bool | FieldTy::Struct { .. } | FieldTy::Seq { .. } => None,
        })
        .min_by_key(|(_, max_bytes, _)| *max_bytes)
    {
        stmts.push(e_let(
            "too_long_b",
            e_call(
                "bytes.alloc",
                vec![e_call("+", vec![e_int(max_bytes), e_int(1)])],
            ),
        ));

        let mut args: Vec<Expr> = Vec::new();
        for f in &td.fields {
            match (f.required, &f.ty) {
                (true, FieldTy::Bool) => args.push(e_ident(format!("i_{}", f.name))),
                (true, FieldTy::Bytes | FieldTy::Number) => {
                    if f.name == field_name {
                        args.push(e_call("bytes.view", vec![e_ident("too_long_b")]));
                    } else {
                        args.push(e_call("bytes.view", vec![e_ident(format!("b_{}", f.name))]));
                    }
                }
                (true, FieldTy::Struct { .. }) | (true, FieldTy::Seq { .. }) => {
                    args.push(e_call("bytes.view", vec![e_ident(format!("b_{}", f.name))]));
                }
                (false, FieldTy::Bool) => args.push(e_call("option_i32.none", vec![])),
                (false, FieldTy::Bytes | FieldTy::Number) => {
                    if !is_required && f.name == field_name {
                        args.push(e_call("option_bytes.some", vec![e_ident("too_long_b")]));
                    } else {
                        args.push(e_call("option_bytes.none", vec![]));
                    }
                }
                (false, FieldTy::Struct { .. }) | (false, FieldTy::Seq { .. }) => {
                    args.push(e_call("option_bytes.none", vec![]));
                }
            }
        }

        stmts.push(e_let(
            "enc_res",
            e_call(&format!("{}.encode_doc_v1", td.module_id), args),
        ));
        let code_fn = format!("{}.code_too_long_{field_name}_v1", td.module_id);
        stmts.push(e_call(
            "try",
            vec![e_call(
                "std.test.assert_i32_eq",
                vec![
                    e_call("result_bytes.err_code", vec![e_ident("enc_res")]),
                    e_call(&code_fn, vec![]),
                    e_call("std.test.code_assert_i32_eq", vec![]),
                ],
            )],
        ));
    }

    stmts.push(e_call("std.test.pass", vec![]));

    Ok(FunctionDef {
        name,
        params: Vec::new(),
        ret_ty: Ty::ResultI32,
        body: e_begin(stmts),
    })
}

fn gen_test_negative_enum(td: &TypeDef) -> Result<FunctionDef> {
    let name = format!("{}.test_negative_v1", td.tests_module_id);

    let v0 = td
        .variants
        .first()
        .ok_or_else(|| anyhow::anyhow!("internal error: enum has no variants"))?;

    let mut stmts: Vec<Expr> = Vec::new();

    // Build a minimal valid payload value for variant v0.
    stmts.push(e_let(
        "payload0_b",
        match &v0.payload {
            VariantPayloadDef::Unit => e_call("ext.data_model.value_null", vec![]),
            VariantPayloadDef::Value { ty, .. } => match ty {
                FieldTy::Bool => e_call("ext.data_model.value_bool", vec![e_int(0)]),
                FieldTy::Bytes => e_call(
                    "ext.data_model.value_string",
                    vec![e_call("bytes.view", vec![e_bytes_lit("a")])],
                ),
                FieldTy::Number => e_call(
                    "ext.data_model.value_number",
                    vec![e_call("bytes.view", vec![e_bytes_lit("0")])],
                ),
                FieldTy::Struct { .. } => e_call(
                    "ext.data_model.value_map_from_entries",
                    vec![e_call(
                        "bytes.view",
                        vec![e_call("codec.write_u32_le", vec![e_int(0)])],
                    )],
                ),
                FieldTy::Seq { .. } => e_call(
                    "ext.data_model.value_seq_from_elems",
                    vec![e_call(
                        "bytes.view",
                        vec![e_call("codec.write_u32_le", vec![e_int(0)])],
                    )],
                ),
            },
        },
    ));
    stmts.push(e_let(
        "payload0",
        e_call("bytes.view", vec![e_ident("payload0_b")]),
    ));

    // Corrupt the doc wrapper tag: doc_is_err should trip with code_doc_invalid_v1.
    stmts.push(e_let(
        "doc0",
        e_call(
            "try",
            vec![e_call(
                &format!("{}.encode_doc_v1", td.module_id),
                vec![e_int(v0.id), e_ident("payload0")],
            )],
        ),
    ));
    stmts.push(e_let(
        "bad0",
        e_call("bytes.set_u8", vec![e_ident("doc0"), e_int(0), e_int(0)]),
    ));
    stmts.push(e_let(
        "r0",
        e_call(
            &format!("{}.validate_doc_v1", td.module_id),
            vec![e_call("bytes.view", vec![e_ident("bad0")])],
        ),
    ));
    stmts.push(e_call(
        "try",
        vec![e_call(
            "std.test.assert_i32_eq",
            vec![
                e_call("result_i32.err_code", vec![e_ident("r0")]),
                e_call(&format!("{}.code_doc_invalid_v1", td.module_id), vec![]),
                e_call("std.test.code_assert_i32_eq", vec![]),
            ],
        )],
    ));

    // Corrupt root kind: validate_doc_v1 should trip with code_root_kind_v1.
    stmts.push(e_let(
        "doc1",
        e_call(
            "try",
            vec![e_call(
                &format!("{}.encode_doc_v1", td.module_id),
                vec![e_int(v0.id), e_ident("payload0")],
            )],
        ),
    ));
    stmts.push(e_let(
        "bad1",
        e_call("bytes.set_u8", vec![e_ident("doc1"), e_int(1), e_int(5)]),
    ));
    stmts.push(e_let(
        "r1",
        e_call(
            &format!("{}.validate_doc_v1", td.module_id),
            vec![e_call("bytes.view", vec![e_ident("bad1")])],
        ),
    ));
    stmts.push(e_call(
        "try",
        vec![e_call(
            "std.test.assert_i32_eq",
            vec![
                e_call("result_i32.err_code", vec![e_ident("r1")]),
                e_call(&format!("{}.code_root_kind_v1", td.module_id), vec![]),
                e_call("std.test.code_assert_i32_eq", vec![]),
            ],
        )],
    ));

    // Encoder should reject unknown variant ids.
    stmts.push(e_let(
        "enc_bad_variant",
        e_call(
            &format!("{}.encode_doc_v1", td.module_id),
            vec![e_int(0), e_ident("payload0")],
        ),
    ));
    stmts.push(e_call(
        "try",
        vec![e_call(
            "std.test.assert_i32_eq",
            vec![
                e_call("result_bytes.err_code", vec![e_ident("enc_bad_variant")]),
                e_call(
                    &format!("{}.code_enum_tag_invalid_v1", td.module_id),
                    vec![],
                ),
                e_call("std.test.code_assert_i32_eq", vec![]),
            ],
        )],
    ));

    // Wrong kind payload must be rejected.
    if let Some(vp) = td.variants.iter().find_map(|v| match &v.payload {
        VariantPayloadDef::Unit => None,
        VariantPayloadDef::Value { ty, .. } => Some((v, ty)),
    }) {
        let (v, ty) = vp;
        stmts.push(e_let(
            "wrong_kind_b",
            match ty {
                FieldTy::Bool => e_call(
                    "ext.data_model.value_string",
                    vec![e_call("bytes.view", vec![e_bytes_lit("a")])],
                ),
                FieldTy::Bytes | FieldTy::Number => {
                    e_call("ext.data_model.value_bool", vec![e_int(0)])
                }
                FieldTy::Struct { .. } | FieldTy::Seq { .. } => e_call(
                    "ext.data_model.value_number",
                    vec![e_call("bytes.view", vec![e_bytes_lit("0")])],
                ),
            },
        ));
        stmts.push(e_let(
            "wrong_kind_v",
            e_call("bytes.view", vec![e_ident("wrong_kind_b")]),
        ));
        stmts.push(e_let(
            "enc_wrong_kind",
            e_call(
                &format!("{}.encode_doc_v1", td.module_id),
                vec![e_int(v.id), e_ident("wrong_kind_v")],
            ),
        ));
        stmts.push(e_call(
            "try",
            vec![e_call(
                "std.test.assert_i32_eq",
                vec![
                    e_call("result_bytes.err_code", vec![e_ident("enc_wrong_kind")]),
                    e_call(
                        &format!("{}.code_enum_payload_invalid_v1", td.module_id),
                        vec![],
                    ),
                    e_call("std.test.code_assert_i32_eq", vec![]),
                ],
            )],
        ));
    }

    // Overlong bytes/number payload must be rejected.
    if let Some((v, kind_byte, max_bytes)) = td.variants.iter().find_map(|v| match &v.payload {
        VariantPayloadDef::Unit => None,
        VariantPayloadDef::Value {
            ty: FieldTy::Bytes,
            max_bytes: Some(mb),
            ..
        } => Some((v, 3, *mb)),
        VariantPayloadDef::Value {
            ty: FieldTy::Number,
            max_bytes: Some(mb),
            ..
        } => Some((v, 2, *mb)),
        _ => None,
    }) {
        stmts.push(e_let(
            "too_long_v",
            e_call("vec_u8.with_capacity", vec![e_int(0)]),
        ));
        stmts.push(e_set(
            "too_long_v",
            e_call("vec_u8.push", vec![e_ident("too_long_v"), e_int(kind_byte)]),
        ));
        stmts.push(e_set(
            "too_long_v",
            e_call(
                "vec_u8.extend_bytes",
                vec![
                    e_ident("too_long_v"),
                    e_call(
                        "codec.write_u32_le",
                        vec![e_call("+", vec![e_int(max_bytes), e_int(1)])],
                    ),
                ],
            ),
        ));
        stmts.push(e_let(
            "too_long_b",
            e_call("vec_u8.into_bytes", vec![e_ident("too_long_v")]),
        ));
        stmts.push(e_let(
            "enc_too_long",
            e_call(
                &format!("{}.encode_doc_v1", td.module_id),
                vec![
                    e_int(v.id),
                    e_call("bytes.view", vec![e_ident("too_long_b")]),
                ],
            ),
        ));
        stmts.push(e_call(
            "try",
            vec![e_call(
                "std.test.assert_i32_eq",
                vec![
                    e_call("result_bytes.err_code", vec![e_ident("enc_too_long")]),
                    e_call(
                        &format!("{}.code_enum_payload_invalid_v1", td.module_id),
                        vec![],
                    ),
                    e_call("std.test.code_assert_i32_eq", vec![]),
                ],
            )],
        ));
    }

    stmts.push(e_call("std.test.pass", vec![]));

    Ok(FunctionDef {
        name,
        params: Vec::new(),
        ret_ty: Ty::ResultI32,
        body: e_begin(stmts),
    })
}

fn gen_test_vectors(type_index: &TypeIndex, td: &TypeDef) -> Result<FunctionDef> {
    let name = format!("{}.test_vectors_v1", td.tests_module_id);
    let mut stmts: Vec<Expr> = Vec::new();

    for ex in &td.examples {
        let ex_prefix = ex.name.replace('-', "_");
        let ExampleKind::Struct { values } = &ex.kind else {
            anyhow::bail!("internal error: struct test vectors for non-struct example");
        };

        for f in &td.fields {
            let val = values.get(&f.name);
            match (f.required, &f.ty, val) {
                (true, FieldTy::Bool, Some(ExampleValue::Bool(b))) => {
                    stmts.push(e_let(
                        &format!("i_{}_{}", f.name, ex_prefix),
                        e_int(if *b { 1 } else { 0 }),
                    ));
                }
                (true, FieldTy::Bytes | FieldTy::Number, Some(ExampleValue::Bytes(s))) => {
                    stmts.push(e_let(
                        &format!("b_{}_{}", f.name, ex_prefix),
                        e_bytes_lit(s),
                    ));
                }
                (false, FieldTy::Bool, Some(ExampleValue::Bool(b))) => {
                    stmts.push(e_let(
                        &format!("i_{}_{}", f.name, ex_prefix),
                        e_int(if *b { 1 } else { 0 }),
                    ));
                }
                (false, FieldTy::Bytes | FieldTy::Number, Some(ExampleValue::Bytes(s))) => {
                    stmts.push(e_let(
                        &format!("b_{}_{}", f.name, ex_prefix),
                        e_bytes_lit(s),
                    ));
                }
                (true, FieldTy::Struct { .. } | FieldTy::Seq { .. }, Some(ev)) => {
                    let v_var = format!("v_{}_{}", f.name, ex_prefix);
                    emit_dm_value_bytes(type_index, &f.ty, ev, &v_var, &mut stmts, &v_var)?;
                }
                (false, FieldTy::Struct { .. } | FieldTy::Seq { .. }, Some(ev)) => {
                    let v_var = format!("v_{}_{}", f.name, ex_prefix);
                    emit_dm_value_bytes(type_index, &f.ty, ev, &v_var, &mut stmts, &v_var)?;
                }
                (true, FieldTy::Struct { .. } | FieldTy::Seq { .. }, None) => {
                    anyhow::bail!("example {:?} missing required field {:?}", ex.name, f.name)
                }
                (_, _, None) => {}
                _ => anyhow::bail!("example {:?} field {:?} type mismatch", ex.name, f.name),
            }
        }

        let mut call_args: Vec<Expr> = Vec::new();
        for f in &td.fields {
            let val = values.get(&f.name);
            match (f.required, &f.ty, val) {
                (true, FieldTy::Bool, Some(ExampleValue::Bool(b))) => {
                    call_args.push(e_ident(format!("i_{}_{}", f.name, ex_prefix)));
                    let _ = b;
                }
                (true, FieldTy::Bytes | FieldTy::Number, Some(ExampleValue::Bytes(_))) => {
                    call_args.push(e_call(
                        "bytes.view",
                        vec![e_ident(format!("b_{}_{}", f.name, ex_prefix))],
                    ));
                }
                (true, FieldTy::Struct { .. } | FieldTy::Seq { .. }, Some(_)) => {
                    call_args.push(e_call(
                        "bytes.view",
                        vec![e_ident(format!("v_{}_{}", f.name, ex_prefix))],
                    ));
                }
                (true, FieldTy::Struct { .. } | FieldTy::Seq { .. }, None) => {
                    anyhow::bail!("example {:?} field {:?} encode mismatch", ex.name, f.name)
                }
                (false, FieldTy::Bool, Some(ExampleValue::Bool(_))) => {
                    call_args.push(e_call(
                        "option_i32.some",
                        vec![e_ident(format!("i_{}_{}", f.name, ex_prefix))],
                    ));
                }
                (false, FieldTy::Bool, None) => {
                    call_args.push(e_call("option_i32.none", vec![]));
                }
                (false, FieldTy::Bytes | FieldTy::Number, Some(ExampleValue::Bytes(_))) => {
                    call_args.push(e_call(
                        "option_bytes.some",
                        vec![e_call(
                            "view.to_bytes",
                            vec![e_call(
                                "bytes.view",
                                vec![e_ident(format!("b_{}_{}", f.name, ex_prefix))],
                            )],
                        )],
                    ));
                }
                (false, FieldTy::Bytes | FieldTy::Number, None) => {
                    call_args.push(e_call("option_bytes.none", vec![]));
                }
                (false, FieldTy::Struct { .. } | FieldTy::Seq { .. }, Some(_)) => {
                    call_args.push(e_call(
                        "option_bytes.some",
                        vec![e_call(
                            "view.to_bytes",
                            vec![e_call(
                                "bytes.view",
                                vec![e_ident(format!("v_{}_{}", f.name, ex_prefix))],
                            )],
                        )],
                    ));
                }
                (false, FieldTy::Struct { .. } | FieldTy::Seq { .. }, None) => {
                    call_args.push(e_call("option_bytes.none", vec![]));
                }
                _ => anyhow::bail!("example {:?} field {:?} encode mismatch", ex.name, f.name),
            }
        }

        stmts.push(e_let(
            &format!("doc_{}", ex_prefix),
            e_call(
                "try",
                vec![e_call(
                    &format!("{}.encode_doc_v1", td.module_id),
                    call_args,
                )],
            ),
        ));

        stmts.push(e_let(
            &format!("expected_{}", ex_prefix),
            e_call(
                "try",
                vec![e_call(
                    &format!("{}._golden_{}_doc_v1", td.tests_module_id, ex.name),
                    vec![],
                )],
            ),
        ));
        stmts.push(e_call(
            "try",
            vec![e_call(
                "std.test.assert_view_eq",
                vec![
                    e_call("bytes.view", vec![e_ident(format!("doc_{}", ex_prefix))]),
                    e_call(
                        "bytes.view",
                        vec![e_ident(format!("expected_{}", ex_prefix))],
                    ),
                    e_call("std.test.code_assert_view_eq", vec![]),
                ],
            )],
        ));

        stmts.push(e_let(
            &format!("docv_{}", ex_prefix),
            e_call("bytes.view", vec![e_ident(format!("doc_{}", ex_prefix))]),
        ));
        stmts.push(e_let(
            &format!("vr_{}", ex_prefix),
            e_call(
                &format!("{}.validate_doc_v1", td.module_id),
                vec![e_ident(format!("docv_{}", ex_prefix))],
            ),
        ));
        stmts.push(e_call(
            "try",
            vec![e_call(
                "std.test.assert_true",
                vec![
                    e_call(
                        "result_i32.is_ok",
                        vec![e_ident(format!("vr_{}", ex_prefix))],
                    ),
                    e_call("std.test.code_assert_true", vec![]),
                ],
            )],
        ));

        for f in &td.fields {
            match (&f.ty, values.get(&f.name), f.required) {
                (FieldTy::Bool, Some(ExampleValue::Bool(b)), _) => {
                    let got = format!("got_{}_{}", f.name, ex_prefix);
                    stmts.push(e_let(
                        &got,
                        e_call(
                            &format!("{}.get_{}_v1", td.module_id, f.name),
                            vec![e_ident(format!("docv_{}", ex_prefix))],
                        ),
                    ));
                    stmts.push(e_call(
                        "try",
                        vec![e_call(
                            "std.test.assert_i32_eq",
                            vec![
                                e_ident(&got),
                                e_int(if *b { 1 } else { 0 }),
                                e_call("std.test.code_assert_i32_eq", vec![]),
                            ],
                        )],
                    ));
                    if !f.required {
                        let has = format!("has_{}_{}", f.name, ex_prefix);
                        stmts.push(e_let(
                            &has,
                            e_call(
                                &format!("{}.has_{}_v1", td.module_id, f.name),
                                vec![e_ident(format!("docv_{}", ex_prefix))],
                            ),
                        ));
                        stmts.push(e_call(
                            "try",
                            vec![e_call(
                                "std.test.assert_i32_eq",
                                vec![
                                    e_ident(&has),
                                    e_int(1),
                                    e_call("std.test.code_assert_i32_eq", vec![]),
                                ],
                            )],
                        ));
                    }
                }
                (FieldTy::Bool, None, false) => {
                    let has = format!("has_{}_{}", f.name, ex_prefix);
                    let got = format!("got_{}_{}", f.name, ex_prefix);
                    stmts.push(e_let(
                        &got,
                        e_call(
                            &format!("{}.get_{}_v1", td.module_id, f.name),
                            vec![e_ident(format!("docv_{}", ex_prefix))],
                        ),
                    ));
                    stmts.push(e_call(
                        "try",
                        vec![e_call(
                            "std.test.assert_i32_eq",
                            vec![
                                e_ident(&got),
                                e_int(0),
                                e_call("std.test.code_assert_i32_eq", vec![]),
                            ],
                        )],
                    ));
                    stmts.push(e_let(
                        &has,
                        e_call(
                            &format!("{}.has_{}_v1", td.module_id, f.name),
                            vec![e_ident(format!("docv_{}", ex_prefix))],
                        ),
                    ));
                    stmts.push(e_call(
                        "try",
                        vec![e_call(
                            "std.test.assert_i32_eq",
                            vec![
                                e_ident(&has),
                                e_int(0),
                                e_call("std.test.code_assert_i32_eq", vec![]),
                            ],
                        )],
                    ));
                }
                (FieldTy::Bytes | FieldTy::Number, Some(ExampleValue::Bytes(_)), _) => {
                    let got = format!("got_{}_{}", f.name, ex_prefix);
                    stmts.push(e_let(
                        &got,
                        e_call(
                            &format!("{}.get_{}_view_v1", td.module_id, f.name),
                            vec![e_ident(format!("docv_{}", ex_prefix))],
                        ),
                    ));
                    stmts.push(e_call(
                        "try",
                        vec![e_call(
                            "std.test.assert_view_eq",
                            vec![
                                e_ident(&got),
                                e_call(
                                    "bytes.view",
                                    vec![e_ident(format!("b_{}_{}", f.name, ex_prefix))],
                                ),
                                e_call("std.test.code_assert_view_eq", vec![]),
                            ],
                        )],
                    ));
                    if !f.required {
                        let has = format!("has_{}_{}", f.name, ex_prefix);
                        stmts.push(e_let(
                            &has,
                            e_call(
                                &format!("{}.has_{}_v1", td.module_id, f.name),
                                vec![e_ident(format!("docv_{}", ex_prefix))],
                            ),
                        ));
                        stmts.push(e_call(
                            "try",
                            vec![e_call(
                                "std.test.assert_i32_eq",
                                vec![
                                    e_ident(&has),
                                    e_int(1),
                                    e_call("std.test.code_assert_i32_eq", vec![]),
                                ],
                            )],
                        ));
                    }
                }
                (FieldTy::Bytes | FieldTy::Number, None, false) => {
                    let got = format!("got_{}_{}", f.name, ex_prefix);
                    stmts.push(e_let(
                        &got,
                        e_call(
                            &format!("{}.get_{}_view_v1", td.module_id, f.name),
                            vec![e_ident(format!("docv_{}", ex_prefix))],
                        ),
                    ));
                    stmts.push(e_call(
                        "try",
                        vec![e_call(
                            "std.test.assert_i32_eq",
                            vec![
                                e_call("view.len", vec![e_ident(&got)]),
                                e_int(0),
                                e_call("std.test.code_assert_i32_eq", vec![]),
                            ],
                        )],
                    ));
                    let has = format!("has_{}_{}", f.name, ex_prefix);
                    stmts.push(e_let(
                        &has,
                        e_call(
                            &format!("{}.has_{}_v1", td.module_id, f.name),
                            vec![e_ident(format!("docv_{}", ex_prefix))],
                        ),
                    ));
                    stmts.push(e_call(
                        "try",
                        vec![e_call(
                            "std.test.assert_i32_eq",
                            vec![
                                e_ident(&has),
                                e_int(0),
                                e_call("std.test.code_assert_i32_eq", vec![]),
                            ],
                        )],
                    ));
                }
                (FieldTy::Struct { .. } | FieldTy::Seq { .. }, Some(_), _) => {
                    let got = format!("got_{}_{}", f.name, ex_prefix);
                    stmts.push(e_let(
                        &got,
                        e_call(
                            &format!("{}.get_{}_value_view_v1", td.module_id, f.name),
                            vec![e_ident(format!("docv_{}", ex_prefix))],
                        ),
                    ));
                    stmts.push(e_call(
                        "try",
                        vec![e_call(
                            "std.test.assert_view_eq",
                            vec![
                                e_ident(&got),
                                e_call(
                                    "bytes.view",
                                    vec![e_ident(format!("v_{}_{}", f.name, ex_prefix))],
                                ),
                                e_call("std.test.code_assert_view_eq", vec![]),
                            ],
                        )],
                    ));
                    if !f.required {
                        let has = format!("has_{}_{}", f.name, ex_prefix);
                        stmts.push(e_let(
                            &has,
                            e_call(
                                &format!("{}.has_{}_v1", td.module_id, f.name),
                                vec![e_ident(format!("docv_{}", ex_prefix))],
                            ),
                        ));
                        stmts.push(e_call(
                            "try",
                            vec![e_call(
                                "std.test.assert_i32_eq",
                                vec![
                                    e_ident(&has),
                                    e_int(1),
                                    e_call("std.test.code_assert_i32_eq", vec![]),
                                ],
                            )],
                        ));
                    }
                }
                (FieldTy::Struct { .. } | FieldTy::Seq { .. }, None, false) => {
                    let got = format!("got_{}_{}", f.name, ex_prefix);
                    stmts.push(e_let(
                        &got,
                        e_call(
                            &format!("{}.get_{}_value_view_v1", td.module_id, f.name),
                            vec![e_ident(format!("docv_{}", ex_prefix))],
                        ),
                    ));
                    stmts.push(e_call(
                        "try",
                        vec![e_call(
                            "std.test.assert_i32_eq",
                            vec![
                                e_call("view.len", vec![e_ident(&got)]),
                                e_int(0),
                                e_call("std.test.code_assert_i32_eq", vec![]),
                            ],
                        )],
                    ));
                    let has = format!("has_{}_{}", f.name, ex_prefix);
                    stmts.push(e_let(
                        &has,
                        e_call(
                            &format!("{}.has_{}_v1", td.module_id, f.name),
                            vec![e_ident(format!("docv_{}", ex_prefix))],
                        ),
                    ));
                    stmts.push(e_call(
                        "try",
                        vec![e_call(
                            "std.test.assert_i32_eq",
                            vec![
                                e_ident(&has),
                                e_int(0),
                                e_call("std.test.code_assert_i32_eq", vec![]),
                            ],
                        )],
                    ));
                }
                _ => {}
            }
        }
    }

    stmts.push(e_call(
        "try",
        vec![e_call(
            &format!("{}.test_negative_v1", td.tests_module_id),
            vec![],
        )],
    ));
    stmts.push(e_call("std.test.pass", vec![]));

    Ok(FunctionDef {
        name,
        params: Vec::new(),
        ret_ty: Ty::ResultI32,
        body: e_begin(stmts),
    })
}

fn gen_test_vectors_enum(type_index: &TypeIndex, td: &TypeDef) -> Result<FunctionDef> {
    let name = format!("{}.test_vectors_v1", td.tests_module_id);
    let mut stmts: Vec<Expr> = Vec::new();

    for ex in &td.examples {
        let ex_prefix = ex.name.replace('-', "_");
        let ExampleKind::Enum { variant, payload } = &ex.kind else {
            anyhow::bail!("internal error: enum test vectors for non-enum example");
        };
        let v = td
            .variants
            .iter()
            .find(|v| v.name == *variant)
            .ok_or_else(|| {
                anyhow::anyhow!("example {:?} unknown enum variant {:?}", ex.name, variant)
            })?;

        let tag_digits = v.id.to_string();
        stmts.push(e_let(
            &format!("tag_{}", ex_prefix),
            e_bytes_lit(&tag_digits),
        ));

        match (&v.payload, payload) {
            (VariantPayloadDef::Unit, None) => {
                stmts.push(e_let(
                    &format!("payload_{}", ex_prefix),
                    e_call("ext.data_model.value_null", vec![]),
                ));
            }
            (VariantPayloadDef::Unit, Some(_)) => anyhow::bail!(
                "example {:?} enum variant {:?} does not take a payload",
                ex.name,
                variant
            ),
            (VariantPayloadDef::Value { ty, .. }, Some(ev)) => {
                emit_dm_value_bytes(
                    type_index,
                    ty,
                    ev,
                    &format!("payload_{}", ex_prefix),
                    &mut stmts,
                    &format!("enum_payload_{}_{}", ex.name, ex_prefix),
                )
                .with_context(|| format!("example {:?} enum payload", ex.name))?;
            }
            (VariantPayloadDef::Value { .. }, None) => {
                anyhow::bail!("example {:?} missing enum payload", ex.name)
            }
        }

        stmts.push(e_let(
            &format!("doc_{}", ex_prefix),
            e_call(
                "try",
                vec![e_call(
                    &format!("{}.encode_doc_v1", td.module_id),
                    vec![
                        e_int(v.id),
                        e_call(
                            "bytes.view",
                            vec![e_ident(format!("payload_{}", ex_prefix))],
                        ),
                    ],
                )],
            ),
        ));

        stmts.push(e_let(
            &format!("expected_{}", ex_prefix),
            e_call(
                "try",
                vec![e_call(
                    &format!("{}._golden_{}_doc_v1", td.tests_module_id, ex.name),
                    vec![],
                )],
            ),
        ));
        stmts.push(e_call(
            "try",
            vec![e_call(
                "std.test.assert_view_eq",
                vec![
                    e_call("bytes.view", vec![e_ident(format!("doc_{}", ex_prefix))]),
                    e_call(
                        "bytes.view",
                        vec![e_ident(format!("expected_{}", ex_prefix))],
                    ),
                    e_call("std.test.code_assert_view_eq", vec![]),
                ],
            )],
        ));

        stmts.push(e_let(
            &format!("docv_{}", ex_prefix),
            e_call("bytes.view", vec![e_ident(format!("doc_{}", ex_prefix))]),
        ));
        stmts.push(e_let(
            &format!("vr_{}", ex_prefix),
            e_call(
                &format!("{}.validate_doc_v1", td.module_id),
                vec![e_ident(format!("docv_{}", ex_prefix))],
            ),
        ));
        stmts.push(e_call(
            "try",
            vec![e_call(
                "std.test.assert_true",
                vec![
                    e_call(
                        "result_i32.is_ok",
                        vec![e_ident(format!("vr_{}", ex_prefix))],
                    ),
                    e_call("std.test.code_assert_true", vec![]),
                ],
            )],
        ));

        stmts.push(e_let(
            &format!("got_tag_{}", ex_prefix),
            e_call(
                &format!("{}.get_tag_view_v1", td.module_id),
                vec![e_ident(format!("docv_{}", ex_prefix))],
            ),
        ));
        stmts.push(e_call(
            "try",
            vec![e_call(
                "std.test.assert_view_eq",
                vec![
                    e_ident(format!("got_tag_{}", ex_prefix)),
                    e_call("bytes.view", vec![e_ident(format!("tag_{}", ex_prefix))]),
                    e_call("std.test.code_assert_view_eq", vec![]),
                ],
            )],
        ));

        stmts.push(e_let(
            &format!("got_payload_{}", ex_prefix),
            e_call(
                &format!("{}.get_payload_value_view_v1", td.module_id),
                vec![e_ident(format!("docv_{}", ex_prefix))],
            ),
        ));
        stmts.push(e_call(
            "try",
            vec![e_call(
                "std.test.assert_view_eq",
                vec![
                    e_ident(format!("got_payload_{}", ex_prefix)),
                    e_call(
                        "bytes.view",
                        vec![e_ident(format!("payload_{}", ex_prefix))],
                    ),
                    e_call("std.test.code_assert_view_eq", vec![]),
                ],
            )],
        ));
    }

    stmts.push(e_call("std.test.pass", vec![]));

    Ok(FunctionDef {
        name,
        params: Vec::new(),
        ret_ty: Ty::ResultI32,
        body: e_begin(stmts),
    })
}

fn build_key_match_chain(
    _td: &TypeDef,
    fields: &[FieldDef],
    dup_code: Expr,
    unknown_else: Expr,
) -> Result<Expr> {
    let mut chain = unknown_else;
    for f in fields.iter().rev() {
        let off_var = format!("off_{}", f.name);
        let then = e_begin(vec![
            e_if(
                e_call(">=", vec![e_ident(&off_var), e_int(0)]),
                e_return(e_call("result_i32.err", vec![dup_code.clone()])),
                e_int(0),
            ),
            e_set(&off_var, e_ident("k_end")),
            e_int(0),
        ]);
        chain = e_if(
            e_call(
                "view.eq",
                vec![
                    e_ident("k_view"),
                    e_call("bytes.view", vec![e_ident(format!("k_lit_{}", f.name))]),
                ],
            ),
            then,
            chain,
        );
    }
    Ok(chain)
}

fn e_int(value: i32) -> Expr {
    Expr::Int {
        value,
        ptr: String::new(),
    }
}

fn e_ident(name: impl Into<String>) -> Expr {
    Expr::Ident {
        name: name.into(),
        ptr: String::new(),
    }
}

fn e_list(items: Vec<Expr>) -> Expr {
    Expr::List {
        items,
        ptr: String::new(),
    }
}

fn e_call(name: &str, args: Vec<Expr>) -> Expr {
    let mut items = Vec::with_capacity(1 + args.len());
    items.push(e_ident(name.to_string()));
    items.extend(args);
    e_list(items)
}

fn e_begin(stmts: Vec<Expr>) -> Expr {
    let mut items = Vec::with_capacity(1 + stmts.len());
    items.push(e_ident("begin"));
    items.extend(stmts);
    e_list(items)
}

fn e_let(name: &str, value: Expr) -> Expr {
    e_list(vec![e_ident("let"), e_ident(name), value])
}

fn e_set(name: &str, value: Expr) -> Expr {
    e_list(vec![e_ident("set"), e_ident(name), value])
}

fn e_if(cond: Expr, then: Expr, els: Expr) -> Expr {
    e_list(vec![e_ident("if"), cond, then, els])
}

fn e_for(var: &str, start: Expr, end: Expr, body: Expr) -> Expr {
    e_list(vec![e_ident("for"), e_ident(var), start, end, body])
}

fn e_return(expr: Expr) -> Expr {
    e_list(vec![e_ident("return"), expr])
}

fn e_bytes_lit(s: &str) -> Expr {
    e_call("bytes.lit", vec![e_ident(s)])
}
