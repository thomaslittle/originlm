export interface ErrorLike {
  code?: unknown;
}
export function userFacingError(error: ErrorLike | null | undefined): string;
export function userFacingSessionError(payload: { code?: unknown } | null | undefined): string;
export function debugSessionError(payload?: { code?: unknown; message?: unknown }): { code: string; message: string };
