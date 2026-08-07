import { signal, type Signal } from '../mixins/signal-watcher.js';
import {
  isKnownApiErrorCode,
  type Account,
  type ApiErrorCode,
  type ApiErrorEnvelope,
  type AppSettings,
  type BalanceReport,
  type CashflowReport,
  type CategoryRow,
  type ChangePasswordRequest,
  type CompanyNameResponse,
  type FlaggedTransaction,
  type InvalidPasswordDetails,
  type PasswordStateResponse,
  type PingResponse,
  type PnlReport,
  type RegisterReport,
  type RegisterRow,
  type RemovePasswordRequest,
  type ReportEnvelope,
  type SetPasswordRequest,
  type StatusResponse,
  type TransactionPatch,
  type UnlockResponse,
  type UpdateAppSettingsRequest,
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

export interface ApiClient {
  ping(): Promise<PingResponse>;
  getStatus(): Promise<StatusResponse>;
  unlock(password: string): Promise<UnlockResponse>;

  getPnl(params?: ReportDateParams): Promise<ReportEnvelope<PnlReport>>;
  getBalance(): Promise<ReportEnvelope<BalanceReport>>;
  getCashflow(params?: CashflowParams): Promise<ReportEnvelope<CashflowReport>>;
  getFlagged(): Promise<ReportEnvelope<FlaggedTransaction[]>>;
  getRegister(params?: RegisterParams): Promise<ReportEnvelope<RegisterReport>>;

  getAccounts(): Promise<Account[]>;
  getCategories(): Promise<CategoryRow[]>;
  /** Partial update; answers with the row as it now stands. */
  patchTransaction(id: number, changes: TransactionPatch): Promise<RegisterRow>;

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

  getAccounts(): Promise<Account[]> {
    return this.request<Account[]>('GET', '/accounts');
  }

  getCategories(): Promise<CategoryRow[]> {
    return this.request<CategoryRow[]>('GET', '/categories');
  }

  patchTransaction(id: number, changes: TransactionPatch): Promise<RegisterRow> {
    return this.request<RegisterRow>('PATCH', `/transactions/${id}`, changes);
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
    let response: Response;
    try {
      response = await this.fetchImpl(`${this.baseUrl}${path}`, {
        method,
        // The session cookie is HttpOnly and same-site. Behind the vite dev
        // proxy the browser's origin is the vite one, where /auth set the
        // cookie, so same-origin is right in both deployments.
        credentials: 'same-origin',
        headers: { 'Content-Type': 'application/json' },
        body: body === undefined ? undefined : JSON.stringify(body),
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
