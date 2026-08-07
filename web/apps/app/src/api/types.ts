/**
 * Hand-written TypeScript mirrors of the server's serde structs.
 *
 * The Rust side is `#[serde(rename_all = "camelCase")]` throughout, so field
 * names here match the wire exactly. `docs/api.md` is the contract; every API
 * task keeps this file in step with it in the same commit.
 *
 * Today this covers the endpoints that answer while the database is locked and
 * the settings surface. Read, write, import and export types arrive with their
 * own tasks.
 */

/** `GET /api/ping` */
export interface PingResponse {
  ok: boolean;
  version: string;
}

/** `GET /api/status` */
export interface StatusResponse {
  initialized: boolean;
  encrypted: boolean;
  locked: boolean;
  companyName: string | null;
  version: string;
  dataDir: string;
}

/** `POST /api/unlock` */
export interface UnlockRequest {
  password: string;
}

export interface UnlockResponse {
  locked: boolean;
}

/** `GET /api/settings/app`, and the body of a successful `PUT`. */
export interface AppSettings {
  userName: string;
  updateCheck: boolean;
  lastUpdateCheck: string | null;
}

/**
 * `PUT /api/settings/app`. Only the update check is web-editable — the user
 * name comes from onboarding and the timestamp is the updater's bookkeeping.
 */
export interface UpdateAppSettingsRequest {
  updateCheck: boolean;
}

/** `PUT /api/settings/company-name` */
export interface CompanyNameRequest {
  name: string;
}

export interface CompanyNameResponse {
  companyName: string;
}

/** `POST /api/settings/data-dir` — answers with the new database's status. */
export interface DataDirRequest {
  path: string;
}

/** `POST /api/settings/password/set` */
export interface SetPasswordRequest {
  newPassword: string;
}

/** `POST /api/settings/password/change` */
export interface ChangePasswordRequest {
  currentPassword: string;
  newPassword: string;
}

/** `POST /api/settings/password/remove` */
export interface RemovePasswordRequest {
  currentPassword: string;
}

/** What the three password routes answer with. */
export interface PasswordStateResponse {
  encrypted: boolean;
  locked: boolean;
}

/**
 * Every error code the server can currently emit. New codes appear as the API
 * grows (31.7 adds `payload_too_large`), so the client normalizes anything it
 * does not recognize to `unknown` rather than lying about the type — the raw
 * string stays available on `ApiError.rawCode`.
 */
export const API_ERROR_CODES = [
  'bad_request',
  'unauthorized',
  'invalid_password',
  'forbidden',
  'not_found',
  'conflict',
  'locked',
  'internal',
  'feature_disabled',
] as const;

export type KnownApiErrorCode = (typeof API_ERROR_CODES)[number];
export type ApiErrorCode = KnownApiErrorCode | 'unknown';

export function isKnownApiErrorCode(value: string): value is KnownApiErrorCode {
  return (API_ERROR_CODES as readonly string[]).includes(value);
}

/** The `{"error": {...}}` envelope every failing response carries. */
export interface ApiErrorEnvelope {
  error: {
    code: string;
    message: string;
    details?: unknown;
  };
}

/** `details` on a 401 `invalid_password`. */
export interface InvalidPasswordDetails {
  attemptsRemaining: number;
  retryAfterMs: number;
}
