use serde_json::Value;

use super::X07CliDiagnostic;

const SEV_ERROR: &str = "error";
const RESERVED_LONG_HELP: &str = "--help";
const RESERVED_SHORT_HELP: &str = "-h";
const RESERVED_LONG_VERSION: &str = "--version";
const RESERVED_SHORT_VERSION: &str = "-V";

fn diag(code: &str, msg: String, scope: &str, row_index: i32) -> X07CliDiagnostic {
    X07CliDiagnostic {
        severity: SEV_ERROR.to_string(),
        code: code.to_string(),
        scope: scope.to_string(),
        row_index,
        message: msg,
    }
}

fn is_empty_str(s: &str) -> bool {
    s.is_empty()
}

#[derive(Default)]
struct SeenOpts {
    short: std::collections::BTreeMap<String, usize>,
    long: std::collections::BTreeMap<String, usize>,
}

fn check_short_long(
    diags: &mut Vec<X07CliDiagnostic>,
    scope: &str,
    kind: &str,
    row_index: usize,
    short_opt: &str,
    long_opt: &str,
    seen: &mut SeenOpts,
) {
    if is_empty_str(short_opt) && is_empty_str(long_opt) {
        diags.push(diag(
            if kind == "flag" {
                "ECLI_FLAG_NO_NAMES"
            } else {
                "ECLI_OPT_NO_NAMES"
            },
            format!("{kind} row must provide at least one of shortOpt or longOpt"),
            scope,
            row_index as i32,
        ));
        return;
    }

    if kind != "help" && kind != "version" {
        if long_opt == RESERVED_LONG_HELP || short_opt == RESERVED_SHORT_HELP {
            diags.push(diag(
                "ECLI_RESERVED_HELP_USED",
                format!("{kind} row uses reserved help option"),
                scope,
                row_index as i32,
            ));
        }
        if long_opt == RESERVED_LONG_VERSION || short_opt == RESERVED_SHORT_VERSION {
            diags.push(diag(
                "ECLI_RESERVED_VERSION_USED",
                format!("{kind} row uses reserved version option"),
                scope,
                row_index as i32,
            ));
        }
    }

    if !short_opt.is_empty() {
        if seen.short.contains_key(short_opt) {
            diags.push(diag(
                "ECLI_DUP_SHORT",
                format!("duplicate short option {short_opt}"),
                scope,
                row_index as i32,
            ));
        } else {
            seen.short.insert(short_opt.to_string(), row_index);
        }
    }

    if !long_opt.is_empty() {
        if seen.long.contains_key(long_opt) {
            diags.push(diag(
                "ECLI_DUP_LONG",
                format!("duplicate long option {long_opt}"),
                scope,
                row_index as i32,
            ));
        } else {
            seen.long.insert(long_opt.to_string(), row_index);
        }
    }
}

fn row_get_str(row: &[Value], idx: usize) -> &str {
    row.get(idx).and_then(Value::as_str).unwrap_or("")
}

fn row_get_meta(row: &[Value], idx: usize) -> Option<&serde_json::Map<String, Value>> {
    row.get(idx).and_then(Value::as_object)
}

fn canon_sort_key_str(s: &str) -> (u8, &str) {
    if s.is_empty() {
        (1, "")
    } else {
        (0, s)
    }
}

pub(crate) struct CanonResult {
    pub(crate) canon: Value,
    pub(crate) diagnostics: Vec<X07CliDiagnostic>,
}

pub(crate) fn validate_and_canon(doc: &Value) -> CanonResult {
    let mut diags: Vec<X07CliDiagnostic> = Vec::new();

    let rows: Vec<Vec<Value>> = doc
        .get("rows")
        .and_then(Value::as_array)
        .unwrap_or(&Vec::new())
        .iter()
        .map(|v| v.as_array().cloned().unwrap_or_default())
        .collect();

    let mut by_scope: std::collections::BTreeMap<String, Vec<(usize, Vec<Value>)>> =
        std::collections::BTreeMap::new();
    for (i, row) in rows.iter().cloned().enumerate() {
        if row.len() < 2 {
            diags.push(diag(
                "ECLI_ROW_SHAPE",
                "row must be an array with at least [scope, kind, ...]".to_string(),
                "",
                i as i32,
            ));
            continue;
        }
        let scope = row_get_str(&row, 0).to_string();
        by_scope.entry(scope).or_default().push((i, row));
    }

    // Deterministic scope iteration (root first)
    let mut scopes: Vec<String> = by_scope.keys().cloned().collect();
    scopes.sort();
    if let Some(pos) = scopes.iter().position(|s| s == "root") {
        let root = scopes.remove(pos);
        scopes.insert(0, root);
    }

    let mut canon_rows: Vec<Value> = Vec::new();

    for scope in scopes {
        let scoped = by_scope.get(&scope).cloned().unwrap_or_default();

        let mut about_rows: Vec<(usize, Vec<Value>)> = Vec::new();
        let mut help_rows: Vec<(usize, Vec<Value>)> = Vec::new();
        let mut version_rows: Vec<(usize, Vec<Value>)> = Vec::new();
        let mut flag_rows: Vec<(usize, Vec<Value>)> = Vec::new();
        let mut opt_rows: Vec<(usize, Vec<Value>)> = Vec::new();
        let mut arg_rows: Vec<(usize, Vec<Value>)> = Vec::new();

        let mut seen = SeenOpts::default();
        let mut key_seen: std::collections::BTreeMap<String, usize> =
            std::collections::BTreeMap::new();

        for (row_index, row) in &scoped {
            let kind = row_get_str(row, 1);
            match kind {
                "about" => about_rows.push((*row_index, row.clone())),
                "help" => help_rows.push((*row_index, row.clone())),
                "version" => version_rows.push((*row_index, row.clone())),
                "flag" => flag_rows.push((*row_index, row.clone())),
                "opt" => opt_rows.push((*row_index, row.clone())),
                "arg" => arg_rows.push((*row_index, row.clone())),
                _ => {
                    diags.push(diag(
                        "ECLI_ROW_KIND_UNKNOWN",
                        format!("unknown row kind {kind:?}"),
                        &scope,
                        *row_index as i32,
                    ));
                }
            }
        }

        if about_rows.len() > 1 {
            diags.push(diag(
                "ECLI_ABOUT_DUP",
                "more than one about row in scope".to_string(),
                &scope,
                about_rows[1].0 as i32,
            ));
        }
        if help_rows.len() > 1 {
            diags.push(diag(
                "ECLI_HELP_DUP",
                "more than one help row in scope".to_string(),
                &scope,
                help_rows[1].0 as i32,
            ));
        }
        if version_rows.len() > 1 {
            diags.push(diag(
                "ECLI_VERSION_DUP",
                "more than one version row in scope".to_string(),
                &scope,
                version_rows[1].0 as i32,
            ));
        }

        // help rows: [scope,"help",short,long,desc]
        for (row_index, row) in &help_rows {
            let short_opt = row_get_str(row, 2);
            let long_opt = row_get_str(row, 3);
            check_short_long(
                &mut diags, &scope, "help", *row_index, short_opt, long_opt, &mut seen,
            );
        }

        // version rows: [scope,"version",short,long,desc]
        for (row_index, row) in &version_rows {
            let short_opt = row_get_str(row, 2);
            let long_opt = row_get_str(row, 3);
            check_short_long(
                &mut diags, &scope, "version", *row_index, short_opt, long_opt, &mut seen,
            );
        }

        // flags: [scope,"flag",short,long,key,desc,(meta?)]
        for (row_index, row) in &flag_rows {
            let short_opt = row_get_str(row, 2);
            let long_opt = row_get_str(row, 3);
            let key = row_get_str(row, 4);
            check_short_long(
                &mut diags, &scope, "flag", *row_index, short_opt, long_opt, &mut seen,
            );

            if key_seen.contains_key(key) {
                diags.push(diag(
                    "ECLI_DUP_KEY",
                    format!("duplicate key {key}"),
                    &scope,
                    *row_index as i32,
                ));
            } else {
                key_seen.insert(key.to_string(), *row_index);
            }

            if let Some(meta) = row_get_meta(row, 6) {
                if let Some(meta_key) = meta.get("key").and_then(Value::as_str) {
                    if meta_key != key {
                        diags.push(diag(
                            "ECLI_META_KEY_MISMATCH",
                            format!("meta.key {meta_key:?} does not match key {key:?}"),
                            &scope,
                            *row_index as i32,
                        ));
                    }
                }
            }
        }

        // opts: [scope,"opt",short,long,key,value_kind,desc,(meta?)]
        let allowed_value_kinds = [
            "STR",
            "PATH",
            "U32",
            "I32",
            "BOOL",
            "ENUM",
            "BYTES",
            "BYTES_HEX",
        ];
        for (row_index, row) in &opt_rows {
            let short_opt = row_get_str(row, 2);
            let long_opt = row_get_str(row, 3);
            let key = row_get_str(row, 4);
            let value_kind = row_get_str(row, 5);

            check_short_long(
                &mut diags, &scope, "opt", *row_index, short_opt, long_opt, &mut seen,
            );

            if key_seen.contains_key(key) {
                diags.push(diag(
                    "ECLI_DUP_KEY",
                    format!("duplicate key {key}"),
                    &scope,
                    *row_index as i32,
                ));
            } else {
                key_seen.insert(key.to_string(), *row_index);
            }

            if !allowed_value_kinds.contains(&value_kind) {
                diags.push(diag(
                    "ECLI_OPT_VALUE_KIND_UNKNOWN",
                    format!("unknown value_kind {value_kind:?}"),
                    &scope,
                    *row_index as i32,
                ));
            }

            if let Some(meta) = row_get_meta(row, 7) {
                if let Some(meta_key) = meta.get("key").and_then(Value::as_str) {
                    if meta_key != key {
                        diags.push(diag(
                            "ECLI_META_KEY_MISMATCH",
                            format!("meta.key {meta_key:?} does not match key {key:?}"),
                            &scope,
                            *row_index as i32,
                        ));
                    }
                }

                if value_kind == "ENUM" {
                    if !meta.contains_key("enum") {
                        diags.push(diag(
                            "ECLI_ENUM_MISSING",
                            "ENUM options must provide meta.enum (list of allowed values)"
                                .to_string(),
                            &scope,
                            *row_index as i32,
                        ));
                    } else if let (Some(enum_vals), Some(default_val)) = (
                        meta.get("enum").and_then(Value::as_array),
                        meta.get("default"),
                    ) {
                        let allowed: std::collections::BTreeSet<&str> =
                            enum_vals.iter().filter_map(Value::as_str).collect();
                        if let Some(s) = default_val.as_str() {
                            if !allowed.contains(s) {
                                diags.push(diag(
                                    "ECLI_ENUM_DEFAULT_INVALID",
                                    "default is not one of meta.enum values".to_string(),
                                    &scope,
                                    *row_index as i32,
                                ));
                            }
                        }
                    }
                }

                if let Some(default_val) = meta.get("default") {
                    if allowed_value_kinds.contains(&value_kind)
                        && !default_parse_ok(value_kind, default_val)
                    {
                        diags.push(diag(
                            "ECLI_OPT_DEFAULT_INVALID",
                            format!("default is not valid for {value_kind}"),
                            &scope,
                            *row_index as i32,
                        ));
                    }
                }
            }
        }

        // args: [scope,"arg",POS_NAME,key,desc,(meta?)]
        let mut saw_optional = false;
        let mut saw_multi = false;
        for (pos, (row_index, row)) in arg_rows.iter().enumerate() {
            let key = row_get_str(row, 3);
            if key_seen.contains_key(key) {
                diags.push(diag(
                    "ECLI_DUP_KEY",
                    format!("duplicate key {key}"),
                    &scope,
                    *row_index as i32,
                ));
            } else {
                key_seen.insert(key.to_string(), *row_index);
            }

            let mut required = true;
            let mut multiple = false;
            if let Some(meta) = row_get_meta(row, 5) {
                if let Some(v) = meta.get("required") {
                    required = v.as_bool().unwrap_or(true);
                }
                if let Some(v) = meta.get("multiple") {
                    multiple = v.as_bool().unwrap_or(false);
                }
            }

            if !required {
                saw_optional = true;
            } else if saw_optional {
                diags.push(diag(
                    "ECLI_ARG_REQUIRED_AFTER_OPTIONAL",
                    "required arg appears after optional arg".to_string(),
                    &scope,
                    *row_index as i32,
                ));
            }

            if multiple {
                if saw_multi {
                    diags.push(diag(
                        "ECLI_ARG_MULTI_DUP",
                        "more than one arg has multiple=true".to_string(),
                        &scope,
                        *row_index as i32,
                    ));
                }
                saw_multi = true;
                if pos != arg_rows.len().saturating_sub(1) {
                    diags.push(diag(
                        "ECLI_ARG_MULTI_NOT_LAST",
                        "arg with multiple=true must be last".to_string(),
                        &scope,
                        *row_index as i32,
                    ));
                }
            }
        }

        // Canonical row ordering per scope
        if let Some((_, r)) = about_rows.first() {
            canon_rows.push(Value::Array(r.clone()));
        }

        if let Some((_, r)) = help_rows.first() {
            canon_rows.push(Value::Array(r.clone()));
        } else {
            // Insert help if missing and safe
            if !seen.long.contains_key(RESERVED_LONG_HELP) {
                let short = if seen.short.contains_key(RESERVED_SHORT_HELP) {
                    ""
                } else {
                    RESERVED_SHORT_HELP
                };
                canon_rows.push(Value::Array(vec![
                    Value::String(scope.clone()),
                    Value::String("help".to_string()),
                    Value::String(short.to_string()),
                    Value::String(RESERVED_LONG_HELP.to_string()),
                    Value::String("Show help".to_string()),
                ]));
            }
        }

        if let Some((_, r)) = version_rows.first() {
            canon_rows.push(Value::Array(r.clone()));
        } else if scope == "root" && !seen.long.contains_key(RESERVED_LONG_VERSION) {
            let short = if seen.short.contains_key(RESERVED_SHORT_VERSION) {
                ""
            } else {
                RESERVED_SHORT_VERSION
            };
            canon_rows.push(Value::Array(vec![
                Value::String("root".to_string()),
                Value::String("version".to_string()),
                Value::String(short.to_string()),
                Value::String(RESERVED_LONG_VERSION.to_string()),
                Value::String("Show version".to_string()),
            ]));
        }

        let mut flag_sorted = flag_rows.clone();
        flag_sorted.sort_by(|a, b| {
            let ra = &a.1;
            let rb = &b.1;
            let a_long = row_get_str(ra, 3);
            let a_short = row_get_str(ra, 2);
            let a_key = row_get_str(ra, 4);
            let b_long = row_get_str(rb, 3);
            let b_short = row_get_str(rb, 2);
            let b_key = row_get_str(rb, 4);
            (
                canon_sort_key_str(a_long),
                canon_sort_key_str(a_short),
                a_key,
            )
                .cmp(&(
                    canon_sort_key_str(b_long),
                    canon_sort_key_str(b_short),
                    b_key,
                ))
        });
        for (_, r) in flag_sorted {
            canon_rows.push(Value::Array(r));
        }

        let mut opt_sorted = opt_rows.clone();
        opt_sorted.sort_by(|a, b| {
            let ra = &a.1;
            let rb = &b.1;
            let a_long = row_get_str(ra, 3);
            let a_short = row_get_str(ra, 2);
            let a_key = row_get_str(ra, 4);
            let b_long = row_get_str(rb, 3);
            let b_short = row_get_str(rb, 2);
            let b_key = row_get_str(rb, 4);
            (
                canon_sort_key_str(a_long),
                canon_sort_key_str(a_short),
                a_key,
            )
                .cmp(&(
                    canon_sort_key_str(b_long),
                    canon_sort_key_str(b_short),
                    b_key,
                ))
        });
        for (_, r) in opt_sorted {
            canon_rows.push(Value::Array(r));
        }

        for (_, r) in arg_rows {
            canon_rows.push(Value::Array(r));
        }
    }

    diags.sort_by(|a, b| {
        (
            a.code.as_str(),
            a.scope.as_str(),
            a.row_index,
            a.message.as_str(),
        )
            .cmp(&(
                b.code.as_str(),
                b.scope.as_str(),
                b.row_index,
                b.message.as_str(),
            ))
    });

    let canon = {
        let mut out = doc.clone();
        if let Some(obj) = out.as_object_mut() {
            obj.insert(
                "schema_version".to_string(),
                Value::String("x07cli.specrows@0.1.0".to_string()),
            );
            obj.insert("rows".to_string(), Value::Array(canon_rows));
        }
        out
    };

    CanonResult {
        canon,
        diagnostics: diags,
    }
}

fn default_parse_ok(value_kind: &str, default_val: &Value) -> bool {
    if default_val.is_null() {
        return false;
    }

    match value_kind {
        "STR" | "PATH" | "BYTES" | "ENUM" => default_val.as_str().is_some(),
        "BOOL" => {
            if default_val.as_bool().is_some() {
                return true;
            }
            let Some(s) = default_val.as_str() else {
                return false;
            };
            matches!(
                s,
                "0" | "1" | "true" | "false" | "yes" | "no" | "on" | "off"
            )
        }
        "BYTES_HEX" => {
            let Some(s) = default_val.as_str() else {
                return false;
            };
            let s = s.trim();
            if s.len() % 2 != 0 {
                return false;
            }
            s.bytes().all(|b| b.is_ascii_hexdigit())
        }
        "U32" => {
            if let Some(n) = default_val.as_u64() {
                return n <= u32::MAX as u64;
            }
            let Some(s) = default_val.as_str() else {
                return false;
            };
            if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
                return false;
            }
            let Ok(n) = s.parse::<u64>() else {
                return false;
            };
            n <= u32::MAX as u64
        }
        "I32" => {
            if let Some(n) = default_val.as_i64() {
                return n >= i32::MIN as i64 && n <= i32::MAX as i64;
            }
            let Some(s) = default_val.as_str() else {
                return false;
            };
            let (sign, digits) = s.strip_prefix('-').map_or((1i64, s), |_| (-1, &s[1..]));
            if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
                return false;
            }
            let Ok(n) = digits.parse::<i64>() else {
                return false;
            };
            let n = n.saturating_mul(sign);
            n >= i32::MIN as i64 && n <= i32::MAX as i64
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn assert_has_code(diags: &[X07CliDiagnostic], code: &str) {
        assert!(
            diags.iter().any(|d| d.code == code),
            "expected diagnostics to include {code}, got: {diags:?}"
        );
    }

    #[test]
    fn canon_inserts_help_and_version() {
        let doc = json!({
            "schema_version": "x07cli.specrows@0.1.0",
            "app": { "name": "tool", "version": "0.1.0" },
            "rows": [
                ["root", "about", "demo"]
            ]
        });
        let res = validate_and_canon(&doc);
        assert!(res.diagnostics.is_empty(), "{:?}", res.diagnostics);
        let rows = res.canon.get("rows").unwrap().as_array().unwrap();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0], json!(["root", "about", "demo"]));
        assert_eq!(
            rows[1],
            json!(["root", "help", "-h", "--help", "Show help"])
        );
        assert_eq!(
            rows[2],
            json!(["root", "version", "-V", "--version", "Show version"])
        );
    }

    #[test]
    fn detects_duplicate_key() {
        let doc = json!({
            "schema_version": "x07cli.specrows@0.1.0",
            "app": { "name": "tool", "version": "0.1.0" },
            "rows": [
                ["root", "flag", "-v", "--verbose", "k", "Verbose"],
                ["root", "opt", "", "--x", "k", "STR", "X", { "required": true }]
            ]
        });
        let res = validate_and_canon(&doc);
        assert_has_code(&res.diagnostics, "ECLI_DUP_KEY");
    }

    #[test]
    fn enum_requires_meta_enum() {
        let doc = json!({
            "schema_version": "x07cli.specrows@0.1.0",
            "app": { "name": "tool", "version": "0.1.0" },
            "rows": [
                ["root", "opt", "", "--mode", "mode", "ENUM", "Mode", { "default": "safe" }]
            ]
        });
        let res = validate_and_canon(&doc);
        assert_has_code(&res.diagnostics, "ECLI_ENUM_MISSING");
    }

    #[test]
    fn validates_default_bytes_hex() {
        let doc = json!({
            "schema_version": "x07cli.specrows@0.1.0",
            "app": { "name": "tool", "version": "0.1.0" },
            "rows": [
                ["root", "opt", "", "--token", "token", "BYTES_HEX", "Token", { "default": "abc" }]
            ]
        });
        let res = validate_and_canon(&doc);
        assert_has_code(&res.diagnostics, "ECLI_OPT_DEFAULT_INVALID");
    }

    #[test]
    fn required_arg_after_optional_is_error() {
        let doc = json!({
            "schema_version": "x07cli.specrows@0.1.0",
            "app": { "name": "tool", "version": "0.1.0" },
            "rows": [
                ["root", "arg", "FIRST", "first", "First", { "required": false }],
                ["root", "arg", "SECOND", "second", "Second", { "required": true }]
            ]
        });
        let res = validate_and_canon(&doc);
        assert_has_code(&res.diagnostics, "ECLI_ARG_REQUIRED_AFTER_OPTIONAL");
    }

    #[test]
    fn reserved_help_used_by_non_help_row_is_error() {
        let doc = json!({
            "schema_version": "x07cli.specrows@0.1.0",
            "app": { "name": "tool", "version": "0.1.0" },
            "rows": [
                ["root", "flag", "", "--help", "h", "oops"]
            ]
        });
        let res = validate_and_canon(&doc);
        assert_has_code(&res.diagnostics, "ECLI_RESERVED_HELP_USED");
    }
}
