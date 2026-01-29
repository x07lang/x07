# x07-ext-math — v1 native backend (normative guidance)

This package is intended to be deterministic across platforms by *pinning* the native implementations used for:
- float-to-string
- string-to-float
- libm transcendental functions

Recommended pinned components:
- OpenLibm (portable, standalone libm)
- Ryu (shortest round-trip float-to-string)
- fast_float (exact rounding string-to-float)

The toolchain should vendor these sources at pinned commits and build them deterministically.

Current in-repo native backend (`crates/x07-math-native`) uses:
- `libm` (pure Rust libm)
- `ryu` (formatting)
- `lexical-core` (parsing)

See upstream references:
- OpenLibm: https://openlibm.org/ and https://github.com/JuliaMath/openlibm
- Ryu: https://github.com/ulfjack/ryu
- fast_float: https://github.com/fastfloat/fast_float
