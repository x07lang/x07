# x07import diagnostics catalog

This document is generated from `crates/x07import-core/src/diagnostics.rs`.

| Code | Phase | Severity | Message | Help |
| ---- | ----- | -------- | ------- | ---- |
| X7I0001 | Parse | Error | failed to parse source file | Ensure the file parses as Rust/C and contains only supported items for x07import v1. |
| X7I0100 | Validate | Error | unsupported top-level item |  |
| X7I0110 | Validate | Error | unsupported function signature |  |
| X7I0111 | Validate | Error | unsupported parameter pattern |  |
| X7I0120 | Validate | Error | unsupported type |  |
| X7I0122 | Validate | Error | unsupported reference type |  |
| X7I0123 | Validate | Error | mutable references are not supported | Rewrite the code to avoid &mut. |
| X7I0200 | Validate | Error | unsupported let pattern |  |
| X7I0201 | Validate | Error | let binding is missing initializer |  |
| X7I0210 | Validate | Error | statement items are not supported |  |
| X7I0211 | Validate | Error | statement macros are not supported |  |
| X7I0300 | Validate | Error | unsupported literal |  |
| X7I0301 | Validate | Error | integer literal out of range |  |
| X7I0310 | Validate | Error | unsupported path |  |
| X7I0311 | Validate | Error | unknown name |  |
| X7I0320 | Validate | Error | unsupported call target |  |
| X7I0330 | Validate | Error | unsupported method call |  |
| X7I0340 | Validate | Error | unsupported binary operator |  |
| X7I0350 | Validate | Error | if branches have mismatched types |  |
| X7I0360 | Validate | Error | unsupported for-loop iterator |  |
| X7I0901 | Internal | Error | internal x07import bug | This is a bug in x07import. Please report it with the input source file. |
