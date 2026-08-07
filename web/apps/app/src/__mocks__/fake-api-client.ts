import type {
  ApiClient,
  CashflowParams,
  ExpenseParams,
  ReconciliationParams,
  RegisterParams,
  ReportDateParams,
  YearParams,
} from '../api/client.js';
import type {
  Account,
  AccountPatch,
  AppSettings,
  BalanceReport,
  CashflowReport,
  CategoryPatch,
  CategoryRow,
  ChangePasswordRequest,
  CompanyNameResponse,
  ConfirmImportRequest,
  ConflictDetails,
  CsvProfile,
  Deleted,
  ExpenseBreakdown,
  ExportFormat,
  ExportParams,
  FlaggedTransaction,
  FlaggedTxn,
  ImportConfirmation,
  ImporterFormat,
  ImportListItem,
  ImportPreview,
  ImportRequest,
  K1PrepReport,
  NewAccountRequest,
  NewCategoryRequest,
  NewRuleRequest,
  PasswordStateResponse,
  PingResponse,
  PnlReport,
  ReconcileRequest,
  ReconcileResult,
  ReconciliationRecord,
  RegisterReport,
  RegisterRow,
  RemovePasswordRequest,
  ReportEnvelope,
  ReportSlug,
  ReviewApplyRequest,
  ReviewApplyResponse,
  ReviewUndoRequest,
  RulePatch,
  RuleRow,
  RuleTestRequest,
  RuleTestResult,
  SetPasswordRequest,
  StatusResponse,
  TaxSummary,
  TransactionPatch,
  UndoneImport,
  UnlockResponse,
  UpdateAppSettingsRequest,
  UploadResponse,
} from '../api/types.js';
import { UPLOAD_NOT_FOUND } from '../api/types.js';
import { ApiError } from '../api/client.js';

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

export const EMPTY_EXPENSES: ExpenseBreakdown = {
  categories: [],
  total: 0,
  topVendors: [],
};

export const EMPTY_TAX: TaxSummary = { lineItems: [] };

export const EMPTY_K1: K1PrepReport = {
  grossReceipts: 0,
  cogs: 0,
  grossProfit: 0,
  otherIncome: 0,
  totalDeductions: 0,
  ordinaryBusinessIncome: 0,
  deductionLines: [],
  scheduleKItems: [],
  otherDeductions: [],
  otherDeductionsTotal: 0,
  autoMapped: [],
  unmapped: [],
  validation: {
    uncategorizedCount: 0,
    officerComp: 0,
    distributions: 0,
    compDistRatio: null,
  },
};

export const DEFAULT_APP_SETTINGS: AppSettings = {
  userName: 'Tester',
  updateCheck: true,
  lastUpdateCheck: null,
};

export const EMPTY_IMPORT_PREVIEW: ImportPreview = {
  imported: 0,
  skipped: 0,
  malformed: 0,
  duplicateFile: false,
  sample: [],
  format: null,
  importId: null,
};

export const EMPTY_IMPORT_CONFIRMATION: ImportConfirmation = {
  ...EMPTY_IMPORT_PREVIEW,
  categorized: 0,
  stillFlagged: 0,
  snapshot: '/tmp/nigel/snapshots/pre-import-20250401-120000.db',
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

/** The order `GET /api/rules` answers in: priority descending, ties by id. */
function sortRules(rules: RuleRow[]): RuleRow[] {
  return [...rules].sort((a, b) => b.priority - a.priority || a.id - b.id);
}

/**
 * A 409 exactly as the server shapes one, for the guardrail paths.
 *
 * Every manager screen has to render four of these, and hand-building the
 * envelope at each call site is how one of them ends up subtly different from
 * what the server actually sends.
 */
export function conflictError(
  reason: string,
  extra: Omit<ConflictDetails, 'reason'> & { message?: string } = {},
): ApiError {
  const { message, ...details } = extra;
  return new ApiError({
    code: 'conflict',
    rawCode: 'conflict',
    message: message ?? `Refused: ${reason}`,
    status: 409,
    details: { reason, ...details },
  });
}

/** A 404 exactly as the server shapes one, for the missing-row paths. */
export function notFoundError(message: string): ApiError {
  return new ApiError({
    code: 'not_found',
    rawCode: 'not_found',
    message,
    status: 404,
  });
}

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

  expenses: ExpenseBreakdown = EMPTY_EXPENSES;
  tax: TaxSummary = EMPTY_TAX;
  k1: K1PrepReport = EMPTY_K1;

  expensesError: Error | null = null;
  taxError: Error | null = null;
  k1Error: Error | null = null;

  async getExpenses(
    params: ExpenseParams = {},
  ): Promise<ReportEnvelope<ExpenseBreakdown>> {
    this.calls.push(`getExpenses:${registerQuery(params)}`);
    if (this.expensesError) throw this.expensesError;
    return { granularity: 'monthAndYear', report: this.expenses };
  }

  async getTax(params: YearParams = {}): Promise<ReportEnvelope<TaxSummary>> {
    this.calls.push(`getTax:${registerQuery(params)}`);
    if (this.taxError) throw this.taxError;
    return { granularity: 'yearOnly', report: this.tax };
  }

  async getK1(params: YearParams = {}): Promise<ReportEnvelope<K1PrepReport>> {
    this.calls.push(`getK1:${registerQuery(params)}`);
    if (this.k1Error) throw this.k1Error;
    return { granularity: 'yearOnly', report: this.k1 };
  }

  /**
   * Deliberately not the real address, and deliberately not recorded in
   * `calls`.
   *
   * A screen puts whatever this returns into an href without caring what it
   * looks like, so the fake hands back something no server would serve: a
   * screen test then proves the href came from the client, and the real
   * `/api/exports/...` shape is asserted where it belongs, against
   * `FetchApiClient` inside the seam.
   *
   * It stays out of the call log because it reaches no server — logging it
   * would put an entry in the log on every render and make "issued exactly one
   * request" impossible to assert.
   */
  exportUrl(report: ReportSlug, format: ExportFormat, params: ExportParams = {}): string {
    const search = new URLSearchParams({ format });
    for (const key of ['year', 'month', 'from', 'to', 'account'] as const) {
      const value = params[key];
      if (value !== undefined) search.set(key, String(value));
    }
    return `fake-export:/${report}?${search.toString()}`;
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

  // -- review ---------------------------------------------------------------
  //
  // `applyReview` and `undoReview` really move the fixture between flagged and
  // categorized, because the round trip the Back button makes is the whole
  // point of the feature: a fake that only logged its calls would pass even if
  // undo were wired to the wrong transaction.

  reviewQueue: FlaggedTxn[] = [];
  /** Full rows by id — what re-review by id reads and what undo answers with. */
  reviewRows = new Map<number, RegisterRow>();
  ruleTest: RuleTestResult = { total: 0, matches: [] };

  queueError: Error | null = null;
  reviewTransactionError: Error | null = null;
  applyError: Error | null = null;
  undoError: Error | null = null;
  ruleTestError: Error | null = null;

  /** Rule ids are handed out in order so a test can name the one undo must take. */
  private nextRuleId = 100;

  async getReviewQueue(): Promise<FlaggedTxn[]> {
    this.calls.push('getReviewQueue');
    if (this.queueError) throw this.queueError;
    return this.reviewQueue;
  }

  async getReviewTransaction(id: number): Promise<RegisterRow> {
    this.calls.push(`getReviewTransaction:${id}`);
    if (this.reviewTransactionError) throw this.reviewTransactionError;

    const row = this.reviewRows.get(id);
    if (!row) throw new Error(`no transaction ${id} in the fixture`);
    return { ...row };
  }

  async applyReview(
    id: number,
    input: ReviewApplyRequest,
  ): Promise<ReviewApplyResponse> {
    this.calls.push(`applyReview:${id}:${JSON.stringify(input)}`);
    if (this.applyError) throw this.applyError;

    const row = this.reviewRows.get(id);
    if (row) {
      row.categoryId = input.categoryId;
      row.category =
        this.categories.find((c) => c.id === input.categoryId)?.name ?? row.category;
      row.vendor = input.vendor ?? null;
      row.isFlagged = false;
    }

    return { transactionId: id, ruleId: input.createRule ? this.nextRuleId++ : null };
  }

  async undoReview(id: number, input: ReviewUndoRequest): Promise<RegisterRow> {
    this.calls.push(`undoReview:${id}:${JSON.stringify(input)}`);
    if (this.undoError) throw this.undoError;

    const row = this.reviewRows.get(id);
    if (!row) throw new Error(`no transaction ${id} in the fixture`);

    // What `undo_review` does: re-flag the row and clear both fields.
    row.categoryId = null;
    row.category = null;
    row.vendor = null;
    row.isFlagged = true;
    return { ...row };
  }

  async testRule(input: RuleTestRequest): Promise<RuleTestResult> {
    this.calls.push(`testRule:${JSON.stringify(input)}`);
    if (this.ruleTestError) throw this.ruleTestError;
    return this.ruleTest;
  }

  // -- managers -------------------------------------------------------------
  //
  // Every write really moves the fixture, because the screens refetch after a
  // mutation rather than splicing the response in: a fake that only logged its
  // calls would pass even if the refetch were wired to the wrong list.

  rules: RuleRow[] = [];

  rulesError: Error | null = null;
  createAccountError: Error | null = null;
  renameAccountError: Error | null = null;
  deleteAccountError: Error | null = null;
  createCategoryError: Error | null = null;
  updateCategoryError: Error | null = null;
  deleteCategoryError: Error | null = null;
  createRuleError: Error | null = null;
  updateRuleError: Error | null = null;
  deleteRuleError: Error | null = null;

  /** Ids handed out in order, so a test can name the row it expects. */
  private nextId = 900;

  async getRules(): Promise<RuleRow[]> {
    this.calls.push('getRules');
    if (this.rulesError) throw this.rulesError;
    return this.rules;
  }

  async createAccount(input: NewAccountRequest): Promise<Account> {
    this.calls.push(`createAccount:${JSON.stringify(input)}`);
    if (this.createAccountError) throw this.createAccountError;

    const account: Account = {
      id: this.nextId++,
      name: input.name,
      accountType: input.accountType,
      institution: input.institution ?? null,
      lastFour: input.lastFour ?? null,
    };
    this.accounts = [...this.accounts, account];
    return account;
  }

  async renameAccount(id: number, input: AccountPatch): Promise<Account> {
    this.calls.push(`renameAccount:${id}:${JSON.stringify(input)}`);
    if (this.renameAccountError) throw this.renameAccountError;

    const account = this.accounts.find((candidate) => candidate.id === id);
    if (!account) throw new Error(`no account ${id} in the fixture`);
    account.name = input.name;
    return { ...account };
  }

  async deleteAccount(id: number): Promise<Deleted> {
    this.calls.push(`deleteAccount:${id}`);
    if (this.deleteAccountError) throw this.deleteAccountError;

    this.accounts = this.accounts.filter((account) => account.id !== id);
    return { id, deleted: true };
  }

  async createCategory(input: NewCategoryRequest): Promise<CategoryRow> {
    this.calls.push(`createCategory:${JSON.stringify(input)}`);
    if (this.createCategoryError) throw this.createCategoryError;

    const category: CategoryRow = {
      id: this.nextId++,
      name: input.name,
      categoryType: input.categoryType,
      taxLine: input.taxLine ?? null,
      formLine: input.formLine ?? null,
    };
    this.categories = [...this.categories, category];
    return category;
  }

  async updateCategory(id: number, input: CategoryPatch): Promise<CategoryRow> {
    this.calls.push(`updateCategory:${id}:${JSON.stringify(input)}`);
    if (this.updateCategoryError) throw this.updateCategoryError;

    const category = this.categories.find((candidate) => candidate.id === id);
    if (!category) throw new Error(`no category ${id} in the fixture`);

    // Absent keeps, null clears — the double_option semantics the route has.
    if (input.name !== undefined) category.name = input.name;
    if (input.categoryType !== undefined) category.categoryType = input.categoryType;
    if (input.taxLine !== undefined) category.taxLine = input.taxLine;
    if (input.formLine !== undefined) category.formLine = input.formLine;
    return { ...category };
  }

  async deleteCategory(id: number): Promise<Deleted> {
    this.calls.push(`deleteCategory:${id}`);
    if (this.deleteCategoryError) throw this.deleteCategoryError;

    // Soft-deleted server-side, and the list endpoint omits inactive rows —
    // which from a client's point of view is the same as gone.
    this.categories = this.categories.filter((category) => category.id !== id);
    return { id, deleted: true };
  }

  async createRule(input: NewRuleRequest): Promise<RuleRow> {
    this.calls.push(`createRule:${JSON.stringify(input)}`);
    if (this.createRuleError) throw this.createRuleError;

    const rule: RuleRow = {
      id: this.nextId++,
      pattern: input.pattern,
      matchType: input.matchType ?? 'contains',
      vendor: input.vendor ?? null,
      category:
        this.categories.find((c) => c.id === input.categoryId)?.name ?? 'Unknown',
      categoryId: input.categoryId,
      priority: input.priority ?? 0,
      hitCount: 0,
    };
    this.rules = sortRules([...this.rules, rule]);
    return rule;
  }

  async updateRule(id: number, input: RulePatch): Promise<RuleRow> {
    this.calls.push(`updateRule:${id}:${JSON.stringify(input)}`);
    if (this.updateRuleError) throw this.updateRuleError;

    const rule = this.rules.find((candidate) => candidate.id === id);
    if (!rule) throw new Error(`no rule ${id} in the fixture`);

    if (input.pattern !== undefined) rule.pattern = input.pattern;
    if (input.matchType !== undefined) rule.matchType = input.matchType;
    if (input.vendor !== undefined) rule.vendor = input.vendor;
    if (input.priority !== undefined) rule.priority = input.priority;
    if (input.categoryId !== undefined) {
      rule.categoryId = input.categoryId;
      rule.category =
        this.categories.find((c) => c.id === input.categoryId)?.name ?? rule.category;
    }

    // A priority edit reorders the list, which is the reason the screens
    // refetch instead of splicing the response back in.
    this.rules = sortRules([...this.rules]);
    return { ...rule };
  }

  async deleteRule(id: number): Promise<Deleted> {
    this.calls.push(`deleteRule:${id}`);
    if (this.deleteRuleError) throw this.deleteRuleError;

    this.rules = this.rules.filter((rule) => rule.id !== id);
    return { id, deleted: true };
  }

  // -- imports --------------------------------------------------------------
  //
  // The upload really is remembered and really is consumed: a confirmed import
  // drops its upload, so a screen that reuses a spent id gets the same 404 the
  // server would give it. That is the behaviour the expired-upload retry is
  // built on, and a fake that only logged calls could not exercise it.

  importFormats: ImporterFormat[] = [];
  csvProfiles: CsvProfile[] = [];
  importPreview: ImportPreview = { ...EMPTY_IMPORT_PREVIEW };
  importConfirmation: ImportConfirmation = { ...EMPTY_IMPORT_CONFIRMATION };

  uploadError: Error | null = null;
  previewError: Error | null = null;
  confirmError: Error | null = null;
  formatsError: Error | null = null;
  csvProfilesError: Error | null = null;

  /** Thrown once, then cleared — for the retry paths that must recover. */
  previewErrorOnce: Error | null = null;
  confirmErrorOnce: Error | null = null;

  /** Upload ids handed out in order, so a test can name the one it expects. */
  private nextUploadId = 1;
  /** Ids the server would still recognize. */
  readonly liveUploads = new Set<string>();

  async uploadImport(file: File): Promise<UploadResponse> {
    // The name and size only — never the bytes, which no assertion wants and
    // which would make a failure message unreadable.
    this.calls.push(`uploadImport:${file.name}`);
    if (this.uploadError) throw this.uploadError;

    const uploadId = `upload-${this.nextUploadId++}`;
    this.liveUploads.add(uploadId);
    return { uploadId, filename: file.name, size: file.size };
  }

  async previewImport(input: ImportRequest): Promise<ImportPreview> {
    this.calls.push(`previewImport:${JSON.stringify(input)}`);
    if (this.previewErrorOnce) {
      const error = this.previewErrorOnce;
      this.previewErrorOnce = null;
      throw error;
    }
    if (this.previewError) throw this.previewError;
    this.assertUploadLives(input.uploadId);

    return { ...this.importPreview };
  }

  async confirmImport(input: ConfirmImportRequest): Promise<ImportConfirmation> {
    this.calls.push(`confirmImport:${JSON.stringify(input)}`);
    if (this.confirmErrorOnce) {
      const error = this.confirmErrorOnce;
      this.confirmErrorOnce = null;
      throw error;
    }
    if (this.confirmError) throw this.confirmError;
    this.assertUploadLives(input.uploadId);

    // A confirmed upload is deleted server-side, so reusing the id must fail.
    this.liveUploads.delete(input.uploadId);
    if (input.saveProfile && input.mapping) {
      this.csvProfiles = [
        ...this.csvProfiles.filter((profile) => profile.name !== input.saveProfile),
        { name: input.saveProfile, config: input.mapping },
      ];
    }

    return { ...this.importConfirmation };
  }

  async getImportFormats(): Promise<ImporterFormat[]> {
    this.calls.push('getImportFormats');
    if (this.formatsError) throw this.formatsError;
    return this.importFormats;
  }

  async getCsvProfiles(): Promise<CsvProfile[]> {
    this.calls.push('getCsvProfiles');
    if (this.csvProfilesError) throw this.csvProfilesError;
    return this.csvProfiles;
  }

  private assertUploadLives(uploadId: string): void {
    if (this.liveUploads.has(uploadId)) return;
    throw new ApiError({
      code: 'not_found',
      rawCode: 'not_found',
      message: 'That upload is no longer here. Choose the file again.',
      status: 404,
      details: { reason: UPLOAD_NOT_FOUND },
    });
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

  // -- reconcile and undo ---------------------------------------------------
  //
  // `reconcile` really appends a record, because the server really does — it
  // stores the attempt whichever way the comparison went, and that is what the
  // screen's history refresh is showing. A fake that only returned the verdict
  // would pass even if the refresh were never wired up.

  imports: ImportListItem[] = [];
  reconciliations: ReconciliationRecord[] = [];

  /** What the ledger sums to per account name, for `reconcile` to compare against. */
  calculatedBalances: Record<string, number> = {};
  /** `account|month` pairs the server would answer with a 409. */
  readonly emptyMonths = new Set<string>();

  importsError: Error | null = null;
  deleteImportError: Error | null = null;
  reconcileError: Error | null = null;
  reconciliationsError: Error | null = null;

  private nextReconciliationId = 700;

  async getImports(): Promise<ImportListItem[]> {
    this.calls.push('getImports');
    if (this.importsError) throw this.importsError;
    return this.imports;
  }

  async deleteImport(id: number): Promise<UndoneImport> {
    this.calls.push(`deleteImport:${id}`);
    if (this.deleteImportError) throw this.deleteImportError;

    const item = this.imports.find((candidate) => candidate.id === id);
    if (!item) throw notFoundError(`No import with ID ${id}`);

    this.imports = this.imports.filter((candidate) => candidate.id !== id);
    return { id, deletedTransactions: item.transactionCount };
  }

  async reconcile(input: ReconcileRequest): Promise<ReconcileResult> {
    this.calls.push(`reconcile:${JSON.stringify(input)}`);
    if (this.reconcileError) throw this.reconcileError;
    this.assertAccountExists(input.account);

    if (this.emptyMonths.has(`${input.account}|${input.month}`)) {
      throw conflictError('no_transactions', {
        message: `No transactions for ${input.account} in ${input.month}`,
        account: input.account,
        month: input.month,
      });
    }

    const calculatedBalance = this.calculatedBalances[input.account] ?? 0;
    const difference = Math.abs(calculatedBalance - input.statementBalance);
    const isReconciled = difference < 0.01;
    // The server rounds the reported discrepancy to cents; the verdict is
    // taken on the unrounded difference, which is why both appear here.
    const discrepancy = Math.round(difference * 100) / 100;

    this.reconciliations = [
      {
        id: this.nextReconciliationId++,
        accountId: this.accounts.find((a) => a.name === input.account)?.id ?? 0,
        accountName: input.account,
        month: input.month,
        statementBalance: input.statementBalance,
        calculatedBalance,
        isReconciled,
        reconciledAt: isReconciled ? '2025-03-01 12:00:00' : null,
        notes: null,
      },
      ...this.reconciliations,
    ];

    return {
      isReconciled,
      statementBalance: input.statementBalance,
      calculatedBalance,
      discrepancy,
    };
  }

  async getReconciliations(
    options: ReconciliationParams = {},
  ): Promise<ReconciliationRecord[]> {
    const search = new URLSearchParams();
    if (options.account !== undefined) search.set('account', options.account);
    this.calls.push(`getReconciliations:${search.toString()}`);
    if (this.reconciliationsError) throw this.reconciliationsError;

    if (options.account !== undefined) this.assertAccountExists(options.account);
    const rows =
      options.account === undefined
        ? this.reconciliations
        : this.reconciliations.filter((row) => row.accountName === options.account);
    return [...rows].sort((a, b) => b.month.localeCompare(a.month) || b.id - a.id);
  }

  /** The server's `ensure_account_exists`: a wrong question, not an empty answer. */
  private assertAccountExists(name: string): void {
    // An empty fixture means the test did not care about accounts, so the
    // guard stays out of its way rather than 404-ing every call.
    if (this.accounts.length === 0) return;
    if (this.accounts.some((account) => account.name === name)) return;
    throw notFoundError(`Unknown account: ${name}`);
  }
}
