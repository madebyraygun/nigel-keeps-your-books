import { signal, type Signal } from '../mixins/signal-watcher.js';
import {
  isKnownApiErrorCode,
  UPLOAD_NOT_FOUND,
  type Account,
  type AccountPatch,
  type ApiErrorCode,
  type ApiErrorEnvelope,
  type AppSettings,
  type BalanceReport,
  type CashflowReport,
  type CategoryRow,
  type ChangePasswordRequest,
  type CompanyNameResponse,
  type ConfirmImportRequest,
  type CategoryPatch,
  type CsvProfile,
  type Deleted,
  type ExpenseBreakdown,
  type ExportFormat,
  type ExportParams,
  type FlaggedTransaction,
  type FlaggedTxn,
  type ImportConfirmation,
  type ImporterFormat,
  type ImportListItem,
  type ImportPreview,
  type ImportRequest,
  type InvalidPasswordDetails,
  type K1PrepReport,
  type NewAccountRequest,
  type NewCategoryRequest,
  type NewRuleRequest,
  type PasswordStateResponse,
  type PingResponse,
  type PnlReport,
  type ReconcileRequest,
  type ReconcileResult,
  type ReconciliationRecord,
  type RegisterReport,
  type RegisterRow,
  type RemovePasswordRequest,
  type ReportEnvelope,
  type ReportSlug,
  type ReviewApplyRequest,
  type ReviewApplyResponse,
  type ReviewUndoRequest,
  type RulePatch,
  type RuleRow,
  type RuleTestRequest,
  type RuleTestResult,
  type SetPasswordRequest,
  type StatusResponse,
  type TaxSummary,
  type TransactionPatch,
  type UndoneImport,
  type UnlockResponse,
  type UpdateAppSettingsRequest,
  type UploadResponse,
} from './types.js';

/**
 * Transport-level state, kept here because it belongs to the transport rather
 * than to any one screen.
 *
 * `appLocked` — a 423 was seen, so the database needs a password.
 * `appUnauthorized` — the session cookie is missing or stale. The SPA cannot
 * mint a token, so all it can do is tell the user to reopen the URL that
 * `nigel serve` printed.
 */
export const appLocked: Signal.State<boolean> = signal(false);
export const appUnauthorized: Signal.State<boolean> = signal(false);

/** Status codes the server uses, for responses that carry no envelope. */
const STATUS_CODES: Record<number, ApiErrorCode> = {
  400: 'bad_request',
  401: 'unauthorized',
  403: 'forbidden',
  404: 'not_found',
  409: 'conflict',
  413: 'payload_too_large',
  423: 'locked',
  500: 'internal',
  501: 'feature_disabled',
};

/** A failed API call, normalized from the error envelope. */
export class ApiError extends Error {
  readonly code: ApiErrorCode;
  /** The code exactly as the server sent it, including ones we don't know. */
  readonly rawCode: string;
  /** 0 when the request never reached the server. */
  readonly status: number;
  readonly details?: unknown;

  constructor(init: {
    code: ApiErrorCode;
    rawCode: string;
    message: string;
    status: number;
    details?: unknown;
  }) {
    super(init.message);
    this.name = 'ApiError';
    this.code = init.code;
    this.rawCode = init.rawCode;
    this.status = init.status;
    this.details = init.details;
  }

  get isLocked(): boolean {
    return this.status === 423;
  }

  /**
   * A stale session — deliberately *not* true for a wrong unlock password,
   * which is also a 401 but is the unlock form's business, not a sign that the
   * session went away.
   */
  get isUnauthorized(): boolean {
    return this.status === 401 && this.code !== 'invalid_password';
  }

  /**
   * An upload that has expired or was never there, as opposed to any other
   * 404. Worth distinguishing because it is the one 404 a client can fix by
   * itself: send the bytes again and carry on.
   */
  get isUploadExpired(): boolean {
    if (this.status !== 404) return false;
    const details = this.details as { reason?: unknown } | undefined;
    return details?.reason === UPLOAD_NOT_FOUND;
  }

  invalidPasswordDetails(): InvalidPasswordDetails | null {
    if (this.code !== 'invalid_password') return null;
    const d = this.details as Partial<InvalidPasswordDetails> | undefined;
    if (typeof d?.attemptsRemaining !== 'number' || typeof d?.retryAfterMs !== 'number') {
      return null;
    }
    return { attemptsRemaining: d.attemptsRemaining, retryAfterMs: d.retryAfterMs };
  }
}

/**
 * The whole surface the app is allowed to reach the server through.
 *
 * One method per endpoint, named after it. There is deliberately no generic
 * `request()` here: exposing one would let screens hand-roll URLs, and a Tauri
 * or remote-server implementation of this interface has no URLs to give them.
 * Routes that take more than one parameter take a single typed options object,
 * so adding parameters stays a non-breaking change.
 */
/** Date parameters a `monthAndYear` report accepts. `from` and `to` come as a pair. */
export interface ReportDateParams {
  year?: number;
  /** `YYYY-MM`. */
  month?: string;
  /** `YYYY-MM-DD`, only valid together with `to`. */
  from?: string;
  /** `YYYY-MM-DD`, only valid together with `from`. */
  to?: string;
}

/** Cash flow takes no `from`/`to` — it is grouped by month either way. */
export type CashflowParams = Pick<ReportDateParams, 'year' | 'month'>;

/** The register is the one report that also filters by account, by name. */
export interface RegisterParams extends ReportDateParams {
  account?: string;
}

/** Expenses is grouped by month, so it takes no date range. */
export type ExpenseParams = Pick<ReportDateParams, 'year' | 'month'>;

/** Tax and the K-1 worksheet are annual documents. */
export type YearParams = Pick<ReportDateParams, 'year'>;

/** Reconciliation history, optionally narrowed to one account by name. */
export interface ReconciliationParams {
  account?: string;
}

export interface ApiClient {
  ping(): Promise<PingResponse>;
  getStatus(): Promise<StatusResponse>;
  unlock(password: string): Promise<UnlockResponse>;

  getPnl(params?: ReportDateParams): Promise<ReportEnvelope<PnlReport>>;
  getBalance(): Promise<ReportEnvelope<BalanceReport>>;
  getCashflow(params?: CashflowParams): Promise<ReportEnvelope<CashflowReport>>;
  getFlagged(): Promise<ReportEnvelope<FlaggedTransaction[]>>;
  getRegister(params?: RegisterParams): Promise<ReportEnvelope<RegisterReport>>;
  getExpenses(params?: ExpenseParams): Promise<ReportEnvelope<ExpenseBreakdown>>;
  getTax(params?: YearParams): Promise<ReportEnvelope<TaxSummary>>;
  getK1(params?: YearParams): Promise<ReportEnvelope<K1PrepReport>>;

  /**
   * Where a report's export lives, for a plain download link.
   *
   * This returns a URL instead of bytes because the browser is better at
   * downloading a file than the app is: it streams, it names the file from
   * `Content-Disposition`, and it never holds a PDF in memory. But the address
   * is still built here rather than in a screen — a screen that spells its own
   * endpoint is a screen a Tauri or remote client cannot host, which is the
   * whole reason this interface exists.
   */
  exportUrl(report: ReportSlug, format: ExportFormat, params?: ExportParams): string;

  getAccounts(): Promise<Account[]>;
  getCategories(): Promise<CategoryRow[]>;
  /** Active rules, priority descending — the order the categorizer applies them. */
  getRules(): Promise<RuleRow[]>;
  /** Partial update; answers with the row as it now stands. */
  patchTransaction(id: number, changes: TransactionPatch): Promise<RegisterRow>;

  createAccount(input: NewAccountRequest): Promise<Account>;
  /** The only edit an account has: `PATCH /api/accounts/:id` takes a name. */
  renameAccount(id: number, input: AccountPatch): Promise<Account>;
  /** Hard delete, and it takes the account's reconciliations with it. */
  deleteAccount(id: number): Promise<Deleted>;

  createCategory(input: NewCategoryRequest): Promise<CategoryRow>;
  updateCategory(id: number, input: CategoryPatch): Promise<CategoryRow>;
  /** Soft delete: the row stays on the transactions that already use it. */
  deleteCategory(id: number): Promise<Deleted>;

  createRule(input: NewRuleRequest): Promise<RuleRow>;
  updateRule(id: number, input: RulePatch): Promise<RuleRow>;
  /** Soft delete: the row survives for its hit count. */
  deleteRule(id: number): Promise<Deleted>;

  getReviewQueue(): Promise<FlaggedTxn[]>;
  /** One transaction to re-review, as a full register row. */
  getReviewTransaction(id: number): Promise<RegisterRow>;
  applyReview(id: number, input: ReviewApplyRequest): Promise<ReviewApplyResponse>;
  /** Takes a decision back; answers with the restored row. */
  undoReview(id: number, input: ReviewUndoRequest): Promise<RegisterRow>;
  /** A dry run against real descriptions; writes nothing. */
  testRule(input: RuleTestRequest): Promise<RuleTestResult>;

  /**
   * Park a statement on the server. Deliberately no progress callback: `fetch`
   * cannot report upload progress, and promising it here would commit every
   * future implementation to producing something it may not have. A client
   * that can measure it adds an options object without breaking this one.
   */
  uploadImport(file: File): Promise<UploadResponse>;
  /** A dry run: reports what an import would do, having written nothing. */
  previewImport(input: ImportRequest): Promise<ImportPreview>;
  /** Snapshot, import, categorize — the sequence the TUI has always used. */
  confirmImport(input: ConfirmImportRequest): Promise<ImportConfirmation>;
  /** The built-in importers this build has; Gusto depends on a cargo feature. */
  getImportFormats(): Promise<ImporterFormat[]>;
  getCsvProfiles(): Promise<CsvProfile[]>;
  /** Import history, newest first — what the undo screen offers to roll back. */
  getImports(): Promise<ImportListItem[]>;
  /**
   * Roll one import back: its transactions and its record.
   *
   * By id rather than "the last one", which is all `nigel undo` can address
   * from a terminal. An import that is already gone is a 404, never a
   * successful undo of nothing.
   */
  deleteImport(id: number): Promise<UndoneImport>;

  /**
   * Compare a statement balance against the calculated one — and record the
   * attempt, including a mismatch. That record is the point: it is how the
   * history knows which months have been checked, so this is a POST.
   */
  reconcile(input: ReconcileRequest): Promise<ReconcileResult>;
  /** Recorded attempts, newest month first. An unknown account is a 404. */
  getReconciliations(options?: ReconciliationParams): Promise<ReconciliationRecord[]>;

  getAppSettings(): Promise<AppSettings>;
  updateAppSettings(input: UpdateAppSettingsRequest): Promise<AppSettings>;
  setCompanyName(name: string): Promise<CompanyNameResponse>;
  /** Answers with the status of the database it switched to. */
  setDataDir(path: string): Promise<StatusResponse>;
  setPassword(input: SetPasswordRequest): Promise<PasswordStateResponse>;
  changePassword(input: ChangePasswordRequest): Promise<PasswordStateResponse>;
  removePassword(input: RemovePasswordRequest): Promise<PasswordStateResponse>;
}

export interface FetchApiClientOptions {
  baseUrl?: string;
  fetchImpl?: typeof fetch;
}

/**
 * A query string for the parameters that are actually set, or `''`.
 *
 * Omitted rather than sent empty: the server rejects a parameter a route does
 * not support instead of ignoring it, so a stray `month=` would be a 400.
 *
 * Generic over the parameter object so each route can keep a named interface
 * with only its own fields, rather than widening to an index signature that
 * would accept any key at all.
 */
function query<T extends object>(params: T): string {
  const search = new URLSearchParams();
  for (const [key, value] of Object.entries(params)) {
    if (value !== undefined) search.set(key, String(value));
  }
  const rendered = search.toString();
  return rendered ? `?${rendered}` : '';
}

function isEnvelope(body: unknown): body is ApiErrorEnvelope {
  if (typeof body !== 'object' || body === null) return false;
  const error = (body as { error?: unknown }).error;
  return (
    typeof error === 'object' &&
    error !== null &&
    typeof (error as { code?: unknown }).code === 'string' &&
    typeof (error as { message?: unknown }).message === 'string'
  );
}

/** Talks to `nigel serve` over HTTP. */
export class FetchApiClient implements ApiClient {
  private readonly baseUrl: string;
  private readonly fetchImpl: typeof fetch;

  constructor(options: FetchApiClientOptions = {}) {
    this.baseUrl = options.baseUrl ?? '/api';
    this.fetchImpl = options.fetchImpl ?? globalThis.fetch.bind(globalThis);
  }

  ping(): Promise<PingResponse> {
    return this.request<PingResponse>('GET', '/ping');
  }

  async getStatus(): Promise<StatusResponse> {
    const status = await this.request<StatusResponse>('GET', '/status');
    // status is the authority on lockedness; everywhere else the signal is
    // only ever raised by a 423.
    appLocked.set(status.locked);
    return status;
  }

  unlock(password: string): Promise<UnlockResponse> {
    return this.request<UnlockResponse>('POST', '/unlock', { password });
  }

  getAppSettings(): Promise<AppSettings> {
    return this.request<AppSettings>('GET', '/settings/app');
  }

  updateAppSettings(input: UpdateAppSettingsRequest): Promise<AppSettings> {
    return this.request<AppSettings>('PUT', '/settings/app', input);
  }

  setCompanyName(name: string): Promise<CompanyNameResponse> {
    return this.request<CompanyNameResponse>('PUT', '/settings/company-name', {
      name,
    });
  }

  async setDataDir(path: string): Promise<StatusResponse> {
    const status = await this.request<StatusResponse>('POST', '/settings/data-dir', {
      path,
    });
    // The switch can land on an encrypted database, so this answer moves the
    // lock signal the same way getStatus does.
    appLocked.set(status.locked);
    return status;
  }

  getPnl(params: ReportDateParams = {}): Promise<ReportEnvelope<PnlReport>> {
    return this.request<ReportEnvelope<PnlReport>>(
      'GET',
      `/reports/pnl${query(params)}`,
    );
  }

  getBalance(): Promise<ReportEnvelope<BalanceReport>> {
    return this.request<ReportEnvelope<BalanceReport>>('GET', '/reports/balance');
  }

  getCashflow(
    params: CashflowParams = {},
  ): Promise<ReportEnvelope<CashflowReport>> {
    return this.request<ReportEnvelope<CashflowReport>>(
      'GET',
      `/reports/cashflow${query(params)}`,
    );
  }

  getFlagged(): Promise<ReportEnvelope<FlaggedTransaction[]>> {
    return this.request<ReportEnvelope<FlaggedTransaction[]>>(
      'GET',
      '/reports/flagged',
    );
  }

  getRegister(params: RegisterParams = {}): Promise<ReportEnvelope<RegisterReport>> {
    return this.request<ReportEnvelope<RegisterReport>>(
      'GET',
      `/reports/register${query(params)}`,
    );
  }

  getExpenses(params: ExpenseParams = {}): Promise<ReportEnvelope<ExpenseBreakdown>> {
    return this.request<ReportEnvelope<ExpenseBreakdown>>(
      'GET',
      `/reports/expenses${query(params)}`,
    );
  }

  getTax(params: YearParams = {}): Promise<ReportEnvelope<TaxSummary>> {
    return this.request<ReportEnvelope<TaxSummary>>(
      'GET',
      `/reports/tax${query(params)}`,
    );
  }

  getK1(params: YearParams = {}): Promise<ReportEnvelope<K1PrepReport>> {
    return this.request<ReportEnvelope<K1PrepReport>>(
      'GET',
      `/reports/k1${query(params)}`,
    );
  }

  exportUrl(
    report: ReportSlug,
    format: ExportFormat,
    params: ExportParams = {},
  ): string {
    return `${this.baseUrl}/exports/${report}${query({ format, ...params })}`;
  }

  getAccounts(): Promise<Account[]> {
    return this.request<Account[]>('GET', '/accounts');
  }

  getCategories(): Promise<CategoryRow[]> {
    return this.request<CategoryRow[]>('GET', '/categories');
  }

  getRules(): Promise<RuleRow[]> {
    return this.request<RuleRow[]>('GET', '/rules');
  }

  patchTransaction(id: number, changes: TransactionPatch): Promise<RegisterRow> {
    return this.request<RegisterRow>('PATCH', `/transactions/${id}`, changes);
  }

  createAccount(input: NewAccountRequest): Promise<Account> {
    return this.request<Account>('POST', '/accounts', input);
  }

  renameAccount(id: number, input: AccountPatch): Promise<Account> {
    return this.request<Account>('PATCH', `/accounts/${id}`, input);
  }

  deleteAccount(id: number): Promise<Deleted> {
    return this.request<Deleted>('DELETE', `/accounts/${id}`);
  }

  createCategory(input: NewCategoryRequest): Promise<CategoryRow> {
    return this.request<CategoryRow>('POST', '/categories', input);
  }

  updateCategory(id: number, input: CategoryPatch): Promise<CategoryRow> {
    return this.request<CategoryRow>('PATCH', `/categories/${id}`, input);
  }

  deleteCategory(id: number): Promise<Deleted> {
    return this.request<Deleted>('DELETE', `/categories/${id}`);
  }

  createRule(input: NewRuleRequest): Promise<RuleRow> {
    return this.request<RuleRow>('POST', '/rules', input);
  }

  updateRule(id: number, input: RulePatch): Promise<RuleRow> {
    return this.request<RuleRow>('PATCH', `/rules/${id}`, input);
  }

  deleteRule(id: number): Promise<Deleted> {
    return this.request<Deleted>('DELETE', `/rules/${id}`);
  }

  getReviewQueue(): Promise<FlaggedTxn[]> {
    return this.request<FlaggedTxn[]>('GET', '/review/queue');
  }

  getReviewTransaction(id: number): Promise<RegisterRow> {
    return this.request<RegisterRow>('GET', `/review/${id}`);
  }

  applyReview(id: number, input: ReviewApplyRequest): Promise<ReviewApplyResponse> {
    return this.request<ReviewApplyResponse>('POST', `/review/${id}/apply`, input);
  }

  undoReview(id: number, input: ReviewUndoRequest): Promise<RegisterRow> {
    return this.request<RegisterRow>('POST', `/review/${id}/undo`, input);
  }

  testRule(input: RuleTestRequest): Promise<RuleTestResult> {
    return this.request<RuleTestResult>('POST', '/rules/test', input);
  }

  uploadImport(file: File): Promise<UploadResponse> {
    const form = new FormData();
    form.append('file', file, file.name);
    return this.request<UploadResponse>('POST', '/imports/upload', form);
  }

  previewImport(input: ImportRequest): Promise<ImportPreview> {
    return this.request<ImportPreview>('POST', '/imports/preview', input);
  }

  confirmImport(input: ConfirmImportRequest): Promise<ImportConfirmation> {
    return this.request<ImportConfirmation>('POST', '/imports/confirm', input);
  }

  getImportFormats(): Promise<ImporterFormat[]> {
    return this.request<ImporterFormat[]>('GET', '/imports/formats');
  }

  getCsvProfiles(): Promise<CsvProfile[]> {
    return this.request<CsvProfile[]>('GET', '/csv-profiles');
  }

  getImports(): Promise<ImportListItem[]> {
    return this.request<ImportListItem[]>('GET', '/imports');
  }

  deleteImport(id: number): Promise<UndoneImport> {
    return this.request<UndoneImport>('DELETE', `/imports/${id}`);
  }

  reconcile(input: ReconcileRequest): Promise<ReconcileResult> {
    return this.request<ReconcileResult>('POST', '/reconcile', input);
  }

  getReconciliations(
    options: ReconciliationParams = {},
  ): Promise<ReconciliationRecord[]> {
    return this.request<ReconciliationRecord[]>(
      'GET',
      `/reconciliations${query(options)}`,
    );
  }

  setPassword(input: SetPasswordRequest): Promise<PasswordStateResponse> {
    return this.request<PasswordStateResponse>('POST', '/settings/password/set', input);
  }

  changePassword(input: ChangePasswordRequest): Promise<PasswordStateResponse> {
    return this.request<PasswordStateResponse>(
      'POST',
      '/settings/password/change',
      input,
    );
  }

  removePassword(input: RemovePasswordRequest): Promise<PasswordStateResponse> {
    return this.request<PasswordStateResponse>(
      'POST',
      '/settings/password/remove',
      input,
    );
  }

  private async request<T>(method: string, path: string, body?: unknown): Promise<T> {
    // A FormData body carries its own content type, and only the browser can
    // generate the multipart boundary that belongs with it. Naming the header
    // here would send a body the server cannot parse.
    const isForm = body instanceof FormData;

    let response: Response;
    try {
      response = await this.fetchImpl(`${this.baseUrl}${path}`, {
        method,
        // The session cookie is HttpOnly and same-site. Behind the vite dev
        // proxy the browser's origin is the vite one, where /auth set the
        // cookie, so same-origin is right in both deployments.
        credentials: 'same-origin',
        headers: isForm ? undefined : { 'Content-Type': 'application/json' },
        body: body === undefined ? undefined : isForm ? body : JSON.stringify(body),
      });
    } catch (cause) {
      throw new ApiError({
        code: 'unknown',
        rawCode: 'network_error',
        message: 'Could not reach the nigel server.',
        status: 0,
      });
    }

    if (!response.ok) {
      throw this.toApiError(await this.errorFrom(response));
    }

    appUnauthorized.set(false);
    if (response.status === 204) return undefined as T;

    const text = await response.text();
    if (text.length === 0) return undefined as T;
    return JSON.parse(text) as T;
  }

  private async errorFrom(response: Response): Promise<ApiError> {
    let parsed: unknown;
    try {
      parsed = await response.json();
    } catch {
      parsed = undefined;
    }

    if (isEnvelope(parsed)) {
      const rawCode = parsed.error.code;
      return new ApiError({
        code: isKnownApiErrorCode(rawCode) ? rawCode : 'unknown',
        rawCode,
        message: parsed.error.message,
        status: response.status,
        details: parsed.error.details,
      });
    }

    const code = STATUS_CODES[response.status] ?? 'unknown';
    return new ApiError({
      code,
      rawCode: code,
      message: response.statusText || `Request failed with status ${response.status}.`,
      status: response.status,
    });
  }

  /** Raise the transport signals this failure implies, then hand it back. */
  private toApiError(error: ApiError): ApiError {
    if (error.isLocked) appLocked.set(true);
    if (error.isUnauthorized) appUnauthorized.set(true);
    return error;
  }
}
