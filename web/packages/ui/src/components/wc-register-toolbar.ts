import { LitElement, html, css, nothing } from 'lit';
import { customElement, property, query } from 'lit/decorators.js';
import '@awesome.me/webawesome/dist/components/select/select.js';
import '@awesome.me/webawesome/dist/components/option/option.js';
import '@awesome.me/webawesome/dist/components/input/input.js';
import '../icons/icons.js';
import './wc-period-nav.js';
import type { NcDateGranularity, NcPeriod } from './wc-period-nav.js';

/** An account the register can be filtered to. */
export interface AccountOption {
  id: number;
  name: string;
}

export interface NcAccountChangeDetail {
  /** Null is "all accounts" — the register's unfiltered default. */
  account: string | null;
}

export interface NcSearchChangeDetail {
  query: string;
}

/** The value `wa-select` carries for "all accounts"; empty is not selectable. */
const ALL_ACCOUNTS = '__all__';

/**
 * Filters above the register: account, period, and incremental search.
 *
 * Search is client-side — `/api/reports/register` has no search parameter and
 * the TUI filters in memory too — so there is no debounce. A keystroke is a
 * pass over an array that is already loaded, and waiting would only add
 * latency to something the TUI does instantly.
 */
@customElement('wc-register-toolbar')
export class WcRegisterToolbar extends LitElement {
  static styles = css`
    :host {
      display: flex;
      flex-wrap: wrap;
      align-items: end;
      gap: var(--wa-space-m, 12px);
      font-family: var(--wa-font-family-sans);
      color: var(--wa-color-text);
    }

    wa-select {
      min-width: 14rem;
    }

    .search {
      min-width: 16rem;
      flex: 1 1 16rem;
    }

    .status {
      font-size: var(--wa-font-size-s, 13px);
      color: var(--wa-color-muted);
      padding-bottom: var(--wa-space-2xs, 4px);
      white-space: nowrap;
    }
  `;

  @property({ attribute: false })
  accounts: AccountOption[] = [];

  /** The account name the register is filtered to, or null for all. */
  @property({ type: String })
  account: string | null = null;

  @property({ attribute: false })
  period: NcPeriod = { kind: 'all' };

  @property({ type: String })
  granularity: NcDateGranularity = 'monthAndYear';

  @property({ type: String })
  search = '';

  /** Rows matching the search. Null when nothing is being searched for. */
  @property({ type: Number, attribute: 'match-count' })
  matchCount: number | null = null;

  @property({ type: Number, attribute: 'total-count' })
  totalCount = 0;

  @property({ type: Boolean, reflect: true })
  busy = false;

  @query('.search') private searchInput?: HTMLElement & { value: string };

  /** Focus the search box — what the table asks for when `/` is pressed. */
  focusSearch(): void {
    this.searchInput?.focus();
  }

  private handleAccountChange(event: Event): void {
    const value = (event.target as HTMLElement & { value: string }).value;
    this.dispatchEvent(
      new CustomEvent<NcAccountChangeDetail>('nc-account-change', {
        detail: { account: value === ALL_ACCOUNTS ? null : value },
        bubbles: true,
        composed: true,
      }),
    );
  }

  private handleSearchInput(event: Event): void {
    const value = (event.target as HTMLElement & { value: string }).value ?? '';
    this.dispatchEvent(
      new CustomEvent<NcSearchChangeDetail>('nc-search-change', {
        detail: { query: value },
        bubbles: true,
        composed: true,
      }),
    );
  }

  private get statusText(): string {
    if (this.matchCount === null) {
      return this.totalCount === 1 ? '1 row' : `${this.totalCount} rows`;
    }
    if (this.matchCount === 0) return 'No matches';
    return `${this.matchCount} of ${this.totalCount} rows`;
  }

  render() {
    return html`
      <wa-select
        label="Account"
        size="s"
        value=${this.account ?? ALL_ACCOUNTS}
        ?disabled=${this.busy}
        @change=${this.handleAccountChange}
      >
        <wa-option value=${ALL_ACCOUNTS}>All accounts</wa-option>
        ${this.accounts.map(
          (account) => html`<wa-option value=${account.name}>${account.name}</wa-option>`,
        )}
      </wa-select>

      ${this.granularity === 'none'
        ? nothing
        : html`
            <wc-period-nav
              allow-all
              granularity=${this.granularity}
              .period=${this.period}
              ?disabled=${this.busy}
            ></wc-period-nav>
          `}

      <wa-input
        class="search"
        type="search"
        label="Search"
        size="s"
        placeholder="Description, vendor or category"
        value=${this.search}
        ?disabled=${this.busy}
        @input=${this.handleSearchInput}
      >
        <wc-icon-search slot="start"></wc-icon-search>
      </wa-input>

      <p class="status" role="status" aria-live="polite">${this.statusText}</p>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-register-toolbar': WcRegisterToolbar;
  }

  interface HTMLElementEventMap {
    'nc-account-change': CustomEvent<NcAccountChangeDetail>;
    'nc-search-change': CustomEvent<NcSearchChangeDetail>;
  }
}
