use std::collections::BTreeMap;

use clap::{Arg, Command, ValueHint};
use serde_json::{Map, Value};

pub const X07CLI_SPECROWS_SCHEMA_VERSION: &str = "x07cli.specrows@0.1.0";

pub fn command_to_specrows(cmd: &Command) -> Value {
    let mut obj = Map::new();
    obj.insert(
        "schema_version".to_string(),
        Value::String(X07CLI_SPECROWS_SCHEMA_VERSION.to_string()),
    );
    obj.insert("app".to_string(), app_meta(cmd));
    obj.insert("rows".to_string(), Value::Array(rows_for_command(cmd)));
    Value::Object(obj)
}

pub fn find_command<'a>(root: &'a Command, subcommand_path: &[&str]) -> Option<&'a Command> {
    let mut cur = root;
    for name in subcommand_path {
        cur = cur.get_subcommands().find(|c| c.get_name() == *name)?;
    }
    Some(cur)
}

fn app_meta(cmd: &Command) -> Value {
    let mut app = Map::new();
    app.insert(
        "name".to_string(),
        Value::String(cmd.get_name().to_string()),
    );

    if let Some(version) = cmd.get_version() {
        let s = version.to_string();
        if !s.trim().is_empty() {
            app.insert("version".to_string(), Value::String(s));
        }
    }

    let about = cmd
        .get_about()
        .map(|s| s.to_string())
        .or_else(|| cmd.get_long_about().map(|s| s.to_string()))
        .unwrap_or_default();
    if !about.trim().is_empty() {
        app.insert("about".to_string(), Value::String(about.trim().to_string()));
    }

    Value::Object(app)
}

fn rows_for_command(cmd: &Command) -> Vec<Value> {
    let mut rows: Vec<Value> = Vec::new();

    push_scope_rows(&mut rows, "root", cmd);

    let mut subs: Vec<&Command> = cmd.get_subcommands().collect();
    subs.sort_by(|a, b| a.get_name().cmp(b.get_name()));
    for sc in subs {
        push_scope_rows_recursive(&mut rows, sc.get_name(), sc);
    }

    rows
}

fn push_scope_rows_recursive(rows: &mut Vec<Value>, scope: &str, cmd: &Command) {
    push_scope_rows(rows, scope, cmd);

    let mut subs: Vec<&Command> = cmd.get_subcommands().collect();
    subs.sort_by(|a, b| a.get_name().cmp(b.get_name()));
    for sc in subs {
        let nested = format!("{scope}.{}", sc.get_name());
        push_scope_rows_recursive(rows, &nested, sc);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Row {
    About {
        scope: String,
        desc: String,
    },
    Help {
        scope: String,
        short: String,
        long: String,
        desc: String,
    },
    Version {
        scope: String,
        short: String,
        long: String,
        desc: String,
    },
    Flag {
        scope: String,
        short: String,
        long: String,
        key: String,
        desc: String,
        meta: BTreeMap<String, Value>,
    },
    Opt {
        scope: String,
        short: String,
        long: String,
        key: String,
        value_kind: String,
        desc: String,
        meta: BTreeMap<String, Value>,
    },
    Arg {
        scope: String,
        pos_name: String,
        key: String,
        desc: String,
        meta: BTreeMap<String, Value>,
    },
}

impl Row {
    fn kind_order(&self) -> u8 {
        match self {
            Row::About { .. } => 0,
            Row::Help { .. } => 1,
            Row::Version { .. } => 2,
            Row::Flag { .. } => 3,
            Row::Opt { .. } => 4,
            Row::Arg { .. } => 5,
        }
    }

    fn stable_sort_key(&self) -> (u8, String, String, String) {
        let (a, b, c) = match self {
            Row::About { .. } => ("".to_string(), "".to_string(), "".to_string()),
            Row::Help { long, short, .. } => (long.clone(), short.clone(), "".to_string()),
            Row::Version { long, short, .. } => (long.clone(), short.clone(), "".to_string()),
            Row::Flag {
                long, short, key, ..
            } => (long.clone(), short.clone(), key.clone()),
            Row::Opt {
                long, short, key, ..
            } => (long.clone(), short.clone(), key.clone()),
            Row::Arg { key, .. } => ("".to_string(), "".to_string(), key.clone()),
        };

        let empty_last = |s: String| {
            if s.is_empty() {
                "\u{10FFFF}".to_string()
            } else {
                s
            }
        };

        (
            self.kind_order(),
            empty_last(a),
            empty_last(b),
            empty_last(c),
        )
    }

    fn to_json_row(&self) -> Value {
        match self {
            Row::About { scope, desc } => Value::Array(vec![
                Value::String(scope.clone()),
                Value::String("about".to_string()),
                Value::String(desc.clone()),
            ]),
            Row::Help {
                scope,
                short,
                long,
                desc,
            } => Value::Array(vec![
                Value::String(scope.clone()),
                Value::String("help".to_string()),
                Value::String(short.clone()),
                Value::String(long.clone()),
                Value::String(desc.clone()),
            ]),
            Row::Version {
                scope,
                short,
                long,
                desc,
            } => Value::Array(vec![
                Value::String(scope.clone()),
                Value::String("version".to_string()),
                Value::String(short.clone()),
                Value::String(long.clone()),
                Value::String(desc.clone()),
            ]),
            Row::Flag {
                scope,
                short,
                long,
                key,
                desc,
                meta,
            } => {
                let mut v = vec![
                    Value::String(scope.clone()),
                    Value::String("flag".to_string()),
                    Value::String(short.clone()),
                    Value::String(long.clone()),
                    Value::String(key.clone()),
                    Value::String(desc.clone()),
                ];
                if !meta.is_empty() {
                    v.push(Value::Object(meta_to_json(meta)));
                }
                Value::Array(v)
            }
            Row::Opt {
                scope,
                short,
                long,
                key,
                value_kind,
                desc,
                meta,
            } => {
                let mut v = vec![
                    Value::String(scope.clone()),
                    Value::String("opt".to_string()),
                    Value::String(short.clone()),
                    Value::String(long.clone()),
                    Value::String(key.clone()),
                    Value::String(value_kind.clone()),
                    Value::String(desc.clone()),
                ];
                if !meta.is_empty() {
                    v.push(Value::Object(meta_to_json(meta)));
                }
                Value::Array(v)
            }
            Row::Arg {
                scope,
                pos_name,
                key,
                desc,
                meta,
            } => {
                let mut v = vec![
                    Value::String(scope.clone()),
                    Value::String("arg".to_string()),
                    Value::String(pos_name.clone()),
                    Value::String(key.clone()),
                    Value::String(desc.clone()),
                ];
                if !meta.is_empty() {
                    v.push(Value::Object(meta_to_json(meta)));
                }
                Value::Array(v)
            }
        }
    }
}

fn meta_to_json(meta: &BTreeMap<String, Value>) -> Map<String, Value> {
    meta.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
}

fn push_scope_rows(rows: &mut Vec<Value>, scope: &str, cmd: &Command) {
    let mut scope_rows: Vec<Row> = Vec::new();

    let about = cmd
        .get_about()
        .map(|s| s.to_string())
        .or_else(|| cmd.get_long_about().map(|s| s.to_string()))
        .unwrap_or_default();
    if !about.trim().is_empty() {
        scope_rows.push(Row::About {
            scope: scope.to_string(),
            desc: about.trim().to_string(),
        });
    }

    let mut args: Vec<&Arg> = cmd.get_arguments().collect();
    args.sort_by(|a, b| a.get_id().cmp(b.get_id()));
    for arg in args {
        if arg.is_hide_set() {
            continue;
        }
        if let Some(row) = arg_to_row(scope, arg) {
            scope_rows.push(row);
        }
    }

    scope_rows.sort_by_key(|r| r.stable_sort_key());

    rows.extend(scope_rows.into_iter().map(|r| r.to_json_row()));
}

fn arg_to_row(scope: &str, arg: &Arg) -> Option<Row> {
    if arg.is_positional() {
        let pos_name = arg
            .get_value_names()
            .and_then(|names| names.first())
            .map(|s| s.to_string())
            .unwrap_or_else(|| arg.get_id().to_string().to_ascii_uppercase());
        let key = arg.get_id().to_string();
        let desc = arg_desc(arg);
        let mut meta = BTreeMap::new();
        if arg.is_required_set() {
            meta.insert("required".to_string(), Value::Bool(true));
        }
        return Some(Row::Arg {
            scope: scope.to_string(),
            pos_name,
            key,
            desc,
            meta,
        });
    }

    let (short, long) = arg_names(arg);
    if short.is_empty() && long.is_empty() {
        return None;
    }

    if long == "--help" {
        return Some(Row::Help {
            scope: scope.to_string(),
            short,
            long,
            desc: arg_desc(arg),
        });
    }
    if scope == "root" && long == "--version" {
        return Some(Row::Version {
            scope: scope.to_string(),
            short,
            long,
            desc: arg_desc(arg),
        });
    }

    let key = arg.get_id().to_string();
    if arg.get_num_args().is_none() {
        return Some(Row::Flag {
            scope: scope.to_string(),
            short,
            long,
            key,
            desc: arg_desc(arg),
            meta: BTreeMap::new(),
        });
    }

    let value_kind = value_kind_for_arg(arg);
    let mut meta = BTreeMap::new();
    let defaults = arg.get_default_values();
    if let Some(first) = defaults.first() {
        let s = first.to_string_lossy().to_string();
        if !s.trim().is_empty() {
            meta.insert("default".to_string(), Value::String(s));
        }
    }
    Some(Row::Opt {
        scope: scope.to_string(),
        short,
        long,
        key,
        value_kind,
        desc: arg_desc(arg),
        meta,
    })
}

fn arg_desc(arg: &Arg) -> String {
    arg.get_help()
        .map(|s| s.to_string())
        .or_else(|| arg.get_long_help().map(|s| s.to_string()))
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn arg_names(arg: &Arg) -> (String, String) {
    let short = arg.get_short().map(|c| format!("-{c}")).unwrap_or_default();
    let long = arg.get_long().map(|s| format!("--{s}")).unwrap_or_default();
    (short, long)
}

fn value_kind_for_arg(arg: &Arg) -> String {
    match arg.get_value_hint() {
        ValueHint::AnyPath | ValueHint::DirPath | ValueHint::FilePath => {
            return "PATH".to_string();
        }
        _ => {}
    }

    if let Some(names) = arg.get_value_names() {
        if let Some(name) = names.first() {
            let n = name.to_string().to_ascii_uppercase();
            if n.contains("PATH") || n.contains("DIR") || n.contains("FILE") {
                return "PATH".to_string();
            }
        }
    }

    if let Some(help) = arg.get_help().map(|s| s.to_string()) {
        let n = help.to_ascii_uppercase();
        if n.contains("PATH") || n.contains("DIR") || n.contains("FILE") {
            return "PATH".to_string();
        }
    }

    "STR".to_string()
}
