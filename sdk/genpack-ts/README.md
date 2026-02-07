# @x07/genpack

`@x07/genpack` is a tiny SDK for retrieving x07AST generation-pack artifacts from the `x07` CLI.

The client exposes:

- `getX07AstSchema()`
- `getX07AstGrammarBundle()`
- `getX07AstGenpack()`

It supports CLI mode and directory mode, plus strict version checks and stable error codes.

## Quick start

```ts
import { GenpackClient } from "@x07/genpack";

const client = new GenpackClient();
const genpack = await client.getX07AstGenpack();

console.log(genpack.schema.sha256Hex);
console.log(genpack.grammar.variants.min.cfg);
```
