import { LitElement, html, css, nothing } from 'lit';
import { customElement, property } from 'lit/decorators.js';
import './wc-money.js';
import './wc-spinner.js';
import './wc-invoice-status.js';

/** One invoice as the list shows it. */
export interface InvoiceTableRow {
  number: number;
  status: string;
  /** Null when the client row is gone — the join is a LEFT JOIN. */
  clientName: string | null;
  total: number;
  /**
   * Outstanding. Null renders as an em dash, which is what a void invoice
   * gets: it owes nothing and it never will, and `$0.00` would read as settled.
   */
  balance: number | null;
  dueDate: string | null;
  /** Where the row links. Empty makes the row inert text. */
  href?: string;
}

/**
 * The invoice list — `nigel invoice list` plus the balance the TUI's own list
 * carries.
 *
 * Not `wc-report-table`: that renders section and total bands for read-only
 * financial output, and these rows carry a status chip and a link. Not
 * `wc-manager-table` either — an invoice is edited on its own screen, never
 * from a row button, because void and send are not row-level affordances.
 */
@customElement('wc-invoice-table')
export class WcInvoiceTable extends LitElement {
  static styles = css`
    :host {
      display: block;
      font-family: var(--wa-font-family-sans);
      color: var(--wa-color-text);
    }

    .scroll {
      overflow-x: auto;
    }

    table {
      width: 100%;
      border-collapse: collapse;
      font-size: var(--wa-font-size-s, 13px);
    }

    caption {
      position: absolute;
      width: 1px;
      height: 1px;
      overflow: hidden;
      clip-path: inset(50%);
      white-space: nowrap;
    }

    th,
    td {
      padding: var(--wa-space-xs, 6px) var(--wa-space-s, 8px);
      border-bottom: 1px solid var(--wa-color-border);
      text-align: start;
      vertical-align: baseline;
      white-space: nowrap;
    }

    th {
      color: var(--wa-color-muted);
      font-weight: var(--wa-font-weight-medium, 500);
    }

    td.end,
    th.end {
      text-align: end;
    }

    td.number {
      font-variant-numeric: tabular-nums;
    }

    td.client {
      white-space: normal;
    }

    .muted {
      color: var(--wa-color-muted);
    }

    a {
      color: inherit;
      text-decoration: none;
    }

    a:hover {
      text-decoration: underline;
    }

    a:focus-visible {
      outline: 2px solid var(--wa-color-focus);
      outline-offset: 2px;
    }

    .state {
      padding: var(--wa-space-l, 16px) 0;
      color: var(--wa-color-muted);
      font-size: var(--wa-font-size-s, 13px);
    }
  `;

  @property({ attribute: false })
  rows: InvoiceTableRow[] = [];

  @property({ type: Boolean, reflect: true })
  loading = false;

  @property({ type: String })
  caption = 'Invoices';

  @property({ type: String, attribute: 'empty-message' })
  emptyMessage = 'No invoices yet.';

  render() {
    if (this.loading) {
      return html`<div class="state">
        <wc-spinner show-label label="Loading invoices"></wc-spinner>
      </div>`;
    }

    if (this.rows.length === 0) {
      return html`<p class="state" data-empty>${this.emptyMessage}</p>`;
    }

    return html`
      <div class="scroll">
        <table>
          <caption>
            ${this.caption}
          </caption>
          <thead>
            <tr>
              <th scope="col">#</th>
              <th scope="col">Status</th>
              <th scope="col">Client</th>
              <th scope="col" class="end">Total</th>
              <th scope="col" class="end">Balance</th>
              <th scope="col">Due</th>
            </tr>
          </thead>
          <tbody>
            ${this.rows.map((row) => this.renderRow(row))}
          </tbody>
        </table>
      </div>
    `;
  }

  private renderRow(row: InvoiceTableRow) {
    const number = row.href
      ? html`<a href=${row.href}>${row.number}</a>`
      : html`${row.number}`;

    return html`
      <tr data-row=${row.number}>
        <td class="number">${number}</td>
        <td><wc-invoice-status status=${row.status}></wc-invoice-status></td>
        <td class="client ${row.clientName === null ? 'muted' : ''}">
          ${row.clientName ?? '—'}
        </td>
        <td class="end">
          <wc-money .amount=${row.total} variant="plain" align="end"></wc-money>
        </td>
        <td class="end ${row.balance === null ? 'muted' : ''}" data-balance>
          ${row.balance === null
            ? '—'
            : html`<wc-money
                .amount=${row.balance}
                variant="plain"
                align="end"
              ></wc-money>`}
        </td>
        <td class=${row.dueDate === null ? 'muted' : nothing}>${row.dueDate ?? '—'}</td>
      </tr>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-invoice-table': WcInvoiceTable;
  }
}
