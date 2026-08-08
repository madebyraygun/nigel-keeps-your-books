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
  /**
   * Which invoicing keys are set, by name. Absent while the database is
   * locked — `/api/status` is ungated, and which integrations an installation
   * has configured is not something to advertise before the gate.
   */
  invoicing?: InvoicingStatus;
}

/**
 * `status.invoicing` — key names only. The values never leave the server, so
 * this says what is missing, never what anything is set to.
 */
export interface InvoicingStatus {
  /** All nine send keys are present. */
  sendConfigured: boolean;
  /** `stripe_secret_key` alone, which is all `invoice sync` needs. */
  syncConfigured: boolean;
  /** The unset keys, in `docs/invoicing.md`'s order. */
  missing: string[];
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

/** One category's spending, in `ExpenseBreakdown`. */
export interface ExpenseItem {
  name: string;
  /** Negative, as stored. */
  total: number;
  count: number;
  /** Share of the period's spending, 0-100. */
  pct: number;
}

/** One vendor's spending, in `ExpenseBreakdown`. */
export interface VendorItem {
  vendor: string;
  total: number;
  count: number;
}

/** `GET /api/reports/expenses` */
export interface ExpenseBreakdown {
  categories: ExpenseItem[];
  total: number;
  topVendors: VendorItem[];
}

/** One category mapped to its tax line, in `TaxSummary`. */
export interface TaxItem {
  name: string;
  taxLine: string | null;
  categoryType: string;
  total: number;
}

/** `GET /api/reports/tax` */
export interface TaxSummary {
  lineItems: TaxItem[];
}

/** A category on one 1120-S line, in the K-1 worksheet. */
export interface K1LineItem {
  formLine: string;
  categoryName: string;
  total: number;
}

/**
 * A line 19 deduction, where what is deductible can be less than what was
 * spent — meals are limited to half.
 */
export interface K1OtherDeduction {
  categoryName: string;
  total: number;
  deductible: number;
}

/** The worksheet's sanity checks. */
export interface K1Validation {
  uncategorizedCount: number;
  officerComp: number;
  distributions: number;
  /** Null when there are no distributions to compare against. */
  compDistRatio: number | null;
}

/**
 * `GET /api/reports/k1`
 *
 * `autoMapped` names income categories with no `formLine`, which fall back to
 * gross receipts. `unmapped` holds expense categories with no `formLine`: they
 * have activity but no line to sit on, so they are excluded from every total
 * above and surfaced for the user to map.
 */
export interface K1PrepReport {
  grossReceipts: number;
  cogs: number;
  grossProfit: number;
  otherIncome: number;
  totalDeductions: number;
  ordinaryBusinessIncome: number;
  deductionLines: K1LineItem[];
  scheduleKItems: K1LineItem[];
  otherDeductions: K1OtherDeduction[];
  otherDeductionsTotal: number;
  autoMapped: string[];
  unmapped: K1LineItem[];
  validation: K1Validation;
}

/** The eight reports, as each is spelled in its route. */
export const REPORT_SLUGS = [
  'pnl',
  'expenses',
  'tax',
  'cashflow',
  'balance',
  'flagged',
  'register',
  'k1',
] as const;

export type ReportSlug = (typeof REPORT_SLUGS)[number];

export type ExportFormat = 'pdf' | 'text';

/**
 * Everything an export route can be given. Which of these a given report
 * accepts is the report's business — sending one it does not take is a `400`,
 * not a silently ignored parameter.
 */
export interface ExportParams {
  year?: number;
  month?: string;
  from?: string;
  to?: string;
  account?: string;
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

/**
 * `POST /api/categorize` — what the rules pass over everything uncategorized
 * did.
 *
 * Both counts are ledger-wide, as `categorize_transactions` scans every
 * transaction with no category rather than any one import's.
 */
export interface CategorizeResult {
  categorized: number;
  stillFlagged: number;
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
 * `GET /api/rules` — active rules in the order the categorizer applies them:
 * priority descending, ties by id.
 *
 * `matchType` is a plain string rather than `RuleMatchType`. Every write path
 * validates against the three, but a row written by some other tool cannot be
 * assumed to be one of them, and typing it narrowly would make the manager's
 * select quietly retype a rule it merely displayed.
 */
export interface RuleRow {
  id: number;
  pattern: string;
  matchType: string;
  vendor: string | null;
  /** The category's name, joined for display. */
  category: string;
  categoryId: number;
  priority: number;
  hitCount: number;
}

/** `POST /api/accounts` */
export interface NewAccountRequest {
  name: string;
  accountType: string;
  institution?: string | null;
  lastFour?: string | null;
}

/**
 * `PATCH /api/accounts/:id`
 *
 * Renaming is the whole of it: institution and last four are set at creation,
 * which is all the data layer offers.
 */
export interface AccountPatch {
  name: string;
}

/** `POST /api/categories` */
export interface NewCategoryRequest {
  name: string;
  categoryType: string;
  taxLine?: string | null;
  formLine?: string | null;
}

/**
 * `PATCH /api/categories/:id` — a true partial update.
 *
 * Omitting a field keeps it; sending `null` clears `taxLine` or `formLine`. A
 * body with no recognized field is a 400, so a save with nothing changed must
 * not be sent at all.
 */
export interface CategoryPatch {
  name?: string;
  categoryType?: string;
  taxLine?: string | null;
  formLine?: string | null;
}

/** `POST /api/rules` — `matchType` defaults to `contains`, `priority` to 0. */
export interface NewRuleRequest {
  pattern: string;
  categoryId: number;
  vendor?: string | null;
  matchType?: RuleMatchType;
  priority?: number;
}

/** `PATCH /api/rules/:id`. `vendor: null` clears it. */
export interface RulePatch {
  pattern?: string;
  matchType?: RuleMatchType;
  vendor?: string | null;
  categoryId?: number;
  priority?: number;
}

/**
 * What every delete answers with — a body rather than a bare 204, so a client
 * decodes every response the same way.
 */
export interface Deleted {
  id: number;
  deleted: boolean;
}

/**
 * The `details.reason` values a 409 carries.
 *
 * The codes are the contract: a client explains the block in its own words
 * instead of parsing the server's sentence, which is what makes the message
 * translatable and the count formattable.
 */
export const CONFLICT_REASONS = [
  'has_transactions',
  'has_active_rules',
  'duplicate_name',
  'already_inactive',
  'no_transactions',
  'already_encrypted',
  'not_encrypted',
] as const;

export type ConflictReason = (typeof CONFLICT_REASONS)[number];

/** The shape of `error.details` on a 409. Every field is optional by reason. */
export interface ConflictDetails {
  reason?: ConflictReason;
  count?: number;
  name?: string;
  /** `no_transactions` names the account and month it found nothing in. */
  account?: string;
  month?: string;
}

/**
 * The `details.reason` values a 404 carries.
 *
 * The same contract as the conflict reasons, and it matters most where one
 * route can answer 404 for two unrelated things: an apply that cannot find its
 * transaction is a row to skip past, an apply that cannot find its category is
 * a decision still waiting to be made.
 */
export const NOT_FOUND_REASONS = [
  'transaction_not_found',
  'category_not_found',
  'account_not_found',
  'upload_not_found',
  'invoice_not_found',
  'client_not_found',
] as const;

export type NotFoundReason = (typeof NOT_FOUND_REASONS)[number];

/** The shape of `error.details` on a 404. */
export interface NotFoundDetails {
  reason?: NotFoundReason;
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
export const UPLOAD_NOT_FOUND: NotFoundReason = 'upload_not_found';

// -- reconcile and undo -------------------------------------------------------

/** `POST /api/reconcile` */
export interface ReconcileRequest {
  account: string;
  month: string;
  statementBalance: number;
}

/**
 * `POST /api/reconcile` — the comparison, already made.
 *
 * `discrepancy` is the absolute difference rounded to cents, and
 * `isReconciled` is the server's `< 0.01` verdict on it. Neither is
 * recomputed here: a client that re-derived the tolerance would eventually
 * disagree with the record the server just wrote.
 */
export interface ReconcileResult {
  isReconciled: boolean;
  statementBalance: number;
  calculatedBalance: number;
  discrepancy: number;
}

/**
 * `GET /api/reconciliations` — one recorded attempt, newest month first.
 *
 * Both balances and `reconciledAt` are nullable because the columns are: a
 * record can predate either figure, and only a reconciled one is stamped.
 */
export interface ReconciliationRecord {
  id: number;
  accountId: number;
  accountName: string;
  month: string;
  statementBalance: number | null;
  calculatedBalance: number | null;
  isReconciled: boolean;
  reconciledAt: string | null;
  notes: string | null;
}

/**
 * `GET /api/imports` — import history, newest first.
 *
 * `transactionCount` is what is still attached to the import, so an import
 * whose rows were removed some other way lists at zero rather than vanishing.
 */
export interface ImportListItem {
  id: number;
  filename: string;
  accountName: string;
  importDate: string;
  transactionCount: number;
}

/** `DELETE /api/imports/:id` — what rolling one back removed. */
export interface UndoneImport {
  id: number;
  deletedTransactions: number;
}

// ---------------------------------------------------------------------------
// Invoicing — read side
// ---------------------------------------------------------------------------

/**
 * The six derived statuses. `refresh_status` computes them from `publishedAt`,
 * the payment total and the due date; nothing sets one by hand.
 */
export const INVOICE_STATUSES = [
  'draft',
  'sent',
  'partial',
  'paid',
  'overdue',
  'void',
] as const;

export type InvoiceStatus = (typeof INVOICE_STATUSES)[number];

/** The `invoice_payments.method` CHECK set. */
export const PAYMENT_METHODS = ['stripe', 'ach', 'direct_deposit', 'other'] as const;

export type PaymentMethod = (typeof PAYMENT_METHODS)[number];

/** `GET /api/clients` */
export interface Client {
  id: number;
  name: string;
  email: string | null;
  billingAddress: string | null;
  notes: string | null;
}

/** One row of a client's invoice history. */
export interface ClientInvoiceRow {
  number: number;
  status: string;
  issueDate: string;
  dueDate: string | null;
  total: number;
  paid: number;
}

/**
 * `GET /api/clients/{id}` — the client's own fields flattened beside its
 * history, which is why this extends `Client` rather than nesting one.
 */
export interface ClientDetail extends Client {
  /** Newest number first. */
  invoices: ClientInvoiceRow[];
  /** Open invoices only, clamped per invoice so no overpayment leaks across. */
  outstanding: number;
}

/**
 * `GET /api/invoices` — one row per invoice, number descending.
 *
 * `status` is `string`, not `InvoiceStatus`: the column has no CHECK
 * constraint, so a row written by the InvoiceShelf importer or by hand cannot
 * be assumed to be one of the six. The same deliberate widening
 * `RuleRow.matchType` documents.
 */
export interface InvoiceListRow {
  id: number;
  number: number;
  status: string;
  clientId: number;
  /**
   * Null when the client row is gone — the join is a LEFT JOIN, so an orphaned
   * invoice shows a dash rather than dropping off the list.
   */
  clientName: string | null;
  issueDate: string;
  dueDate: string | null;
  currency: string;
  total: number;
  paid: number;
  balance: number;
}

export interface InvoiceLineItem {
  id: number | null;
  invoiceId: number;
  description: string;
  quantity: number;
  unitAmount: number;
  lineTotal: number;
  position: number;
}

export interface InvoicePayment {
  id: number | null;
  invoiceId: number;
  amount: number;
  paidDate: string;
  method: string;
  stripeCheckoutSessionId: string | null;
}

/**
 * `GET /api/invoices/{number}` — the invoice flattened, plus everything a
 * detail screen prints.
 *
 * There is no `token`: it is the only access control on a published invoice and
 * never crosses the wire. `publicUrl` is the address computed from it.
 */
export interface InvoiceDetail {
  id: number;
  number: number;
  clientId: number;
  status: string;
  currency: string;
  issueDate: string;
  dueDate: string | null;
  subtotal: number;
  tax: number;
  total: number;
  notes: string | null;
  terms: string | null;
  stripePaymentLinkId: string | null;
  stripePaymentLinkUrl: string | null;
  publishedAt: string | null;
  voidedAt: string | null;
  client: Client;
  items: InvoiceLineItem[];
  payments: InvoicePayment[];
  paid: number;
  balance: number;
  /** Null when unpublished or when `public_base_url` is unset — never an error. */
  publicUrl: string | null;
  /**
   * The server's own guards, not a re-derivation from `status`: an edit is
   * blocked by recorded payments as well as by status. These disable a control;
   * the 409 is what enforces it.
   */
  canEdit: boolean;
  canSend: boolean;
  canVoid: boolean;
  canPay: boolean;
}

export interface AgingBucket {
  label: string;
  count: number;
  total: number;
}

export interface AgingInvoice {
  number: number;
  client: string;
  /** The date the bucket aged from: the due date, or the issue date if none. */
  dueDate: string;
  daysPastDue: number;
  bucket: string;
  total: number;
  paid: number;
  balance: number;
}

/** `GET /api/invoices/aging` */
export interface AgingReport {
  asOf: string;
  /** Always five, in fixed order. */
  buckets: AgingBucket[];
  /** Open invoices, most overdue first. */
  invoices: AgingInvoice[];
  outstanding: number;
}

/**
 * `GET /api/invoices` filters. An absent one is omitted from the query string
 * entirely rather than sent empty.
 */
export interface InvoiceListParams {
  /** A status word, or `open` for sent/partial/overdue. */
  status?: string;
  clientId?: number;
}

/** `GET /api/invoices/next-number` — reads the counter, reserves nothing. */
export interface NextInvoiceNumber {
  number: number;
}

// ---------------------------------------------------------------------------
// Invoicing — write side
// ---------------------------------------------------------------------------

/** `POST /api/clients` */
export interface NewClientRequest {
  name: string;
  email?: string | null;
  billingAddress?: string | null;
  notes?: string | null;
}

/**
 * `PATCH /api/clients/{id}` — an omitted field is left alone and an explicit
 * `null` clears it, so the two must stay distinguishable all the way to the
 * request body. An all-absent patch is a 400.
 */
export interface ClientPatch {
  name?: string;
  email?: string | null;
  billingAddress?: string | null;
  notes?: string | null;
}

/**
 * One line of an invoice as it is sent. No `id` and no `position`: the server
 * derives positions from the array order, and the whole list is replaced.
 */
export interface NewLineItem {
  description: string;
  quantity: number;
  unitAmount: number;
}

/**
 * `POST /api/invoices` — answers 201 with the created draft's whole
 * `InvoiceDetail`. `currency` defaults to `USD` server-side.
 */
export interface NewInvoiceRequest {
  clientId: number;
  issueDate: string;
  dueDate?: string | null;
  currency?: string;
  items: NewLineItem[];
  notes?: string | null;
  terms?: string | null;
}

/**
 * `PATCH /api/invoices/{number}` — draft-only, and `items` is a **whole-list
 * replacement**: sending it at all means "these are the line items now".
 */
export interface InvoicePatch {
  issueDate?: string;
  dueDate?: string | null;
  currency?: string;
  notes?: string | null;
  terms?: string | null;
  items?: NewLineItem[];
}

/**
 * `POST /api/invoices/{number}/pay`. An omitted `amount` records the whole
 * outstanding balance, exactly as omitting `--amount` does; `method` defaults
 * to `direct_deposit`.
 */
export interface PayInvoiceRequest {
  date: string;
  amount?: number;
  method?: PaymentMethod;
}
