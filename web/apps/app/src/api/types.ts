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
  /** Whether this build can render PDFs. False means offer text export only. */
  pdfExport: boolean;
  /**
   * The version of a newer release, or null.
   *
   * Filled in by a background check on the server, so it is null on the first
   * status of a run even when an update does exist.
   */
  updateAvailable: string | null;
}

/**
 * Which of `year` and `month` a report route accepts.
 *
 * Reported by the response rather than hardcoded per screen, so date controls
 * are built from what the server says it supports.
 */
export type DateGranularity = 'monthAndYear' | 'yearOnly' | 'none';

/** Every report response is wrapped with the date granularity it supports. */
export interface ReportEnvelope<T> {
  granularity: DateGranularity;
  report: T;
}

/** A category and its total, in `PnlReport`. */
export interface PnlItem {
  name: string;
  total: number;
}

/** `GET /api/reports/pnl` */
export interface PnlReport {
  income: PnlItem[];
  expenses: PnlItem[];
  totalIncome: number;
  /** Negative — expenses are stored as negative amounts. */
  totalExpenses: number;
  net: number;
}

/** One account's cash position, in `BalanceReport`. */
export interface AccountBalance {
  name: string;
  accountType: string;
  balance: number;
}

/** `GET /api/reports/balance` */
export interface BalanceReport {
  accounts: AccountBalance[];
  total: number;
  ytdNetIncome: number;
}

/** One month of cash movement, in `CashflowReport`. */
export interface CashflowMonth {
  /** `YYYY-MM`. */
  month: string;
  inflows: number;
  /** Negative — the sum of that month's negative amounts. */
  outflows: number;
  net: number;
  runningBalance: number;
}

/**
 * `GET /api/reports/cashflow`
 *
 * Only months with transactions appear; there are no zero-filled gaps.
 */
export interface CashflowReport {
  months: CashflowMonth[];
}

/** `GET /api/reports/flagged` — one transaction awaiting review. */
export interface FlaggedTransaction {
  id: number;
  date: string;
  description: string;
  amount: number;
  accountName: string;
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
