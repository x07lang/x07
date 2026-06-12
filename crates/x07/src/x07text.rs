//! x07text: a lossless, deterministic text projection of x07AST JSON.
//!
//! The canonical source format stays JSON (`*.x07.json`); x07text is a
//! read/write surface for humans and agents. Mapping is schema-agnostic so
//! every current and future x07AST field round-trips:
//!
//! - JSON string  <-> bare atom when safe, else double-quoted string
//! - JSON number  <-> bare integer (non-integer numbers use `#num"..."`)
//! - JSON bool    <-> `true` / `false` (the strings "true"/"false" stay quoted)
//! - JSON null    <-> `null`
//! - JSON array   <-> `(item ...)`
//! - JSON object  <-> `{:key value ...}`
//!
//! `;` starts a line comment in text input; comments are not preserved in the
//! canonical JSON. Whitespace is insignificant outside strings, so the pretty
//! layout carries no information.

use anyhow::{bail, Result};
use serde_json::Value;

const INLINE_WIDTH: usize = 80;

/// Object keys printed before the remaining (alphabetical) keys, because they
/// carry the salient identity of a node when humans scan a file.
const KEY_PRIORITY: [&str; 5] = ["kind", "name", "module_id", "schema_version", "imports"];

pub fn to_text(value: &Value) -> String {
    let mut out = String::new();
    render(value, 0, &mut out);
    out.push('\n');
    out
}

pub fn from_text(input: &str) -> Result<Value> {
    let mut p = Parser::new(input);
    p.skip_trivia();
    let mut v = p.parse_value()?;
    p.skip_trivia();
    if !p.at_end() {
        bail!(
            "trailing content at line {} column {}",
            p.line(),
            p.column()
        );
    }
    // x07AST documents require `decls`; entry files routinely have none, so
    // default it to an empty list instead of failing validation.
    if let Value::Object(obj) = &mut v {
        if obj.get("kind").and_then(Value::as_str).is_some() && !obj.contains_key("decls") {
            obj.insert("decls".to_string(), Value::Array(Vec::new()));
        }
    }
    Ok(v)
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

fn is_atom_start(c: char) -> bool {
    c.is_ascii_alphabetic() || "_+-*/<>=!?&|%^~$.".contains(c)
}

fn is_atom_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || "_+-*/<>=!?&|%^~$.@".contains(c)
}

fn is_safe_atom(s: &str) -> bool {
    if s.is_empty() || s == "true" || s == "false" || s == "null" {
        return false;
    }
    if parses_as_integer(s) {
        return false;
    }
    let mut chars = s.chars();
    let first = chars.next().expect("non-empty");
    is_atom_start(first) && chars.all(is_atom_char)
}

fn parses_as_integer(s: &str) -> bool {
    let body = s.strip_prefix('-').unwrap_or(s);
    !body.is_empty() && body.bytes().all(|b| b.is_ascii_digit())
}

fn render_string(s: &str, out: &mut String) {
    if is_safe_atom(s) {
        out.push_str(s);
        return;
    }
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{{{:x}}}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

fn render_number(n: &serde_json::Number, out: &mut String) {
    if n.is_i64() || n.is_u64() {
        out.push_str(&n.to_string());
    } else {
        // Non-integer numbers do not occur in x07AST proper (i32 only) but can
        // appear in free-form metadata; keep the exact serde representation.
        out.push_str("#num\"");
        out.push_str(&n.to_string());
        out.push('"');
    }
}

fn ordered_keys(map: &serde_json::Map<String, Value>) -> Vec<&String> {
    let mut keys: Vec<&String> = map.keys().collect();
    keys.sort_by_key(|k| {
        (
            KEY_PRIORITY
                .iter()
                .position(|p| p == k)
                .unwrap_or(KEY_PRIORITY.len()),
            k.as_str(),
        )
    });
    keys
}

fn render_inline(value: &Value, out: &mut String) {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(true) => out.push_str("true"),
        Value::Bool(false) => out.push_str("false"),
        Value::Number(n) => render_number(n, out),
        Value::String(s) => render_string(s, out),
        Value::Array(items) => {
            out.push('(');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(' ');
                }
                render_inline(item, out);
            }
            out.push(')');
        }
        Value::Object(map) => {
            out.push('{');
            for (i, key) in ordered_keys(map).iter().enumerate() {
                if i > 0 {
                    out.push(' ');
                }
                out.push(':');
                render_string(key, out);
                out.push(' ');
                render_inline(&map[key.as_str()], out);
            }
            out.push('}');
        }
    }
}

fn inline_len(value: &Value) -> usize {
    let mut s = String::new();
    render_inline(value, &mut s);
    s.len()
}

fn indent(depth: usize, out: &mut String) {
    for _ in 0..depth {
        out.push_str("  ");
    }
}

fn render(value: &Value, depth: usize, out: &mut String) {
    match value {
        Value::Array(items) => {
            if inline_len(value) + 2 * depth <= INLINE_WIDTH {
                render_inline(value, out);
                return;
            }
            out.push('(');
            // Keep the head plus leading scalar arguments on the opening line
            // ("(for i 0 n" / "(let toks ..."), then break per item.
            let mut head_len = 0usize;
            let mut idx = 0usize;
            while idx < items.len() {
                let item = &items[idx];
                let is_scalar = !matches!(item, Value::Array(_) | Value::Object(_));
                let take_as_head = idx == 0 || (is_scalar && idx <= 3);
                if !take_as_head {
                    break;
                }
                let len = inline_len(item);
                if idx > 0 && head_len + len + 2 * depth > INLINE_WIDTH {
                    break;
                }
                if idx > 0 {
                    out.push(' ');
                }
                render_inline(item, out);
                head_len += len + 1;
                idx += 1;
            }
            for item in &items[idx..] {
                out.push('\n');
                indent(depth + 1, out);
                render(item, depth + 1, out);
            }
            out.push('\n');
            indent(depth, out);
            out.push(')');
        }
        Value::Object(map) => {
            if inline_len(value) + 2 * depth <= INLINE_WIDTH {
                render_inline(value, out);
                return;
            }
            out.push('{');
            for key in ordered_keys(map) {
                out.push('\n');
                indent(depth + 1, out);
                out.push(':');
                render_string(key, out);
                out.push(' ');
                render(&map[key.as_str()], depth + 1, out);
            }
            out.push('\n');
            indent(depth, out);
            out.push('}');
        }
        _ => render_inline(value, out),
    }
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

struct Parser {
    chars: Vec<char>,
    pos: usize,
}

impl Parser {
    fn new(src: &str) -> Self {
        Self {
            chars: src.chars().collect(),
            pos: 0,
        }
    }

    fn at_end(&self) -> bool {
        self.pos >= self.chars.len()
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek();
        if c.is_some() {
            self.pos += 1;
        }
        c
    }

    fn line(&self) -> usize {
        1 + self.chars[..self.pos]
            .iter()
            .filter(|c| **c == '\n')
            .count()
    }

    fn column(&self) -> usize {
        let mut col = 1;
        for c in self.chars[..self.pos].iter().rev() {
            if *c == '\n' {
                break;
            }
            col += 1;
        }
        col
    }

    fn err(&self, msg: &str) -> anyhow::Error {
        anyhow::anyhow!("{} at line {} column {}", msg, self.line(), self.column())
    }

    fn skip_trivia(&mut self) {
        loop {
            match self.peek() {
                Some(c) if c.is_whitespace() => {
                    self.pos += 1;
                }
                Some(';') => {
                    while let Some(c) = self.peek() {
                        self.pos += 1;
                        if c == '\n' {
                            break;
                        }
                    }
                }
                _ => return,
            }
        }
    }

    fn parse_value(&mut self) -> Result<Value> {
        self.skip_trivia();
        match self.peek() {
            None => Err(self.err("unexpected end of input")),
            Some('(') => self.parse_list(),
            Some('{') => self.parse_map(),
            Some('"') => Ok(Value::String(self.parse_quoted()?)),
            Some('#') => self.parse_rawnum(),
            Some(')') | Some('}') => Err(self.err("unexpected closing delimiter")),
            Some(':') => Err(self.err("map key outside of {}")),
            Some(_) => self.parse_token(),
        }
    }

    fn parse_list(&mut self) -> Result<Value> {
        self.bump(); // '('
        let mut items = Vec::new();
        loop {
            self.skip_trivia();
            match self.peek() {
                None => return Err(self.err("unclosed list")),
                Some(')') => {
                    self.bump();
                    return Ok(Value::Array(items));
                }
                _ => items.push(self.parse_value()?),
            }
        }
    }

    fn parse_map(&mut self) -> Result<Value> {
        self.bump(); // '{'
        let mut map = serde_json::Map::new();
        loop {
            self.skip_trivia();
            match self.peek() {
                None => return Err(self.err("unclosed map")),
                Some('}') => {
                    self.bump();
                    return Ok(Value::Object(map));
                }
                Some(':') => {
                    self.bump();
                    let key = if self.peek() == Some('"') {
                        self.parse_quoted()?
                    } else {
                        self.parse_atom_chars()?
                    };
                    let value = self.parse_value()?;
                    if map.insert(key.clone(), value).is_some() {
                        return Err(self.err(&format!("duplicate map key {key:?}")));
                    }
                }
                Some(_) => return Err(self.err("expected :key inside map")),
            }
        }
    }

    fn parse_quoted(&mut self) -> Result<String> {
        self.bump(); // '"'
        let mut s = String::new();
        loop {
            let Some(c) = self.bump() else {
                return Err(self.err("unclosed string"));
            };
            match c {
                '"' => return Ok(s),
                '\\' => {
                    let Some(esc) = self.bump() else {
                        return Err(self.err("unclosed escape"));
                    };
                    match esc {
                        '"' => s.push('"'),
                        '\\' => s.push('\\'),
                        'n' => s.push('\n'),
                        'r' => s.push('\r'),
                        't' => s.push('\t'),
                        'u' => {
                            if self.bump() != Some('{') {
                                return Err(self.err("expected { after \\u"));
                            }
                            let mut hex = String::new();
                            loop {
                                match self.bump() {
                                    Some('}') => break,
                                    Some(c) if c.is_ascii_hexdigit() => hex.push(c),
                                    _ => return Err(self.err("invalid \\u{...} escape")),
                                }
                            }
                            let code = u32::from_str_radix(&hex, 16)
                                .map_err(|_| self.err("invalid \\u{...} escape"))?;
                            let c = char::from_u32(code)
                                .ok_or_else(|| self.err("invalid unicode scalar"))?;
                            s.push(c);
                        }
                        _ => return Err(self.err(&format!("unknown escape \\{esc}"))),
                    }
                }
                c => s.push(c),
            }
        }
    }

    fn parse_rawnum(&mut self) -> Result<Value> {
        for expected in "#num".chars() {
            if self.bump() != Some(expected) {
                return Err(self.err("expected #num\"...\""));
            }
        }
        if self.peek() != Some('"') {
            return Err(self.err("expected quoted payload after #num"));
        }
        let raw = self.parse_quoted()?;
        let n: serde_json::Number = serde_json::from_str(&raw)
            .map_err(|_| self.err(&format!("invalid number payload {raw:?}")))?;
        Ok(Value::Number(n))
    }

    fn parse_atom_chars(&mut self) -> Result<String> {
        let mut s = String::new();
        while let Some(c) = self.peek() {
            if is_atom_char(c) {
                s.push(c);
                self.pos += 1;
            } else {
                break;
            }
        }
        if s.is_empty() {
            return Err(self.err("expected token"));
        }
        Ok(s)
    }

    fn parse_token(&mut self) -> Result<Value> {
        let start = self.pos;
        let s = self.parse_atom_chars()?;
        match s.as_str() {
            "true" => return Ok(Value::Bool(true)),
            "false" => return Ok(Value::Bool(false)),
            "null" => return Ok(Value::Null),
            _ => {}
        }
        if parses_as_integer(&s) {
            if let Ok(i) = s.parse::<i64>() {
                return Ok(Value::Number(i.into()));
            }
            if let Ok(u) = s.parse::<u64>() {
                return Ok(Value::Number(u.into()));
            }
            self.pos = start;
            return Err(self.err("integer out of range"));
        }
        let mut chars = s.chars();
        let first = chars.next().expect("non-empty token");
        if !is_atom_start(first) {
            self.pos = start;
            return Err(self.err(&format!("invalid token {s:?}")));
        }
        Ok(Value::String(s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip(v: &Value) {
        let text = to_text(v);
        let back = from_text(&text).expect("parse rendered text");
        assert_eq!(&back, v, "round-trip mismatch; text was:\n{text}");
    }

    #[test]
    fn scalars_roundtrip() {
        roundtrip(&serde_json::json!(0));
        roundtrip(&serde_json::json!(-2147483648i64));
        roundtrip(&serde_json::json!(u64::MAX));
        roundtrip(&serde_json::json!("atom"));
        roundtrip(&serde_json::json!("two words"));
        roundtrip(&serde_json::json!("42"));
        roundtrip(&serde_json::json!("-7"));
        roundtrip(&serde_json::json!("true"));
        roundtrip(&serde_json::json!(""));
        roundtrip(&serde_json::json!("line\nbreak\t\"quoted\" \\ \u{1}"));
        roundtrip(&serde_json::json!(true));
        roundtrip(&serde_json::json!(false));
        roundtrip(&serde_json::json!(null));
        roundtrip(&serde_json::json!(1.5));
    }

    #[test]
    fn expressions_roundtrip() {
        roundtrip(&serde_json::json!([
            "begin",
            ["let", "toks", ["std.text.ascii.tokenize_words_lower", "b"]],
            [
                "set",
                "out",
                ["bytes.concat", "out", ["bytes.lit", " the="]]
            ],
            [
                "if",
                [">=u", "i", "end"],
                ["return", ["result_i32.err", 1]],
                0
            ],
            "out"
        ]));
    }

    #[test]
    fn module_object_roundtrips() {
        roundtrip(&serde_json::json!({
            "schema_version": "x07.x07ast@0.8.0",
            "kind": "module",
            "module_id": "app",
            "imports": ["std.fmt", "std.small_map"],
            "decls": [
                {"kind": "export", "names": ["app.solve"]},
                {
                    "kind": "defn",
                    "name": "app.solve",
                    "params": [{"name": "b", "ty": "bytes_view"}],
                    "result": "bytes",
                    "body": ["view.to_bytes", "b"]
                }
            ]
        }));
    }

    #[test]
    fn operators_render_as_atoms() {
        let v = serde_json::json!(["+", ">=u", "==", "-", "std.u32.read_le_at"]);
        let text = to_text(&v);
        assert!(
            !text.contains('"'),
            "operators should not be quoted: {text}"
        );
        roundtrip(&v);
    }

    #[test]
    fn comments_are_ignored() {
        let v = from_text("; leading comment\n(begin ; trailing\n  1)\n").expect("parse");
        assert_eq!(v, serde_json::json!(["begin", 1]));
    }

    #[test]
    fn duplicate_keys_rejected() {
        assert!(from_text("{:a 1 :a 2}").is_err());
    }

    #[test]
    fn errors_carry_position() {
        let err = from_text("(begin\n  (let x 1\n").unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("line"), "error should name a line: {msg}");
    }
}
