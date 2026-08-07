import { LitElement, html, css } from 'lit';
import { customElement, property } from 'lit/decorators.js';
import './wc-empty-state.js';
import './wc-money.js';
import './wc-notice-bar.js';
import './wc-spinner.js';

/**
 * One recorded reconciliation, as `GET /api/reconciliations` returns it.
 *
 * Both balances and the timestamp are nullable because the columns are — a
 * record can predate either figure, and only a reconciled one is stamped.
 * Restated here rather than imported: `@nigel/ui` depends on `lit` alone.
 */
export interface ReconciliationHistoryRow {
  id: number;
  month: string;
  statementBalance: number | null;
  calculatedBalance: number | null;
  isReconciled: boolean;
  reconciledAt: string | null;
}

/**
 * Which months have been checked, and how they came out.
 *
 * Every attempt is here, including the ones that did not balance: the server
 * records a mismatch deliberately, because "we looked at March and it was
 * off by 128.56" is the fact worth keeping.
 */
@customElement('wc-reconciliation-history')
export class WcReconciliationHistory extends LitElement {
  static styles = css`
    :host {
      display: block;
      overflow-x: auto;
    }

    table {
      width: 100%;
      border-collapse: collapse;
      font-size: var(--wa-font-size-s, 13px);
      color: var(--wa-color-text);
      font-family: var(--wa-font-family-sans);
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
      text-align: start;
      padding: var(--wa-space-xs, 6px) var(--wa-space-s, 8px);
      border-bottom: 1px solid var(--wa-color-border);
      vertical-align: top;
    }

    th {
      font-weight: var(--wa-font-weight-medium, 500);
      color: var(--wa-color-muted);
      white-space: nowrap;
    }

    td.amount,
    th.amount {
      text-align: end;
    }

    td.month {
      font-variant-numeric: tabular-nums;
      white-space: nowrap;
    }

    .muted {
      color: var(--wa-color-muted);
    }

    .status {
      white-space: nowrap;
    }

    .status.ok {
      color: var(--wa-color-success);
    }

    .status.off {
      color: var(--wa-color-danger);
    }

    .loading {
      display: grid;
      place-items: center;
      padding: var(--wa-space-l, 16px);
    }
  `;

  @property({ attribute: false })
  rows: ReconciliationHistoryRow[] = [];

  @property({ type: Boolean, reflect: true })
  loading = false;

  @property({ attribute: false })
  error: string | null = null;

  private handleRetry = (): void => {
    this.dispatchEvent(new CustomEvent('nc-retry', { bubbles: true, composed: true }));
  };

  render() {
    if (this.error) {
      return html`
        <wc-notice-bar
          variant="danger"
          message=${this.error}
          action-label="Try again"
          @nc-notice-action=${this.handleRetry}
        ></wc-notice-bar>
      `;
    }

    if (this.loading) {
      return html`
        <div class="loading">
          <wc-spinner show-label label="Loading reconciliations"></wc-spinner>
        </div>
      `;
    }

    if (this.rows.length === 0) {
      return html`
        <wc-empty-state
          compact
          icon="wc-icon-reconcile"
          heading="No reconciliations yet"
          message="Checking a month against a statement records it here."
        ></wc-empty-state>
      `;
    }

    return html`
      <table>
        <caption>
          Past reconciliations
        </caption>
        <thead>
          <tr>
            <th scope="col">Month</th>
            <th scope="col" class="amount">Statement</th>
            <th scope="col" class="amount">Calculated</th>
            <th scope="col">Result</th>
            <th scope="col">Checked</th>
          </tr>
        </thead>
        <tbody>
          ${this.rows.map((row) => this.renderRow(row))}
        </tbody>
      </table>
    `;
  }

  private renderRow(row: ReconciliationHistoryRow) {
    return html`
      <tr data-row=${row.id}>
        <td class="month">${row.month}</td>
        <td class="amount">${amount(row.statementBalance)}</td>
        <td class="amount">${amount(row.calculatedBalance)}</td>
        <td>
          <span class=${`status ${row.isReconciled ? 'ok' : 'off'}`}>
            ${row.isReconciled ? '✓ Reconciled' : '✗ Discrepancy'}
          </span>
        </td>
        <td class=${row.reconciledAt === null ? 'muted' : ''}>
          ${row.reconciledAt ?? '—'}
        </td>
      </tr>
    `;
  }
}

/**
 * A nullable balance. An em dash rather than `$0.00`, because a record that
 * predates the column did not reconcile to nothing — it has no figure at all.
 */
function amount(value: number | null) {
  return value === null
    ? html`<span class="muted">—</span>`
    : html`<wc-money .amount=${value} variant="plain" align="end"></wc-money>`;
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-reconciliation-history': WcReconciliationHistory;
  }
}
