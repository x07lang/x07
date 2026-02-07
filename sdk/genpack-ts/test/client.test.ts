import { execFileSync } from "node:child_process";
import { promises as fs } from "node:fs";
import path from "node:path";

import { describe, expect, test } from "vitest";

import { GenpackClient } from "../src/client";
import {
  X07_GENPACK_E_HASH_MISMATCH,
  X07_GENPACK_E_SCHEMA_JSON_PARSE,
  X07_GENPACK_E_SEMANTIC_SUPPLEMENT_VERSION_MISMATCH
} from "../src/errorCodes";

function x07Bin(): string {
  return process.env.X07_BIN ?? "x07";
}

function hasX07(): boolean {
  try {
    execFileSync(x07Bin(), ["--version"], { stdio: "ignore" });
    return true;
  } catch {
    return false;
  }
}

async function freshDir(name: string): Promise<string> {
  const root = path.join("target", "sdk-genpack-ts-tests", name);
  await fs.rm(root, { recursive: true, force: true });
  await fs.mkdir(root, { recursive: true });
  return root;
}

const describeIfX07 = hasX07() ? describe : describe.skip;

describeIfX07("genpack client", () => {
  test("cli source parses schema and bundle", async () => {
    const client = new GenpackClient({ source: { kind: "cli", x07Path: x07Bin() } });

    const schema = await client.getX07AstSchema();
    const bundle = await client.getX07AstGrammarBundle();

    expect(schema.json.$id).toBe("https://x07.io/spec/x07ast.schema.json");
    expect(bundle.schemaVersion).toBe("x07.ast.grammar_bundle@0.1.0");
    expect(bundle.variants.min).toBeDefined();
    expect(bundle.variants.min?.cfg.startsWith("root ::= ")).toBe(true);
  });

  test("dir mode matches cli mode", async () => {
    const outDir = await freshDir("dir_mode_matches");

    const cliClient = new GenpackClient({ source: { kind: "cli", x07Path: x07Bin() } });
    await cliClient.materialize(outDir);

    const dirClient = new GenpackClient({ source: { kind: "dir", dir: outDir } });
    const cliPack = await cliClient.getX07AstGenpack();
    const dirPack = await dirClient.getX07AstGenpack();

    expect(cliPack.schema.sha256Hex).toBe(dirPack.schema.sha256Hex);
    expect(cliPack.grammar.variants.min).toBeDefined();
    expect(dirPack.grammar.variants.min).toBeDefined();
    expect(cliPack.grammar.variants.min?.sha256Hex).toBe(dirPack.grammar.variants.min?.sha256Hex);
    expect(cliPack.grammar.semanticSupplement.sha256Hex).toBe(dirPack.grammar.semanticSupplement.sha256Hex);
  });

  test("dir mode rejects semantic version mismatch", async () => {
    const outDir = await freshDir("semantic_version_mismatch");

    const cliClient = new GenpackClient({ source: { kind: "cli", x07Path: x07Bin() } });
    await cliClient.materialize(outDir);

    const semanticPath = path.join(outDir, "x07ast.semantic.json");
    const semantic = JSON.parse(await fs.readFile(semanticPath, "utf8")) as Record<string, unknown>;
    semantic.schema_version = "x07.x07ast.semantic@999.0.0";
    await fs.writeFile(semanticPath, JSON.stringify(semantic), "utf8");

    const dirClient = new GenpackClient({ source: { kind: "dir", dir: outDir }, strict: true });
    await expect(dirClient.getX07AstGrammarBundle()).rejects.toMatchObject({
      code: X07_GENPACK_E_SEMANTIC_SUPPLEMENT_VERSION_MISMATCH
    });
  });

  test("dir mode rejects invalid schema JSON", async () => {
    const outDir = await freshDir("invalid_schema_json");

    const cliClient = new GenpackClient({ source: { kind: "cli", x07Path: x07Bin() } });
    await cliClient.materialize(outDir);
    await fs.writeFile(path.join(outDir, "x07ast.schema.json"), "{broken", "utf8");

    const dirClient = new GenpackClient({ source: { kind: "dir", dir: outDir }, strict: true });
    await expect(dirClient.getX07AstSchema()).rejects.toMatchObject({
      code: X07_GENPACK_E_SCHEMA_JSON_PARSE
    });
  });

  test("dir mode rejects manifest hash mismatch", async () => {
    const outDir = await freshDir("manifest_hash_mismatch");

    const cliClient = new GenpackClient({ source: { kind: "cli", x07Path: x07Bin() } });
    await cliClient.materialize(outDir);

    const manifestPath = path.join(outDir, "manifest.json");
    const manifest = JSON.parse(await fs.readFile(manifestPath, "utf8")) as Record<string, unknown>;
    const artifacts = (manifest.artifacts as Array<Record<string, unknown>> | undefined) ?? [];
    for (const artifact of artifacts) {
      if (artifact.name === "x07ast.min.gbnf") {
        artifact.sha256 = "0".repeat(64);
      }
    }
    await fs.writeFile(manifestPath, JSON.stringify(manifest), "utf8");

    const dirClient = new GenpackClient({ source: { kind: "dir", dir: outDir }, strict: true });
    await expect(dirClient.getX07AstGrammarBundle()).rejects.toMatchObject({
      code: X07_GENPACK_E_HASH_MISMATCH
    });
  });
});
