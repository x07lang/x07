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

The current in-repo native backend and its build/CI wiring are described in the
[native backend](native-backend-v1.md) page.

See upstream references:
- OpenLibm: https://openlibm.org/ and https://github.com/JuliaMath/openlibm
- Ryu: https://github.com/ulfjack/ryu
- fast_float: https://github.com/fastfloat/fast_float
