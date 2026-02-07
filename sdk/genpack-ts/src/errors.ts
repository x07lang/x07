import type { GenpackErrorCode } from "./errorCodes";

export class GenpackError extends Error {
  public readonly code: GenpackErrorCode;
  public readonly data: Record<string, unknown>;

  constructor(code: GenpackErrorCode, message: string, data?: Record<string, unknown>) {
    super(`${code}: ${message}`);
    this.code = code;
    this.data = data ?? {};
    this.name = "GenpackError";
  }
}
