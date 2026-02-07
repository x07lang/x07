import { createHash } from "node:crypto";
import { execFile, type ExecException } from "node:child_process";
import { promises as fs } from "node:fs";
import path from "node:path";

import {
  X07_GENPACK_E_BUNDLE_SHAPE_INVALID,
  X07_GENPACK_E_CACHE_IO,
  X07_GENPACK_E_DIR_MISSING_ARTIFACT,
  X07_GENPACK_E_DIR_READ_FAILED,
  X07_GENPACK_E_GRAMMAR_BUNDLE_JSON_PARSE,
  X07_GENPACK_E_GRAMMAR_BUNDLE_VERSION_MISMATCH,
  X07_GENPACK_E_HASH_MISMATCH,
  X07_GENPACK_E_MATERIALIZE_IO,
  X07_GENPACK_E_SCHEMA_JSON_PARSE,
  X07_GENPACK_E_SCHEMA_VERSION_MISMATCH,
  X07_GENPACK_E_SEMANTIC_SUPPLEMENT_VERSION_MISMATCH,
  X07_GENPACK_E_STDOUT_NOT_UTF8,
  X07_GENPACK_E_SUBPROCESS_FAILED,
  X07_GENPACK_E_SUBPROCESS_TIMEOUT,
  X07_GENPACK_E_VARIANT_MISSING,
  X07_GENPACK_E_X07_NOT_FOUND
} from "./errorCodes";
import { GenpackError } from "./errors";
import type {
  CliSource,
  GenpackClientOptions,
  GenpackSource,
  GrammarBundle,
  GrammarVariant,
  JsonDoc,
  X07AstGenpack
} from "./types";

const EXPECTED_GRAMMAR_BUNDLE_VERSION = "x07.ast.grammar_bundle@0.1.0";
const EXPECTED_SEMANTIC_VERSION = "x07.x07ast.semantic@0.1.0";
const EXPECTED_SCHEMA_ID = "https://x07.io/spec/x07ast.schema.json";
const REQUIRED_DIR_ARTIFACTS = [
  "x07ast.schema.json",
  "x07ast.min.gbnf",
  "x07ast.pretty.gbnf",
  "x07ast.semantic.json"
] as const;

function sha256HexText(input: string): string {
  return createHash("sha256").update(input, "utf8").digest("hex");
}

function preview(text: string, limit = 160): string {
  return text.length <= limit ? text : `${text.slice(0, limit)}...`;
}

function normalizeSource(source?: GenpackSource): GenpackSource {
  if (!source) {
    return { kind: "cli" };
  }
  if (source.kind === "dir") {
    return { kind: "dir", dir: source.dir };
  }
  const cli: CliSource = { kind: "cli" };
  if (source.x07Path) {
    cli.x07Path = source.x07Path;
  }
  if (source.cwd) {
    cli.cwd = source.cwd;
  }
  if (source.env) {
    cli.env = { ...source.env };
  }
  return cli;
}

export class GenpackClient {
  private readonly source: GenpackSource;
  private readonly timeoutMs: number;
  private readonly cacheDir?: string;
  private readonly strict: boolean;

  constructor(opts: GenpackClientOptions = {}) {
    this.source = normalizeSource(opts.source);
    this.timeoutMs = opts.timeoutMs ?? 30_000;
    this.cacheDir = opts.cacheDir;
    this.strict = opts.strict ?? true;
  }

  async getX07AstSchema(): Promise<JsonDoc<Record<string, unknown>>> {
    if (this.source.kind === "dir") {
      const raw = await this.readDirText(path.join(this.source.dir, "x07ast.schema.json"));
      return this.parseSchemaDoc(raw, [path.join(this.source.dir, "x07ast.schema.json")]);
    }

    const raw = await this.runX07(["ast", "schema", "--json-schema"]);
    return this.parseSchemaDoc(raw, ["ast", "schema", "--json-schema"]);
  }

  async getX07AstGrammarBundle(): Promise<GrammarBundle> {
    if (this.source.kind === "dir") {
      return this.bundleFromDir(this.source.dir);
    }

    const raw = await this.runX07(["ast", "grammar", "--cfg"]);
    return this.parseGrammarBundle(raw, ["ast", "grammar", "--cfg"]);
  }

  async getX07AstGenpack(): Promise<X07AstGenpack> {
    const cached = await this.readCache();
    if (cached) {
      return cached;
    }

    const schema = await this.getX07AstSchema();
    const grammar = await this.getX07AstGrammarBundle();
    this.checkSchemaAlignment(schema, grammar);

    const result: X07AstGenpack = {
      toolchain: {
        x07Path: this.x07Path(),
        x07Version: await this.discoverToolchainVersion()
      },
      schema,
      grammar
    };

    await this.writeCache(result);
    return result;
  }

  async materialize(outDir: string): Promise<void> {
    try {
      await fs.mkdir(outDir, { recursive: true });
    } catch (error) {
      throw new GenpackError(X07_GENPACK_E_MATERIALIZE_IO, "failed to create output directory", {
        out_dir: outDir,
        io_error: String(error)
      });
    }

    if (this.source.kind === "dir") {
      try {
        for (const name of [...REQUIRED_DIR_ARTIFACTS, "manifest.json"] as const) {
          const src = path.join(this.source.dir, name);
          const dst = path.join(outDir, name);
          await fs.copyFile(src, dst).catch(() => undefined);
        }
      } catch (error) {
        throw new GenpackError(X07_GENPACK_E_MATERIALIZE_IO, "failed to copy artifacts", {
          out_dir: outDir,
          io_error: String(error)
        });
      }
      return;
    }

    await this.runX07(["ast", "grammar", "--cfg", "--out-dir", outDir]);
  }

  private x07Path(): string {
    if (this.source.kind !== "cli") {
      return "<dir-mode>";
    }
    return this.source.x07Path ?? process.env.X07_BIN ?? "x07";
  }

  private async runX07(args: string[]): Promise<string> {
    if (this.source.kind !== "cli") {
      throw new GenpackError(X07_GENPACK_E_SUBPROCESS_FAILED, "runX07 called in dir mode", {
        argv: args,
        exit_code: -1,
        stderr: "dir mode"
      });
    }

    const x07Path = this.x07Path();
    const argv = [x07Path, ...args];
    const cliSource = this.source as CliSource;

    const execResult = await new Promise<{ stdout: Buffer; stderr: Buffer }>((resolve, reject) => {
      execFile(
        x07Path,
        args,
        {
          cwd: cliSource.cwd,
          env: cliSource.env,
          timeout: this.timeoutMs,
          encoding: "buffer",
          maxBuffer: 16 * 1024 * 1024
        },
        (error, stdout, stderr) => {
          const outBuf = Buffer.isBuffer(stdout) ? stdout : Buffer.from(stdout ?? "", "utf8");
          const errBuf = Buffer.isBuffer(stderr) ? stderr : Buffer.from(stderr ?? "", "utf8");
          if (error) {
            const execError = error as ExecException & NodeJS.ErrnoException;
            if (execError.code === "ENOENT") {
              reject(
                new GenpackError(X07_GENPACK_E_X07_NOT_FOUND, "x07 executable was not found", {
                  x07_path: x07Path,
                  path_env: process.env.PATH ?? ""
                })
              );
              return;
            }
            if (execError.code === "ETIMEDOUT") {
              reject(
                new GenpackError(X07_GENPACK_E_SUBPROCESS_TIMEOUT, "x07 command timed out", {
                  argv,
                  timeout_s: this.timeoutMs / 1000
                })
              );
              return;
            }
            reject(
              new GenpackError(X07_GENPACK_E_SUBPROCESS_FAILED, "x07 subprocess returned non-zero", {
                argv,
                exit_code: typeof execError.code === "number" ? execError.code : -1,
                stderr: errBuf.toString("utf8")
              })
            );
            return;
          }
          resolve({ stdout: outBuf, stderr: errBuf });
        }
      );
    }).catch((error: unknown) => {
      if (error instanceof GenpackError) {
        throw error;
      }
      throw new GenpackError(X07_GENPACK_E_SUBPROCESS_FAILED, "x07 subprocess failed unexpectedly", {
        argv,
        exit_code: -1,
        stderr: String(error)
      });
    });

    try {
      return execResult.stdout.toString("utf8");
    } catch {
      throw new GenpackError(X07_GENPACK_E_STDOUT_NOT_UTF8, "x07 stdout was not UTF-8", { argv });
    }
  }

  private parseSchemaDoc(raw: string, argv: string[]): JsonDoc<Record<string, unknown>> {
    let parsed: unknown;
    try {
      parsed = JSON.parse(raw);
    } catch {
      throw new GenpackError(X07_GENPACK_E_SCHEMA_JSON_PARSE, "failed to parse schema JSON", {
        argv,
        stdout_prefix: preview(raw)
      });
    }

    if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) {
      throw new GenpackError(X07_GENPACK_E_SCHEMA_JSON_PARSE, "schema payload must be an object", {
        argv,
        stdout_prefix: preview(raw)
      });
    }
    const parsedObj = parsed as Record<string, unknown>;

    if (this.strict) {
      const schemaId = parsedObj["$id"];
      if (typeof schemaId !== "string" || schemaId !== EXPECTED_SCHEMA_ID) {
        throw new GenpackError(X07_GENPACK_E_SCHEMA_VERSION_MISMATCH, "schema $id mismatch", {
          expected: EXPECTED_SCHEMA_ID,
          actual: schemaId
        });
      }
      if (typeof parsedObj["$schema"] !== "string") {
        throw new GenpackError(X07_GENPACK_E_SCHEMA_VERSION_MISMATCH, "schema missing $schema", {
          expected: "json-schema URL",
          actual: parsedObj["$schema"]
        });
      }
    }

    return {
      raw,
      json: parsedObj,
      sha256Hex: sha256HexText(raw)
    };
  }

  private parseGrammarBundle(raw: string, argv: string[]): GrammarBundle {
    let parsed: unknown;
    try {
      parsed = JSON.parse(raw);
    } catch {
      throw new GenpackError(X07_GENPACK_E_GRAMMAR_BUNDLE_JSON_PARSE, "failed to parse grammar bundle", {
        argv,
        stdout_prefix: preview(raw)
      });
    }

    if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) {
      throw new GenpackError(X07_GENPACK_E_BUNDLE_SHAPE_INVALID, "grammar bundle payload must be an object", {
        missing_fields: ["schema_version", "variants", "semantic_supplement"]
      });
    }
    const parsedObj = parsed as Record<string, unknown>;

    const schemaVersion = parsedObj["schema_version"];
    if (typeof schemaVersion !== "string") {
      throw new GenpackError(X07_GENPACK_E_BUNDLE_SHAPE_INVALID, "bundle missing schema_version", {
        missing_fields: ["schema_version"]
      });
    }
    if (this.strict && schemaVersion !== EXPECTED_GRAMMAR_BUNDLE_VERSION) {
      throw new GenpackError(
        X07_GENPACK_E_GRAMMAR_BUNDLE_VERSION_MISMATCH,
        "grammar bundle schema_version mismatch",
        { expected: EXPECTED_GRAMMAR_BUNDLE_VERSION, actual: schemaVersion }
      );
    }

    const variantsValue = parsedObj["variants"];
    if (!Array.isArray(variantsValue)) {
      throw new GenpackError(X07_GENPACK_E_BUNDLE_SHAPE_INVALID, "variants must be an array", {
        missing_fields: ["variants"]
      });
    }

    const variants: Record<string, GrammarVariant> = {};
    for (const item of variantsValue) {
      if (typeof item !== "object" || item === null || Array.isArray(item)) {
        continue;
      }
      const name = item["name"];
      const cfg = item["cfg"];
      if ((name === "min" || name === "pretty") && typeof cfg === "string") {
        variants[name] = {
          name,
          cfg,
          sha256Hex: sha256HexText(cfg)
        };
      }
    }

    if (!variants.min) {
      throw new GenpackError(X07_GENPACK_E_VARIANT_MISSING, "required grammar variant is missing", {
        variant: "min",
        available_variants: Object.keys(variants).sort()
      });
    }

    const semantic = parsedObj["semantic_supplement"];
    if (typeof semantic !== "object" || semantic === null || Array.isArray(semantic)) {
      throw new GenpackError(
        X07_GENPACK_E_BUNDLE_SHAPE_INVALID,
        "semantic_supplement must be a JSON object",
        {
          missing_fields: ["semantic_supplement"]
        }
      );
    }

    const semanticRecord = semantic as Record<string, unknown>;
    const semanticVersion = semanticRecord["schema_version"];
    if (this.strict && semanticVersion !== EXPECTED_SEMANTIC_VERSION) {
      throw new GenpackError(
        X07_GENPACK_E_SEMANTIC_SUPPLEMENT_VERSION_MISMATCH,
        "semantic supplement schema_version mismatch",
        { expected: EXPECTED_SEMANTIC_VERSION, actual: semanticVersion }
      );
    }

    const semanticRaw = JSON.stringify(semanticRecord);

    const x07astSchemaVersionValue = parsedObj["x07ast_schema_version"];
    const x07astSchemaVersion = typeof x07astSchemaVersionValue === "string" ? x07astSchemaVersionValue : undefined;

    return {
      raw,
      json: parsedObj,
      schemaVersion,
      x07astSchemaVersion,
      variants,
      semanticSupplement: {
        raw: semanticRaw,
        json: semanticRecord,
        sha256Hex: sha256HexText(semanticRaw)
      }
    };
  }

  private async bundleFromDir(dir: string): Promise<GrammarBundle> {
    const missing: string[] = [];
    for (const name of REQUIRED_DIR_ARTIFACTS) {
      const p = path.join(dir, name);
      try {
        await fs.access(p);
      } catch {
        missing.push(name);
      }
    }
    if (missing.length > 0) {
      throw new GenpackError(X07_GENPACK_E_DIR_MISSING_ARTIFACT, "directory source missing artifacts", {
        dir,
        missing_paths: missing
      });
    }

    const schemaRaw = await this.readDirText(path.join(dir, "x07ast.schema.json"));
    const schemaDoc = this.parseSchemaDoc(schemaRaw, [path.join(dir, "x07ast.schema.json")]);
    const minCfg = await this.readDirText(path.join(dir, "x07ast.min.gbnf"));
    const prettyCfg = await this.readDirText(path.join(dir, "x07ast.pretty.gbnf"));
    const semanticRaw = await this.readDirText(path.join(dir, "x07ast.semantic.json"));

    let semanticObj: unknown;
    try {
      semanticObj = JSON.parse(semanticRaw);
    } catch {
      throw new GenpackError(X07_GENPACK_E_GRAMMAR_BUNDLE_JSON_PARSE, "failed to parse semantic supplement", {
        argv: [path.join(dir, "x07ast.semantic.json")],
        stdout_prefix: preview(semanticRaw)
      });
    }
    if (typeof semanticObj !== "object" || semanticObj === null || Array.isArray(semanticObj)) {
      throw new GenpackError(X07_GENPACK_E_BUNDLE_SHAPE_INVALID, "semantic supplement must be an object", {
        missing_fields: ["semantic_supplement"]
      });
    }
    const semanticRecord = semanticObj as Record<string, unknown>;
    const semanticVersion = semanticRecord["schema_version"];
    if (this.strict && semanticVersion !== EXPECTED_SEMANTIC_VERSION) {
      throw new GenpackError(
        X07_GENPACK_E_SEMANTIC_SUPPLEMENT_VERSION_MISMATCH,
        "semantic supplement schema_version mismatch",
        { expected: EXPECTED_SEMANTIC_VERSION, actual: semanticVersion }
      );
    }

    const props = schemaDoc.json["properties"];
    let x07astSchemaVersion: string | undefined;
    if (typeof props === "object" && props && !Array.isArray(props)) {
      const schemaVersionProp = (props as Record<string, unknown>)["schema_version"];
      if (typeof schemaVersionProp === "object" && schemaVersionProp && !Array.isArray(schemaVersionProp)) {
        const constValue = (schemaVersionProp as Record<string, unknown>)["const"];
        if (typeof constValue === "string") {
          x07astSchemaVersion = constValue;
        }
      }
    }

    await this.verifyManifestHashes(dir, {
      "x07ast.schema.json": sha256HexText(schemaRaw),
      "x07ast.min.gbnf": sha256HexText(minCfg),
      "x07ast.pretty.gbnf": sha256HexText(prettyCfg),
      "x07ast.semantic.json": sha256HexText(semanticRaw)
    });

    const bundleObject = {
      schema_version: EXPECTED_GRAMMAR_BUNDLE_VERSION,
      x07ast_schema_version: x07astSchemaVersion,
      format: "gbnf_v1",
      variants: [
        { name: "min", cfg: minCfg },
        { name: "pretty", cfg: prettyCfg }
      ],
      semantic_supplement: semanticRecord,
      sha256: {
        min_cfg: sha256HexText(minCfg),
        pretty_cfg: sha256HexText(prettyCfg),
        semantic_supplement: sha256HexText(semanticRaw)
      }
    };

    return this.parseGrammarBundle(JSON.stringify(bundleObject), [dir]);
  }

  private async readDirText(filePath: string): Promise<string> {
    try {
      return await fs.readFile(filePath, "utf8");
    } catch (error) {
      throw new GenpackError(X07_GENPACK_E_DIR_READ_FAILED, "failed to read artifact from dir source", {
        path: filePath,
        io_error: String(error)
      });
    }
  }

  private async verifyManifestHashes(dir: string, actual: Record<string, string>): Promise<void> {
    const manifestPath = path.join(dir, "manifest.json");
    let manifestRaw: string;
    try {
      manifestRaw = await fs.readFile(manifestPath, "utf8");
    } catch {
      return;
    }

    let manifest: unknown;
    try {
      manifest = JSON.parse(manifestRaw);
    } catch {
      return;
    }

    if (typeof manifest !== "object" || manifest === null || Array.isArray(manifest)) {
      return;
    }
    const manifestRecord = manifest as Record<string, unknown>;
    const artifacts = manifestRecord["artifacts"];
    if (!Array.isArray(artifacts)) {
      return;
    }

    for (const artifact of artifacts) {
      if (typeof artifact !== "object" || artifact === null || Array.isArray(artifact)) {
        continue;
      }
      const name = artifact["name"];
      const expected = artifact["sha256"];
      if (typeof name !== "string" || typeof expected !== "string") {
        continue;
      }
      const actualValue = actual[name];
      if (!actualValue) {
        continue;
      }
      if (actualValue !== expected) {
        throw new GenpackError(X07_GENPACK_E_HASH_MISMATCH, "artifact hash mismatch against manifest", {
          path: path.join(dir, name),
          expected_sha256: expected,
          actual_sha256: actualValue
        });
      }
    }
  }

  private checkSchemaAlignment(schema: JsonDoc<Record<string, unknown>>, grammar: GrammarBundle): void {
    if (!grammar.x07astSchemaVersion) {
      return;
    }
    const props = schema.json["properties"];
    if (typeof props !== "object" || props === null || Array.isArray(props)) {
      return;
    }
    const schemaVersionProp = (props as Record<string, unknown>)["schema_version"];
    if (typeof schemaVersionProp !== "object" || schemaVersionProp === null || Array.isArray(schemaVersionProp)) {
      return;
    }
    const constValue = (schemaVersionProp as Record<string, unknown>)["const"];
    if (typeof constValue !== "string") {
      return;
    }

    if (this.strict && constValue !== grammar.x07astSchemaVersion) {
      throw new GenpackError(
        X07_GENPACK_E_SCHEMA_VERSION_MISMATCH,
        "x07ast schema version mismatch between schema and grammar bundle",
        { expected: constValue, actual: grammar.x07astSchemaVersion }
      );
    }
  }

  private async discoverToolchainVersion(): Promise<string | undefined> {
    if (this.source.kind !== "cli") {
      return undefined;
    }
    const cliSource = this.source as CliSource;

    const x07Path = this.x07Path();
    const raw = await new Promise<string | undefined>((resolve) => {
      execFile(
        x07Path,
        ["--version"],
        {
          cwd: cliSource.cwd,
          env: cliSource.env,
          timeout: Math.min(this.timeoutMs, 5000),
          encoding: "utf8"
        },
        (error, stdout) => {
          if (error) {
            resolve(undefined);
            return;
          }
          resolve((stdout ?? "").toString().trim() || undefined);
        }
      );
    });

    if (!raw) {
      return undefined;
    }
    if (raw.includes(" ")) {
      const parts = raw.split(" ");
      return parts[1]?.trim() || undefined;
    }
    return raw;
  }

  private cacheKey(version: string | undefined): string {
    const raw = `${version ?? "unknown"}__${EXPECTED_GRAMMAR_BUNDLE_VERSION}__${EXPECTED_SEMANTIC_VERSION}`;
    return raw.replace(/[^A-Za-z0-9._-]/g, "_");
  }

  private async readCache(): Promise<X07AstGenpack | undefined> {
    if (!this.cacheDir) {
      return undefined;
    }

    const version = await this.discoverToolchainVersion();
    const cachePath = path.join(this.cacheDir, this.cacheKey(version));
    const schemaPath = path.join(cachePath, "schema.json");
    const grammarPath = path.join(cachePath, "grammar_bundle.json");

    try {
      await fs.access(schemaPath);
      await fs.access(grammarPath);
    } catch {
      return undefined;
    }

    try {
      const schemaRaw = await fs.readFile(schemaPath, "utf8");
      const grammarRaw = await fs.readFile(grammarPath, "utf8");
      const schema = this.parseSchemaDoc(schemaRaw, [schemaPath]);
      const grammar = this.parseGrammarBundle(grammarRaw, [grammarPath]);
      this.checkSchemaAlignment(schema, grammar);
      return {
        toolchain: {
          x07Path: this.x07Path(),
          x07Version: version
        },
        schema,
        grammar
      };
    } catch (error) {
      if (error instanceof GenpackError) {
        throw error;
      }
      throw new GenpackError(X07_GENPACK_E_CACHE_IO, "failed to read cache entry", {
        cache_dir: cachePath,
        io_error: String(error)
      });
    }
  }

  private async writeCache(genpack: X07AstGenpack): Promise<void> {
    if (!this.cacheDir) {
      return;
    }

    const version = await this.discoverToolchainVersion();
    const cachePath = path.join(this.cacheDir, this.cacheKey(version));
    try {
      await fs.mkdir(cachePath, { recursive: true });
      await fs.writeFile(path.join(cachePath, "schema.json"), genpack.schema.raw, "utf8");
      await fs.writeFile(path.join(cachePath, "grammar_bundle.json"), genpack.grammar.raw, "utf8");
    } catch (error) {
      throw new GenpackError(X07_GENPACK_E_CACHE_IO, "failed to write cache entry", {
        cache_dir: cachePath,
        io_error: String(error)
      });
    }
  }
}
