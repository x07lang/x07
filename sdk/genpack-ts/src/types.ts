export interface ToolchainInfo {
  x07Path: string;
  x07Version?: string;
}

export interface JsonDoc<T> {
  raw: string;
  json: T;
  sha256Hex: string;
}

export type GrammarVariantName = "min" | "pretty";

export interface GrammarVariant {
  name: GrammarVariantName;
  cfg: string;
  sha256Hex: string;
}

export interface GrammarBundle {
  raw: string;
  json: Record<string, unknown>;
  schemaVersion: string;
  x07astSchemaVersion?: string;
  variants: Record<string, GrammarVariant>;
  semanticSupplement: JsonDoc<Record<string, unknown>>;
}

export interface X07AstGenpack {
  toolchain: ToolchainInfo;
  schema: JsonDoc<Record<string, unknown>>;
  grammar: GrammarBundle;
}

export type CliSource = {
  kind: "cli";
  x07Path?: string;
  cwd?: string;
  env?: NodeJS.ProcessEnv;
};

export type DirSource = {
  kind: "dir";
  dir: string;
};

export type GenpackSource = CliSource | DirSource;

export interface GenpackClientOptions {
  source?: GenpackSource;
  timeoutMs?: number;
  cacheDir?: string;
  strict?: boolean;
}
