# Stdlib

X07 includes a small stdlib for:

- bytes/views
- parsing/formatting
- text & validated UTF-8 strings (`std.text`, `std.str`)
- deterministic collections
- deterministic PRNG
- I/O traits (so fixture and OS adapters share a surface)

The canonical index of built-in stdlib modules and exports is published here:

- https://x07lang.org/agent/latest/stdlib/index.json

The stdlib should remain:

- stable,
- small,
- canonical.

Larger features belong in external packages.
