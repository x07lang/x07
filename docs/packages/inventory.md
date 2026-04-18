# Offline package inventory

When working offline (or inside an agent sandbox), it is useful to answer:

- “Which stdlib and official external packages are available in this toolchain bundle?”
- “Which module ids do they export?”

Use:

```bash
x07 pkg inventory
```

The inventory is derived from the toolchain’s checked-in package manifests under `stdlib/**/x07-package.json` and `packages/ext/**/x07-package.json`.

If you need to resolve a single missing import, prefer:

```bash
x07 pkg provides <module-id>
```

For registry browsing when online, use `x07 pkg versions`, `x07 pkg info`, or https://x07.io/packages.

