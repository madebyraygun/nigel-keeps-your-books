/**
 * Hand-written TypeScript mirrors of the server's serde structs.
 *
 * The Rust side is `#[serde(rename_all = "camelCase")]` throughout, so field
 * names here match the wire exactly. `docs/api.md` is the contract; every API
 * task keeps this file in step with it in the same commit.
 *
 * Export types arrive with their own task; everything else the SPA reaches for
 * is here.
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

/** One transaction in the register, and what a transaction edit answers with. */
export interface RegisterRow {
  id: number;
  /** `YYYY-MM-DD`. */
  date: string;
  description: string;
  /** Negative is an expense, positive is income. */
  amount: number;
  category: string | null;
  categoryId: number | null;
  vendor: string | null;
  accountName: string;
  isFlagged: boolean;
}

/**
 * `GET /api/reports/register`
 *
 * Rows come back date ascending, and `total` is the net of the whole result
 * set — not of whatever a client shows after a client-side search.
 */
export interface RegisterReport {
  rows: RegisterRow[];
  total: number;
}

/** `GET /api/accounts` */
export interface Account {
  id: number;
  name: string;
  accountType: string;
  institution: string | null;
  lastFour: string | null;
}

/** `GET /api/categories` — the active chart of accounts. */
export interface CategoryRow {
  id: number;
  name: string;
  categoryType: string;
  taxLine: string | null;
  formLine: string | null;
}

/**
 * `PATCH /api/transactions/:id` — a true partial update.
 *
 * An omitted field is left alone. `vendor: null` clears the vendor; there is
 * no `categoryId: null`, because uncategorizing is what the review undo route
 * is for. `flag` is a state rather than a toggle, so retrying a request whose
 * response was lost cannot land the opposite of what was asked for.
 */
export interface TransactionPatch {
  categoryId?: number;
  vendor?: string | null;
  flag?: boolean;
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
 * `GET /api/review/queue` — one transaction waiting to be reviewed, oldest
 * first.
 *
 * Mirrors `reviewer::FlaggedTxn`, which is a different struct from
 * `reports::FlaggedTransaction` above even though the fields line up today:
 * that one is the flagged *report*, this one is the review *queue*. They are
 * kept apart because the two routes are free to diverge, and folding them into
 * one interface would make the next field either route adds a lie about the
 * other.
 */
export interface FlaggedTxn {
  id: number;
  /** `YYYY-MM-DD`. */
  date: string;
  description: string;
  amount: number;
  accountName: string;
}

/**
 * `POST /api/review/:id/apply`
 *
 * `createRule` without a `rulePattern` is a 400 rather than a quietly rule-less
 * success. There is no match type: the data layer writes `contains`, which is
 * also the only kind of rule the interactive reviewer has ever made.
 */
export interface ReviewApplyRequest {
  categoryId: number;
  vendor?: string;
  createRule?: boolean;
  rulePattern?: string;
}

export interface ReviewApplyResponse {
  transactionId: number;
  /** The rule this decision created; null when it created none. */
  ruleId: number | null;
}

/**
 * `POST /api/review/:id/undo`
 *
 * The body is required but `{}` is valid, and means "just put the transaction
 * back". Passing the `ruleId` an apply answered with deletes that rule too —
 * which is what makes the review screen's Back button leave no trace.
 */
export interface ReviewUndoRequest {
  ruleId?: number;
}

/** The match types the categorizer understands. */
export type RuleMatchType = 'contains' | 'starts_with' | 'regex';

/** `POST /api/rules/test` — a dry run; nothing is written. */
export interface RuleTestRequest {
  pattern: string;
  matchType?: RuleMatchType;
}

/** One description a pattern would match, and how many transactions carry it. */
export interface RuleTestMatch {
  description: string;
  count: number;
}

/**
 * What a pattern would match today. Identical descriptions collapse into one
 * entry with a count, busiest first; matching nothing is a 200 with `total: 0`.
 */
export interface RuleTestResult {
  total: number;
  matches: RuleTestMatch[];
}

/**
 * Every error code the server can emit. The client normalizes anything it does
 * not recognize to `unknown` rather than lying about the type — the raw string
 * stays available on `ApiError.rawCode`, so a code the server learns before
 * this list does still reaches a caller that wants to branch on it.
 */
export const API_ERROR_CODES = [
  'bad_request',
  'unauthorized',
  'invalid_password',
  'forbidden',
  'not_found',
  'conflict',
  'locked',
  'payload_too_large',
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

// -- imports ----------------------------------------------------------------

/** `POST /api/imports/upload` — the file is parked, nothing is parsed yet. */
export interface UploadResponse {
  uploadId: string;
  /** The name as stored, reduced to safe characters. */
  filename: string;
  size: number;
}

/** Column positions for a CSV no built-in importer can read. */
export interface GenericCsvConfig {
  dateCol: number;
  descCol: number;
  amountCol: number;
  /** A chrono format string, e.g. `%m/%d/%Y`. */
  dateFormat: string;
}

/** One built-in importer. Gusto is absent from builds without its feature. */
export interface ImporterFormat {
  key: string;
  name: string;
  accountTypes: string[];
}

/** A saved column mapping, addressed by the name `format` takes. */
export interface CsvProfile {
  name: string;
  config: GenericCsvConfig;
}

/** One parsed row, as the preview reports it. Mirrors `ParsedRow`. */
export interface ImportSampleRow {
  date: string;
  description: string;
  amount: number;
}

/**
 * The body preview and confirm share.
 *
 * `format` and `mapping` are mutually exclusive — sending both is a 400 rather
 * than a guess. Neither means "detect from the account type and the file".
 */
export interface ImportRequest {
  uploadId: string;
  account: string;
  format?: string;
  mapping?: GenericCsvConfig;
}

export interface ConfirmImportRequest extends ImportRequest {
  /** Remembers `mapping` under this name; requires one. */
  saveProfile?: string;
}

/**
 * `POST /api/imports/preview` — what an import *would* do, having written
 * nothing at all.
 *
 * `format` is what actually resolved: a built-in key, a profile name, or
 * `GENERIC_FORMAT`. It is null only for a duplicate file, which is answered
 * before a format is resolved.
 */
export interface ImportPreview {
  imported: number;
  skipped: number;
  malformed: number;
  duplicateFile: boolean;
  sample: ImportSampleRow[];
  format: string | null;
  /** Always null from preview; the `imports` row id after a confirm. */
  importId: number | null;
}

/** `POST /api/imports/confirm` — the preview shape plus what the write did. */
export interface ImportConfirmation extends ImportPreview {
  categorized: number;
  /** The whole ledger's flagged count, not just this import's. */
  stillFlagged: number;
  /** Absolute path of the pre-import snapshot. */
  snapshot: string;
}

/** What `format` reads back as when an inline `mapping` was used. */
export const GENERIC_FORMAT = 'generic';

/** `details.reason` distinguishing an expired upload from any other 404. */
export const UPLOAD_NOT_FOUND = 'upload_not_found';
