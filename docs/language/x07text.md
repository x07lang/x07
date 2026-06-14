# x07text: the text projection

x07text is a lossless, deterministic text rendering of x07AST JSON. The
canonical source format is still `*.x07.json` — x07text exists so humans and
agents can read and author modules without paying the JSON quote/bracket
overhead. Conversion is exact in both directions and gated in CI by a corpus
round-trip test over the whole stdlib and fixture tree.

## Commands

```bash
x07 ast to-text --in src/app.x07.json              # writes src/app.x07t + JSON report
x07 ast to-text --in src/app.x07.json --out other.x07t

x07 ast from-text --in src/app.x07t               # writes src/app.x07.json + JSON report
x07 ast from-text --in src/app.x07t --out other.x07.json
# from-text output is canonical JCS bytes — byte-identical to `x07 fmt` output.
# --validate true (default) also requires a well-formed x07AST document.
# Both commands print an {ok, in, out, sha256} report to stdout.
```

## Format in one minute

Three composite constructs, mapped 1:1 onto JSON:

- `(head arg ...)` — JSON array (x07 expressions)
- `{:key value ...}` — JSON object (file header, decls, params, contracts)
- Scalars — bare atoms for safe strings (`app.solve`, `bytes_view`, `+`,
  `>=u`), quoted strings otherwise (`" the="`, `"42"`), bare integers, and
  `true` / `false` / `null`

`;` starts a line comment. Comments are accepted on input and dropped on
canonicalization (the canonical artifact is JSON).

Example:

```clojure
; x07text
{
  :kind module
  :module_id app
  :schema_version x07.x07ast@0.8.0
  :imports (std.fmt std.text.ascii)
  :decls ({:kind export :names (app.solve)}
    {
      :kind defn
      :name app.solve
      :body (begin
        (let toks (std.text.ascii.tokenize_words_lower b))
        (let out (bytes.lit total=))
        (set out (bytes.concat out (std.fmt.u32_to_dec n)))
        out
      )
      :params ({:name b :ty bytes_view})
      :result bytes
    }
  )
}
```

Strings that would be ambiguous as atoms stay quoted: `"42"` is the string
forty-two (bare `42` is the integer), `"true"` is a string (bare `true` is the
boolean), and anything with whitespace or structural characters is quoted with
the usual `\"` `\\` `\n` `\r` `\t` `\u{hex}` escapes.

## Workflow notes

- Entry programs: `decls` is a required x07AST field, but `from-text`
  defaults a missing `:decls` to the empty list, so `:kind entry` files that
  are only a `:solve` body may omit it. `to-text` always renders `:decls ()`
  explicitly.
- Authoring: write x07text, then `x07 ast from-text --out <file>.x07.json` and
  continue with the normal loop (`x07 run`, `x07 lint`, `x07 fix`). Whole-file
  conversion re-canonicalizes everything, so text editing cannot accumulate
  the paren/brace drift that motivated JSON-first sources.
- Review: render both sides of a change with `to-text` for readable diffs.
- Patching: JSON Patch / quickfix workflows are unchanged — they operate on
  canonical JSON, and pointers in diagnostics refer to the JSON document.

Design record: RFC 0001 (x07text surface syntax) in the `x07-rfcs` repository.
