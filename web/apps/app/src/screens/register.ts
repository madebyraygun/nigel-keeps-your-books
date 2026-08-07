import { LitElement, html, css, nothing, type PropertyValues, type TemplateResult } from 'lit';
import { customElement, property, state, query } from 'lit/decorators.js';
import '@awesome.me/webawesome/dist/components/button/button.js';
import '@nigel/ui';
import {
  dispatchNcToast,
  paramsToPeriod,
  periodToParams,
  type AccountOption,
  type CategoryOption,
  type NcAccountChangeDetail,
  type NcEditCommitDetail,
  type NcFlagToggleDetail,
  type NcPeriod,
  type NcRowEventDetail,
  type NcSearchChangeDetail,
  type WcRegisterTable,
  type WcRegisterToolbar,
} from '@nigel/ui';

import { ApiError, type ApiClient, type RegisterParams } from '../api/index.js';
import type { Account, CategoryRow, RegisterRow } from '../api/types.js';
import {
  buildPatch,
  filterRows,
  indexOfToday,
  registerParamsFrom,
  replaceRow,
  todayIso,
} from './register-data.js';
import type { ScreenContext } from './context.js';
import type { ScreenId } from './registry.js';

/**
 * The transaction register — the web counterpart of `browser.rs`.
 *
 * Search is client-side, as it is in the TUI: `/api/reports/register` has no
 * search parameter and gains none, so filtering never costs a round trip and
 * the predicate stays in one place (`register-data.ts`) where both halves of
 * the app can be held to the same contract.
 *
 * Account and period changes go through the route, so a filtered register is
 * a link you can paste and the back button walks the filters. The search box
 * is the exception: it is read from `?q=` on load but not written back on
 * every keystroke, because that would put one history entry per character
 * between the user and the previous screen.
 */
@customElement('nigel-register-screen')
export class NigelRegisterScreen extends LitElement {
  static styles = css`
    :host {
      display: grid;
      gap: var(--wa-space-m, 12px);
      align-content: start;
      padding: var(--wa-space-l, 16px);
      font-family: var(--wa-font-family-sans);
      color: var(--wa-color-text);
      min-height: 0;
    }

    .bar {
      display: flex;
      align-items: end;
      gap: var(--wa-space-m, 12px);
      flex-wrap: wrap;
    }

    wc-register-toolbar {
      flex: 1 1 auto;
    }

    details.help {
      font-size: var(--wa-font-size-s, 13px);
      color: var(--wa-color-muted);
    }

    details.help summary {
      cursor: pointer;
    }

    details.help dl {
      display: grid;
      grid-template-columns: max-content 1fr;
      gap: var(--wa-space-2xs, 4px) var(--wa-space-s, 8px);
      margin: var(--wa-space-xs, 6px) 0 0;
    }

    details.help dt {
      font-family: var(--wa-font-family-mono, monospace);
      color: var(--wa-color-text);
    }

    details.help dd {
      margin: 0;
    }
  `;

  /** Supplied by the registry from the screen context. */
  @property({ attribute: false })
  client!: ApiClient;

  @property({ attribute: false })
  params: URLSearchParams = new URLSearchParams();

  @property({ attribute: false })
  navigate?: (screen: ScreenId, params?: URLSearchParams) => void;

  @state() private rows: RegisterRow[] = [];
  @state() private total = 0;
  @state() private accounts: Account[] = [];
  @state() private categories: CategoryRow[] = [];
  @state() private search = '';
  @state() private loading = true;
  @state() private error: string | null = null;
  @state() private selectedId: number | null = null;
  @state() private editingId: number | null = null;
  @state() private busyId: number | null = null;

  @query('wc-register-table') private table?: WcRegisterTable;
  @query('wc-register-toolbar') private toolbar?: WcRegisterToolbar;

  /** The request the currently loaded rows answer, so a re-render cannot refetch. */
  private loadedKey: string | null = null;
  private seededSearch = false;

  willUpdate(changed: PropertyValues<this>): void {
    if (!changed.has('params')) return;

    if (!this.seededSearch) {
      this.seededSearch = true;
      this.search = this.params.get('q') ?? '';
    }

    const request = registerParamsFrom(this.params);
    const key = JSON.stringify(request);
    if (key === this.loadedKey) return;
    this.loadedKey = key;
    void this.load(request);
  }

  // -- loading --------------------------------------------------------------

  private async load(request: RegisterParams): Promise<void> {
    this.loading = true;
    this.error = null;

    try {
      const [register, accounts, categories] = await Promise.all([
        this.client.getRegister(request),
        this.client.getAccounts(),
        this.client.getCategories(),
      ]);

      this.rows = register.report.rows;
      this.total = register.report.total;
      this.accounts = accounts;
      this.categories = categories;
      this.loading = false;

      await this.updateComplete;
      this.landOnOpeningRow(request);
    } catch (error) {
      this.loading = false;
      this.rows = [];
      this.error =
        error instanceof ApiError ? error.message : 'Could not load the register.';
      // A locked or expired session is the shell's story to tell; anything
      // else is this screen's, and a toast is how the user hears about it.
      if (!(error instanceof ApiError) || (!error.isLocked && !error.isUnauthorized)) {
        dispatchNcToast(this, { message: this.error, variant: 'danger' });
      }
    }
  }

  /**
   * Where the register opens.
   *
   * A deep link to a transaction wins. Otherwise an unfiltered load lands on
   * the last row dated on or before today, which is `scroll_to_today`; a
   * filtered load stays at the top, because the TUI only scrolls to today when
   * no date filter is in play and a March register has no "today" to find.
   */
  private landOnOpeningRow(request: RegisterParams): void {
    if (this.rows.length === 0) return;

    const wanted = Number(this.params.get('id'));
    if (Number.isInteger(wanted) && wanted > 0 && this.table?.scrollToRow(wanted)) {
      this.selectedId = wanted;
      return;
    }

    const dated =
      request.year !== undefined ||
      request.month !== undefined ||
      request.from !== undefined;
    if (dated) return;

    const index = indexOfToday(this.rows, todayIso());
    this.table?.scrollToIndex(index === -1 ? 0 : index);
  }

  // -- filters --------------------------------------------------------------

  private get period(): NcPeriod {
    return paramsToPeriod(this.params);
  }

  private get visibleRows(): RegisterRow[] {
    return filterRows(this.rows, this.search);
  }

  /** Rewrite the route; the hashchange listener is what actually reloads us. */
  private go(changes: { account?: string | null; period?: NcPeriod }): void {
    const next = new URLSearchParams(this.params);

    if ('account' in changes) {
      if (changes.account) next.set('account', changes.account);
      else next.delete('account');
    }

    if (changes.period) {
      next.delete('year');
      next.delete('month');
      next.delete('from');
      next.delete('to');
      const dates = periodToParams(changes.period);
      if (dates.year !== undefined) next.set('year', String(dates.year));
      if (dates.month !== undefined) next.set('month', dates.month);
    }

    // A row from the old filter is not a row in the new one.
    next.delete('id');
    this.navigate?.('register', next);
  }

  private handleAccountChange = (event: CustomEvent<NcAccountChangeDetail>): void => {
    this.go({ account: event.detail.account });
  };

  private handlePeriodChange = (event: CustomEvent<{ period: NcPeriod }>): void => {
    this.go({ period: event.detail.period });
  };

  private handleSearchChange = (event: CustomEvent<NcSearchChangeDetail>): void => {
    this.search = event.detail.query;
  };

  private handleSearchFocus = (): void => {
    this.toolbar?.focusSearch();
  };

  // -- editing --------------------------------------------------------------

  private handleRowSelect = (event: CustomEvent<NcRowEventDetail>): void => {
    this.selectedId = event.detail.id;
  };

  private handleRowActivate = (event: CustomEvent<NcRowEventDetail>): void => {
    this.editingId = event.detail.id;
  };

  private handleEditCancel = (): void => {
    this.editingId = null;
  };

  private handleEditCommit = async (
    event: CustomEvent<NcEditCommitDetail>,
  ): Promise<void> => {
    const row = this.rows.find((candidate) => candidate.id === event.detail.id);
    if (!row) return;

    this.editingId = null;

    const patch = buildPatch(row, event.detail);
    // Nothing changed, so there is nothing to send: an empty PATCH is a 400.
    if (!patch) return;

    await this.persist(row, patch, 'Could not save the transaction.');
  };

  private handleFlagToggle = async (
    event: CustomEvent<NcFlagToggleDetail>,
  ): Promise<void> => {
    const row = this.rows.find((candidate) => candidate.id === event.detail.id);
    if (!row) return;

    await this.persist(row, { flag: event.detail.flag }, 'Could not change the flag.');
  };

  /**
   * Write one edit through, in place.
   *
   * The row is replaced by the one the server answers with rather than by the
   * optimistic copy — the response is the authority on what the row now is,
   * including any field this screen did not touch. A failure puts the original
   * row back, so the table never keeps showing an edit the database refused.
   */
  private async persist(
    row: RegisterRow,
    patch: Parameters<ApiClient['patchTransaction']>[1],
    fallbackMessage: string,
  ): Promise<void> {
    this.busyId = row.id;
    try {
      const updated = await this.client.patchTransaction(row.id, patch);
      this.rows = replaceRow(this.rows, updated);
    } catch (error) {
      this.rows = replaceRow(this.rows, row);
      dispatchNcToast(this, {
        message: error instanceof ApiError ? error.message : fallbackMessage,
        variant: 'danger',
      });
    } finally {
      this.busyId = null;
    }
  }

  // -- render ---------------------------------------------------------------

  private get accountOptions(): AccountOption[] {
    return this.accounts.map((account) => ({ id: account.id, name: account.name }));
  }

  private get categoryOptions(): CategoryOption[] {
    return this.categories.map((category) => ({
      id: category.id,
      name: category.name,
      categoryType: category.categoryType,
    }));
  }

  render() {
    const visible = this.visibleRows;
    const searching = this.search !== '';

    return html`
      <div class="bar">
        <wc-register-toolbar
          .accounts=${this.accountOptions}
          .account=${this.params.get('account')}
          .period=${this.period}
          .search=${this.search}
          .matchCount=${searching ? visible.length : null}
          .totalCount=${this.rows.length}
          ?busy=${this.loading}
          @nc-account-change=${this.handleAccountChange}
          @nc-period-change=${this.handlePeriodChange}
          @nc-search-change=${this.handleSearchChange}
        ></wc-register-toolbar>
        ${this.renderHelp()}
      </div>

      ${this.loading
        ? html`<wc-spinner size="l" show-label label="Loading the register"></wc-spinner>`
        : nothing}
      ${this.error
        ? html`
            <wc-empty-state
              icon="wc-icon-register"
              heading="Could not load the register"
              message=${this.error}
            >
              <wa-button slot="actions" @click=${this.retry}>Retry</wa-button>
            </wc-empty-state>
          `
        : nothing}
      ${this.loading || this.error
        ? nothing
        : html`
            <wc-register-table
              .rows=${visible}
              .categories=${this.categoryOptions}
              .selectedId=${this.selectedId}
              .editingId=${this.editingId}
              .busyId=${this.busyId}
              .total=${this.total}
              .showAccount=${!this.params.get('account')}
              footer-note=${searching
                ? `${visible.length} of ${this.rows.length} rows shown`
                : ''}
              empty-message=${searching
                ? 'No transactions match this search.'
                : 'No transactions in this period.'}
              @nc-row-select=${this.handleRowSelect}
              @nc-row-activate=${this.handleRowActivate}
              @nc-edit-commit=${this.handleEditCommit}
              @nc-edit-cancel=${this.handleEditCancel}
              @nc-flag-toggle=${this.handleFlagToggle}
              @nc-search-focus=${this.handleSearchFocus}
            ></wc-register-table>
          `}
    `;
  }

  private retry = (): void => {
    void this.load(registerParamsFrom(this.params));
  };

  private renderHelp(): TemplateResult {
    return html`
      <details class="help">
        <summary>Keyboard</summary>
        <dl>
          <dt>↑ ↓</dt>
          <dd>Move between rows</dd>
          <dt>PgUp PgDn</dt>
          <dd>Move a screenful</dd>
          <dt>Home End</dt>
          <dd>First or last row</dd>
          <dt>Enter</dt>
          <dd>Edit the category and vendor</dd>
          <dt>Esc</dt>
          <dd>Cancel the edit</dd>
          <dt>f</dt>
          <dd>Flag or unflag the row</dd>
          <dt>/</dt>
          <dd>Jump to the search box</dd>
        </dl>
      </details>
    `;
  }
}

export function renderRegister(ctx: ScreenContext): TemplateResult {
  return html`
    <nigel-register-screen
      .client=${ctx.client}
      .params=${ctx.params}
      .navigate=${ctx.navigate}
    ></nigel-register-screen>
  `;
}

declare global {
  interface HTMLElementTagNameMap {
    'nigel-register-screen': NigelRegisterScreen;
  }
}
