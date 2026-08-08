import { LitElement, html, css, nothing, type PropertyValues, type TemplateResult } from 'lit';
import { customElement, property, state } from 'lit/decorators.js';
import '@nigel/ui';
import {
  paramsToPeriod,
  periodToParams,
  roundHalfEven,
  type NcDateGranularity,
  type NcPeriod,
  type RegisterTableRow,
} from '@nigel/ui';

import { ApiError, type ApiClient } from '../api/index.js';
import { SignalWatcher } from '../mixins/signal-watcher.js';
import { getAppStore, type AppStore } from '../state/app-store.js';
import type {
  AgingReport,
  BalanceReport,
  CashflowReport,
  ExpenseBreakdown,
  ExportParams,
  FlaggedTransaction,
  K1PrepReport,
  PnlReport,
  RegisterReport,
  TaxSummary,
} from '../api/types.js';
import { cashflowBuckets } from './dashboard-data.js';
import {
  agingInvoicesTable,
  agingTable,
  autoMappedNote,
  cashflowTable,
  expenseTable,
  flaggedTable,
  isReportSlug,
  isReportViewSlug,
  k1DeductionTable,
  k1OtherDeductionsTable,
  k1ScheduleKTable,
  k1SummaryTable,
  k1UnmappedTable,
  k1Warnings,
  pnlTable,
  registerFooterNote,
  reportDef,
  reportDefs,
  reportParamsFrom,
  taxTable,
  vendorTable,
  type ReportTable,
  type ReportViewSlug,
} from './reports-data.js';
import type { ScreenContext } from './context.js';
import type { ScreenId } from './registry.js';

/** Whatever the current report answered with. Narrowed by the slug. */
type ReportData =
  | PnlReport
  | ExpenseBreakdown
  | TaxSummary
  | CashflowReport
  | BalanceReport
  | FlaggedTransaction[]
  | RegisterReport
  | K1PrepReport
  | AgingReport;

/** What a loaded report is, for deciding whether the route still wants it. */
function loadKey(slug: ReportViewSlug, request: ExportParams): string {
  return `${slug}:${JSON.stringify(request)}`;
}

/** The K-1 warnings quote figures; they round as `fmt::money` rounds. */
function money(amount: number): string {
  return new Intl.NumberFormat(undefined, {
    style: 'currency',
    currency: 'USD',
  }).format(roundHalfEven(amount, 2));
}

/**
 * All eight reports, and the directory that lists them.
 *
 * One screen rather than eight because the frame is the same every time —
 * period control, export links, a body — and only the body differs. The body
 * is composed from `@nigel/ui` primitives out of pure mappers in
 * `reports-data.ts`, which is what lets the figure-parity test check the
 * numbers against the CLI's own text export without a browser.
 *
 * The period control is driven by the `granularity` the response carries, not
 * by a table in the client: the server is the authority on which of `year` and
 * `month` a route will accept, and it says so on every envelope.
 */
@customElement('nigel-reports-screen')
export class NigelReportsScreen extends SignalWatcher(LitElement) {
  static styles = css`
    :host {
      display: grid;
      gap: var(--wa-space-l, 16px);
      align-content: start;
      padding: var(--wa-space-l, 16px);
      font-family: var(--wa-font-family-sans);
      color: var(--wa-color-text);
    }

    .lede {
      margin: 0;
      color: var(--wa-color-muted);
      max-width: 44rem;
    }

    .toolbar {
      display: flex;
      flex-wrap: wrap;
      align-items: center;
      justify-content: space-between;
      gap: var(--wa-space-m, 12px);
    }

    .toolbar .left {
      display: flex;
      flex-wrap: wrap;
      align-items: center;
      gap: var(--wa-space-m, 12px);
    }

    .heading {
      display: grid;
      gap: var(--wa-space-2xs, 4px);
    }

    h2 {
      margin: 0;
      font-size: var(--wa-font-size-l, 18px);
    }

    .back {
      color: var(--wa-color-brand);
      font-size: var(--wa-font-size-s, 13px);
      text-decoration: none;
    }

    .back:focus-visible {
      outline: 2px solid var(--wa-color-focus);
      outline-offset: 2px;
    }

    .stack {
      display: grid;
      gap: var(--wa-space-l, 16px);
    }

    .note {
      margin: var(--wa-space-xs, 6px) 0 0;
      font-size: var(--wa-font-size-s, 13px);
      color: var(--wa-color-muted);
    }

    .error {
      margin: 0;
      color: var(--wa-color-danger);
    }

    select {
      font: inherit;
      padding: var(--wa-space-2xs, 4px) var(--wa-space-s, 8px);
      border: 1px solid var(--wa-color-border);
      border-radius: var(--wa-radius-sm, 6px);
      background: var(--wa-color-surface);
      color: inherit;
    }

    .filter {
      display: inline-flex;
      align-items: center;
      gap: var(--wa-space-xs, 6px);
      font-size: var(--wa-font-size-s, 13px);
    }
  `;

  @property({ attribute: false })
  client!: ApiClient;

  @property({ attribute: false })
  params: URLSearchParams = new URLSearchParams();

  @property({ attribute: false })
  navigate?: (screen: ScreenId, params?: URLSearchParams) => void;

  @state() private data: ReportData | null = null;
  @state() private granularity: NcDateGranularity = 'none';
  @state() private loading = false;
  @state() private error: string | null = null;
  @state() private accounts: string[] = [];

  /** The request the shown figures answer, set only once they are shown. */
  private loadedKey = '';

  /** Drops a report response whose request is no longer the current one. */
  private loadSeq = 0;

  private appStore: AppStore = getAppStore();

  /**
   * Whether this build of the server can render PDFs.
   *
   * A compile-time cargo feature, reported once on `/api/status`, which the app
   * has already fetched before any screen exists. A download link cannot
   * inspect what comes back, so without this the PDF button on a no-pdf build
   * would save a 501 error envelope as `pnl.pdf`.
   */
  private get pdfExport(): boolean {
    return this.appStore.status.get()?.pdfExport ?? true;
  }

  willUpdate(changed: PropertyValues<this>): void {
    if (!changed.has('params')) return;

    const slug = this.slug;
    if (!slug) {
      this.loadSeq += 1;
      this.data = null;
      this.loadedKey = '';
      return;
    }

    const request = reportParamsFrom(slug, this.effectiveParams);
    if (loadKey(slug, request) === this.loadedKey) return;
    void this.load(slug, request);
  }

  private get slug(): ReportViewSlug | null {
    const raw = this.params.get('report');
    return isReportViewSlug(raw) ? raw : null;
  }

  /**
   * The route, with a period filled in when it carries none.
   *
   * A report screen always shows a period control, so it always has a period:
   * the current year, which is what the TUI's own date navigation is seeded
   * with (`TableReportView::new` takes today, and every builder passes
   * `year.unwrap_or(current_year)`).
   */
  private get effectiveParams(): URLSearchParams {
    const params = new URLSearchParams(this.params);
    if (!params.has('year') && !params.has('month') && !params.has('from')) {
      params.set('year', String(new Date().getFullYear()));
    }
    return params;
  }

  private get period(): NcPeriod {
    return paramsToPeriod(this.effectiveParams);
  }

  /**
   * Fetch one report into `data`.
   *
   * Sequenced: a slow answer that arrives after a faster newer one is dropped,
   * and `loadedKey` moves only when an answer is actually shown. Setting it up
   * front would let a stale response paint figures under the newer period's
   * heading, and the early return in `willUpdate` would then decline to ever
   * ask again.
   */
  private async load(slug: ReportViewSlug, request: ExportParams): Promise<void> {
    const seq = (this.loadSeq += 1);
    this.loading = true;
    this.error = null;

    try {
      const answer = await this.fetchReport(slug, request);
      if (seq !== this.loadSeq) return;
      this.data = answer.report;
      this.granularity = answer.granularity;
      this.loadedKey = loadKey(slug, request);

      // The register's account filter needs the account list, and nothing else
      // on this screen does.
      if (slug === 'register' && this.accounts.length === 0) {
        const accounts = await this.client.getAccounts();
        if (seq !== this.loadSeq) return;
        this.accounts = accounts.map((account) => account.name);
      }
    } catch (cause) {
      if (seq !== this.loadSeq) return;
      this.data = null;
      // Nothing is shown, so nothing is loaded: coming back to this route asks
      // again rather than settling for the error.
      this.loadedKey = '';
      this.error =
        cause instanceof ApiError ? cause.message : 'Could not load this report.';
    } finally {
      if (seq === this.loadSeq) this.loading = false;
    }
  }

  private async fetchReport(
    slug: ReportViewSlug,
    request: ExportParams,
  ): Promise<{ granularity: NcDateGranularity; report: ReportData }> {
    switch (slug) {
      case 'aging':
        // The one entry in the catalog that is not a /api/reports route: aging
        // lives at /api/invoices/aging and is always as of today, so it takes
        // no period and reports none.
        return { granularity: 'none', report: await this.client.getAging() };
      case 'pnl':
        return this.client.getPnl(request);
      case 'expenses':
        return this.client.getExpenses(request);
      case 'tax':
        return this.client.getTax(request);
      case 'cashflow':
        return this.client.getCashflow(request);
      case 'balance':
        return this.client.getBalance();
      case 'flagged':
        return this.client.getFlagged();
      case 'register':
        return this.client.getRegister(request);
      case 'k1':
        return this.client.getK1(request);
    }
  }

  private retry = (): void => {
    const slug = this.slug;
    if (!slug) return;
    void this.load(slug, reportParamsFrom(slug, this.effectiveParams));
  };

  /** Rewrite the route; the hashchange listener is what reloads us. */
  private go(changes: { period?: NcPeriod; account?: string | null }): void {
    const next = new URLSearchParams(this.params);

    if (changes.period) {
      next.delete('year');
      next.delete('month');
      const dates = periodToParams(changes.period);
      if (dates.year !== undefined) next.set('year', String(dates.year));
      if (dates.month !== undefined) next.set('month', dates.month);
    }

    if ('account' in changes) {
      if (changes.account) next.set('account', changes.account);
      else next.delete('account');
    }

    this.navigate?.('reports', next);
  }

  private handlePeriodChange = (event: CustomEvent<{ period: NcPeriod }>): void => {
    this.go({ period: event.detail.period });
  };

  private handleAccountChange = (event: Event): void => {
    const value = (event.target as HTMLSelectElement).value;
    this.go({ account: value || null });
  };

  // -- rendering ------------------------------------------------------------

  private renderTable(table: ReportTable, caption: string, captionHidden = true) {
    return html`
      <wc-report-table
        caption=${caption}
        ?caption-hidden=${captionHidden}
        .columns=${table.columns}
        .rows=${table.rows}
      ></wc-report-table>
    `;
  }

  private renderPanel(heading: string, body: unknown, description = '') {
    return html`
      <wc-panel heading=${heading} description=${description || nothing}>
        ${body}
      </wc-panel>
    `;
  }

  private renderBody(slug: ReportViewSlug) {
    const data = this.data;
    if (data === null) return nothing;

    switch (slug) {
      case 'pnl':
        return this.renderTable(pnlTable(data as PnlReport), 'Profit and loss');
      case 'expenses':
        return this.renderExpenses(data as ExpenseBreakdown);
      case 'tax':
        return this.renderTable(taxTable(data as TaxSummary), 'Tax summary');
      case 'cashflow':
        return this.renderCashflow(data as CashflowReport);
      case 'balance':
        return this.renderBalance(data as BalanceReport);
      case 'flagged':
        return this.renderFlagged(data as FlaggedTransaction[]);
      case 'register':
        return this.renderRegister(data as RegisterReport);
      case 'k1':
        return this.renderK1(data as K1PrepReport);
      case 'aging':
        return this.renderAging(data as AgingReport);
    }
  }

  private renderAging(report: AgingReport) {
    return html`
      <div class="stack">
        ${this.renderPanel(
          `Summary — as of ${report.asOf}`,
          this.renderTable(agingTable(report), 'A/R aging summary'),
        )}
        ${report.invoices.length === 0
          ? nothing
          : this.renderPanel(
              'Open invoices',
              this.renderTable(agingInvoicesTable(report), 'Open invoices'),
            )}
        <p class="note">
          A/R aging is always as of today, so it takes no period.
          <a href="#/invoices">Manage invoices</a> to send, void or record a payment.
        </p>
      </div>
    `;
  }

  private renderExpenses(report: ExpenseBreakdown) {
    return html`
      <div class="stack">
        ${this.renderTable(expenseTable(report), 'Expenses by category')}
        ${report.topVendors.length === 0
          ? nothing
          : this.renderPanel(
              'Top vendors',
              this.renderTable(vendorTable(report), 'Top vendors'),
            )}
      </div>
    `;
  }

  private renderCashflow(report: CashflowReport) {
    const { buckets, caption } = cashflowBuckets(report);
    return html`
      <div class="stack">
        ${buckets.length === 0
          ? nothing
          : html`<wc-bar-chart .buckets=${buckets} caption=${caption}></wc-bar-chart>`}
        ${this.renderTable(cashflowTable(report), 'Cash flow by month')}
      </div>
    `;
  }

  private renderBalance(report: BalanceReport) {
    return html`
      <div class="stack">
        <wc-balance-list
          caption="Account balances"
          .items=${report.accounts.map((account) => ({
            name: account.name,
            accountType: account.accountType,
            balance: account.balance,
          }))}
          .total=${report.total}
        ></wc-balance-list>
        <wc-stat-card
          label="YTD Net Income"
          .amount=${report.ytdNetIncome}
          hint="Year to date, cash basis"
        ></wc-stat-card>
      </div>
    `;
  }

  private renderFlagged(rows: FlaggedTransaction[]) {
    return html`
      <wc-report-table
        caption="Flagged transactions"
        caption-hidden
        empty-message="No flagged transactions."
        .columns=${flaggedTable(rows).columns}
        .rows=${flaggedTable(rows).rows}
      ></wc-report-table>
    `;
  }

  private renderRegister(report: RegisterReport) {
    const rows: RegisterTableRow[] = report.rows.map((row) => ({
      id: row.id,
      date: row.date,
      description: row.description,
      amount: row.amount,
      category: row.category,
      categoryId: row.categoryId,
      vendor: row.vendor,
      accountName: row.accountName,
      isFlagged: row.isFlagged,
    }));

    const account = this.params.get('account') ?? '';

    return html`
      <div class="stack">
        <label class="filter">
          Account
          <select @change=${this.handleAccountChange} .value=${account}>
            <option value="">All accounts</option>
            ${this.accounts.map(
              (name) =>
                html`<option value=${name} ?selected=${name === account}>${name}</option>`,
            )}
          </select>
        </label>
        <wc-register-table
          readonly
          caption="Transaction register"
          .rows=${rows}
          .total=${report.total}
          footer-note=${registerFooterNote(report)}
        ></wc-register-table>
        <p class="note">
          This view is read only.
          <a href="#/register${account ? `?account=${encodeURIComponent(account)}` : ''}"
            >Open the register browser</a
          >
          to edit categories, vendors and flags.
        </p>
      </div>
    `;
  }

  private renderK1(report: K1PrepReport) {
    const note = autoMappedNote(report);
    const warnings = k1Warnings(report, money);
    const scheduleK = k1ScheduleKTable(report);
    const deductions = k1DeductionTable(report);
    const other = k1OtherDeductionsTable(report);

    return html`
      <div class="stack">
        ${this.renderPanel(
          'Income summary',
          html`
            ${this.renderTable(k1SummaryTable(report), 'Income summary')}
            ${note ? html`<p class="note">${note}</p>` : nothing}
          `,
        )}
        ${deductions.rows.length === 0
          ? nothing
          : this.renderPanel(
              'Deductions by line',
              this.renderTable(deductions, 'Deductions by line'),
            )}
        ${scheduleK.rows.length === 0
          ? nothing
          : this.renderPanel('Schedule K', this.renderTable(scheduleK, 'Schedule K'))}
        ${other.rows.length === 0
          ? nothing
          : this.renderPanel(
              'Line 19 — other deductions',
              this.renderTable(other, 'Other deductions'),
            )}
        ${warnings.map(
          (warning) =>
            html`<wc-notice-bar variant="warning" message=${warning}></wc-notice-bar>`,
        )}
        ${report.unmapped.length === 0
          ? nothing
          : this.renderPanel(
              'Needs mapping',
              this.renderTable(k1UnmappedTable(report), 'Categories needing a form line'),
              'These categories have activity but no form line, so they are excluded from the totals above.',
            )}
      </div>
    `;
  }

  private renderLanding() {
    return html`
      <div class="heading">
        <h2>Reports</h2>
        <p class="lede">
          Every report the CLI prints, for any period. Each one exports as text or PDF.
        </p>
      </div>
      <wc-link-grid
        label="Reports"
        .items=${reportDefs().map((def) => ({
          href: `#/reports?report=${def.slug}`,
          label: def.title,
          description: def.description,
          icon: def.icon,
        }))}
      ></wc-link-grid>
    `;
  }

  private renderReport(slug: ReportViewSlug) {
    const def = reportDef(slug);
    // Aging is not an export route: /api/exports carries the same eight slugs
    // /api/reports does, so there is no file to link to.
    const exportable = isReportSlug(slug);
    const request = exportable ? reportParamsFrom(slug, this.effectiveParams) : {};

    return html`
      <div class="heading">
        <a class="back" href="#/reports">← All reports</a>
        <h2>${def.title}</h2>
      </div>

      <div class="toolbar">
        <div class="left">
          <wc-period-nav
            .granularity=${this.granularity}
            .period=${this.period}
            ?disabled=${this.loading}
            @nc-period-change=${this.handlePeriodChange}
          ></wc-period-nav>
        </div>
        ${exportable
          ? html`<wc-export-links
              .pdfAvailable=${this.pdfExport}
              .textHref=${this.client.exportUrl(slug, 'text', request)}
              .pdfHref=${this.client.exportUrl(slug, 'pdf', request)}
              ?busy=${this.loading}
            ></wc-export-links>`
          : nothing}
      </div>

      ${this.loading
        ? html`<wc-spinner size="l" show-label label=${`Loading ${def.title}`}></wc-spinner>`
        : nothing}
      ${this.error
        ? html`
            <wc-empty-state icon="wc-icon-report" heading="That report did not load">
              <p class="error">${this.error}</p>
              <button slot="actions" class="back" type="button" @click=${this.retry}>
                Try again
              </button>
            </wc-empty-state>
          `
        : nothing}
      ${!this.loading && !this.error ? this.renderBody(slug) : nothing}
    `;
  }

  render() {
    const slug = this.slug;
    return slug ? this.renderReport(slug) : this.renderLanding();
  }
}

export function renderReports(ctx: ScreenContext): TemplateResult {
  return html`
    <nigel-reports-screen
      .client=${ctx.client}
      .params=${ctx.params}
      .navigate=${ctx.navigate}
    ></nigel-reports-screen>
  `;
}

declare global {
  interface HTMLElementTagNameMap {
    'nigel-reports-screen': NigelReportsScreen;
  }
}
