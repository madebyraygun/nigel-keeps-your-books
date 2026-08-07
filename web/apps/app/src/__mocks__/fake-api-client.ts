import type {
  ApiClient,
  CashflowParams,
  RegisterParams,
  ReportDateParams,
} from '../api/client.js';
import type {
  Account,
  AppSettings,
  BalanceReport,
  CashflowReport,
  CategoryRow,
  ChangePasswordRequest,
  CompanyNameResponse,
  FlaggedTransaction,
  PasswordStateResponse,
  PingResponse,
  PnlReport,
  RegisterReport,
  RegisterRow,
  RemovePasswordRequest,
  ReportEnvelope,
  SetPasswordRequest,
  StatusResponse,
  TransactionPatch,
  UnlockResponse,
  UpdateAppSettingsRequest,
} from '../api/types.js';

/**
 * The query a register request would carry, in a stable key order so a test
 * can assert on the whole thing rather than field by field.
 */
function registerQuery(params: RegisterParams): string {
  const search = new URLSearchParams();
  for (const key of ['year', 'month', 'from', 'to', 'account'] as const) {
    const value = params[key];
    if (value !== undefined) search.set(key, String(value));
  }
  return search.toString();
}

/** A database with nothing in it — the state a fresh `nigel init` leaves. */
export const EMPTY_PNL: PnlReport = {
  income: [],
  expenses: [],
  totalIncome: 0,
  totalExpenses: 0,
  net: 0,
};

export const EMPTY_BALANCE: BalanceReport = {
  accounts: [],
  total: 0,
  ytdNetIncome: 0,
};

export const DEFAULT_APP_SETTINGS: AppSettings = {
  userName: 'Tester',
  updateCheck: true,
  lastUpdateCheck: null,
};

export const UNLOCKED_STATUS: StatusResponse = {
  initialized: true,
  encrypted: false,
  locked: false,
  companyName: 'Test Consultancy',
  version: '0.0.0-test',
  dataDir: '/tmp/nigel',
  pdfExport: true,
  updateAvailable: null,
};

export const LOCKED_STATUS: StatusResponse = {
  ...UNLOCKED_STATUS,
  encrypted: true,
  locked: true,
  companyName: null,
};

/**
 * An ApiClient that never touches the network.
 *
 * Its existence is the point: if a screen can be driven by this, the api
 * interface really is a seam, and a Tauri or remote-server implementation can
 * take the same place without a component noticing.
 */
export class FakeApiClient implements ApiClient {
  status: StatusResponse = UNLOCKED_STATUS;
  statusError: Error | null = null;
  unlockError: Error | null = null;

  readonly calls: string[] = [];

  async ping(): Promise<PingResponse> {
    this.calls.push('ping');
    return { ok: true, version: this.status.version };
  }

  async getStatus(): Promise<StatusResponse> {
    this.calls.push('getStatus');
    if (this.statusError) throw this.statusError;
    return this.status;
  }

  async unlock(password: string): Promise<UnlockResponse> {
    this.calls.push(`unlock:${password}`);
    if (this.unlockError) throw this.unlockError;
    this.status = { ...this.status, locked: false };
    return { locked: false };
  }

  // -- reports --------------------------------------------------------------
  //
  // Each fixture and each error is separate, which is the point: the dashboard
  // fetches all four in parallel and a test needs to fail exactly one of them
  // to prove the other three still render.

  pnl: PnlReport = EMPTY_PNL;
  balance: BalanceReport = EMPTY_BALANCE;
  cashflow: CashflowReport = { months: [] };
  flagged: FlaggedTransaction[] = [];

  pnlError: Error | null = null;
  balanceError: Error | null = null;
  cashflowError: Error | null = null;
  flaggedError: Error | null = null;

  async getPnl(params: ReportDateParams = {}): Promise<ReportEnvelope<PnlReport>> {
    this.calls.push(`getPnl:${params.year ?? ''}`);
    if (this.pnlError) throw this.pnlError;
    return { granularity: 'monthAndYear', report: this.pnl };
  }

  async getBalance(): Promise<ReportEnvelope<BalanceReport>> {
    this.calls.push('getBalance');
    if (this.balanceError) throw this.balanceError;
    return { granularity: 'none', report: this.balance };
  }

  async getCashflow(
    params: CashflowParams = {},
  ): Promise<ReportEnvelope<CashflowReport>> {
    this.calls.push(`getCashflow:${params.year ?? ''}`);
    if (this.cashflowError) throw this.cashflowError;
    return { granularity: 'monthAndYear', report: this.cashflow };
  }

  async getFlagged(): Promise<ReportEnvelope<FlaggedTransaction[]>> {
    this.calls.push('getFlagged');
    if (this.flaggedError) throw this.flaggedError;
    return { granularity: 'none', report: this.flagged };
  }

  // -- register -------------------------------------------------------------
  //
  // `patchTransaction` really applies the patch to the fixture and answers
  // with the updated row, because the screen swaps the response in rather than
  // keeping its optimistic copy — a fake that echoed the request back would
  // never catch that.

  register: RegisterReport = { rows: [], total: 0 };
  accounts: Account[] = [];
  categories: CategoryRow[] = [];

  registerError: Error | null = null;
  accountsError: Error | null = null;
  categoriesError: Error | null = null;
  patchError: Error | null = null;

  async getRegister(
    params: RegisterParams = {},
  ): Promise<ReportEnvelope<RegisterReport>> {
    // The whole query, not just the year: the deep-link tests assert on it.
    this.calls.push(`getRegister:${registerQuery(params)}`);
    if (this.registerError) throw this.registerError;
    return { granularity: 'monthAndYear', report: this.register };
  }

  async getAccounts(): Promise<Account[]> {
    this.calls.push('getAccounts');
    if (this.accountsError) throw this.accountsError;
    return this.accounts;
  }

  async getCategories(): Promise<CategoryRow[]> {
    this.calls.push('getCategories');
    if (this.categoriesError) throw this.categoriesError;
    return this.categories;
  }

  async patchTransaction(id: number, changes: TransactionPatch): Promise<RegisterRow> {
    this.calls.push(`patchTransaction:${id}:${JSON.stringify(changes)}`);
    if (this.patchError) throw this.patchError;

    const row = this.register.rows.find((candidate) => candidate.id === id);
    if (!row) throw new Error(`no transaction ${id} in the fixture`);

    if (changes.categoryId !== undefined) {
      row.categoryId = changes.categoryId;
      row.category =
        this.categories.find((c) => c.id === changes.categoryId)?.name ?? row.category;
    }
    if (changes.vendor !== undefined) row.vendor = changes.vendor;
    if (changes.flag !== undefined) row.isFlagged = changes.flag;

    return { ...row };
  }

  // -- settings -------------------------------------------------------------
  //
  // These record the method name only. `unlock` above records its argument
  // because the boot-order test asserts on it; there is no reason to put a
  // password anywhere else, even in a test double.

  appSettings: AppSettings = DEFAULT_APP_SETTINGS;
  settingsError: Error | null = null;

  async getAppSettings(): Promise<AppSettings> {
    this.calls.push('getAppSettings');
    if (this.settingsError) throw this.settingsError;
    return this.appSettings;
  }

  async updateAppSettings(input: UpdateAppSettingsRequest): Promise<AppSettings> {
    this.calls.push('updateAppSettings');
    if (this.settingsError) throw this.settingsError;
    this.appSettings = { ...this.appSettings, updateCheck: input.updateCheck };
    return this.appSettings;
  }

  async setCompanyName(name: string): Promise<CompanyNameResponse> {
    this.calls.push('setCompanyName');
    if (this.settingsError) throw this.settingsError;
    this.status = { ...this.status, companyName: name };
    return { companyName: name };
  }

  async setDataDir(path: string): Promise<StatusResponse> {
    this.calls.push('setDataDir');
    if (this.settingsError) throw this.settingsError;
    this.status = { ...this.status, dataDir: path };
    return this.status;
  }

  async setPassword(_input: SetPasswordRequest): Promise<PasswordStateResponse> {
    this.calls.push('setPassword');
    if (this.settingsError) throw this.settingsError;
    this.status = { ...this.status, encrypted: true, locked: false };
    return { encrypted: true, locked: false };
  }

  async changePassword(_input: ChangePasswordRequest): Promise<PasswordStateResponse> {
    this.calls.push('changePassword');
    if (this.settingsError) throw this.settingsError;
    return { encrypted: true, locked: false };
  }

  async removePassword(_input: RemovePasswordRequest): Promise<PasswordStateResponse> {
    this.calls.push('removePassword');
    if (this.settingsError) throw this.settingsError;
    this.status = { ...this.status, encrypted: false, locked: false };
    return { encrypted: false, locked: false };
  }
}
