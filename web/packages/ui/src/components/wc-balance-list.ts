import { LitElement, html, css, nothing } from 'lit';
import { customElement, property } from 'lit/decorators.js';
import './wc-money.js';
import './wc-spinner.js';

/** One account's cash position. */
export interface BalanceRow {
  name: string;
  accountType?: string;
  balance: number;
}

/**
 * Account balances as a table — the dashboard's counterpart to the TUI's
 * balances panel.
 *
 * A real `<table>` rather than a grid of divs: these are rows of data with
 * headers, and a screen reader reading "BofA Checking, Balance, $4,928.01"
 * needs the header association to say the second part.
 */
@customElement('wc-balance-list')
export class WcBalanceList extends LitElement {
  static styles = css`
    :host {
      display: block;
      font-family: var(--wa-font-family-sans);
      color: var(--wa-color-text);
    }

    table {
      width: 100%;
      border-collapse: collapse;
    }

    caption {
      text-align: left;
      font-size: var(--wa-font-size-s, 13px);
      font-weight: var(--wa-font-weight-medium, 500);
      color: var(--wa-color-muted);
      padding-bottom: var(--wa-space-xs, 6px);
    }

    th {
      text-align: left;
      font-size: var(--wa-font-size-s, 13px);
      font-weight: var(--wa-font-weight-medium, 500);
      color: var(--wa-color-muted);
      padding: var(--wa-space-2xs, 4px) 0;
      border-bottom: 1px solid var(--wa-color-border);
    }

    th.amount {
      text-align: right;
    }

    td {
      padding: var(--wa-space-xs, 6px) 0;
      border-bottom: 1px solid var(--wa-color-border-soft, var(--wa-color-border));
    }

    td.amount {
      text-align: right;
      width: 12ch;
    }

    .type {
      display: block;
      font-size: var(--wa-font-size-s, 13px);
      color: var(--wa-color-muted);
    }

    tfoot td {
      border-bottom: none;
      border-top: 2px solid var(--wa-color-border);
      font-weight: var(--wa-font-weight-bold, 700);
    }

    .empty,
    .error {
      margin: 0;
      font-size: var(--wa-font-size-s, 13px);
      color: var(--wa-color-muted);
    }

    .error {
      color: var(--wa-color-danger);
    }

    .retry {
      margin-top: var(--wa-space-xs, 6px);
      font: inherit;
      font-size: var(--wa-font-size-s, 13px);
      color: var(--wa-color-brand);
      background: none;
      border: 1px solid var(--wa-color-border);
      border-radius: var(--wa-radius-sm, 6px);
      padding: var(--wa-space-2xs, 4px) var(--wa-space-s, 8px);
      cursor: pointer;
    }

    .retry:focus-visible {
      outline: 2px solid var(--wa-color-focus);
      outline-offset: 2px;
    }
  `;

  @property({ attribute: false })
  items: BalanceRow[] = [];

  /** Rendered as a totals row. Omit it and no total is shown. */
  @property({ type: Number })
  total?: number;

  /** The table's accessible name. */
  @property({ type: String })
  caption = 'Account balances';

  @property({ type: String, attribute: 'empty-message' })
  emptyMessage = 'No accounts yet.';

  @property({ type: Boolean, reflect: true })
  loading = false;

  @property({ type: String })
  error = '';

  private handleRetry = () => {
    this.dispatchEvent(
      new CustomEvent('nc-retry', { bubbles: true, composed: true }),
    );
  };

  render() {
    if (this.loading) {
      return html`<wc-spinner show-label label="Loading balances"></wc-spinner>`;
    }

    if (this.error) {
      return html`
        <p class="error">${this.error}</p>
        <button class="retry" type="button" @click=${this.handleRetry}>
          Retry
        </button>
      `;
    }

    if (this.items.length === 0) {
      return html`<p class="empty">${this.emptyMessage}</p>`;
    }

    return html`
      <table>
        <caption>
          ${this.caption}
        </caption>
        <thead>
          <tr>
            <th scope="col">Account</th>
            <th scope="col" class="amount">Balance</th>
          </tr>
        </thead>
        <tbody>
          ${this.items.map(
            (item) => html`
              <tr>
                <th scope="row">
                  ${item.name}
                  ${item.accountType
                    ? html`<span class="type">${item.accountType}</span>`
                    : nothing}
                </th>
                <td class="amount">
                  <wc-money .amount=${item.balance} align="end"></wc-money>
                </td>
              </tr>
            `,
          )}
        </tbody>
        ${this.total === undefined
          ? nothing
          : html`
              <tfoot>
                <tr>
                  <th scope="row">Total</th>
                  <td class="amount">
                    <wc-money .amount=${this.total} align="end"></wc-money>
                  </td>
                </tr>
              </tfoot>
            `}
      </table>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-balance-list': WcBalanceList;
  }
}
