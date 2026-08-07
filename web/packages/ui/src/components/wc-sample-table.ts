import { LitElement, html, css, nothing } from 'lit';
import { customElement, property } from 'lit/decorators.js';
import './wc-money.js';

/** One parsed row, as an import preview reports it. */
export interface SampleTableRow {
  date: string;
  description: string;
  amount: number;
}

/**
 * The first few rows of a statement, exactly as they were parsed.
 *
 * Not `wc-register-table`, which is a `role="grid"` with a roving tabindex,
 * inline category editing and flag toggles, and needs an `id`, a `categoryId`
 * and an `isFlagged` on every row. A previewed row has none of those — it has
 * not been written yet and has no identity to edit. Bending the grid to render
 * three read-only columns would mean disabling most of what it is.
 */
@customElement('wc-sample-table')
export class WcSampleTable extends LitElement {
  static styles = css`
    :host {
      display: block;
      font-family: var(--wa-font-family-sans);
      color: var(--wa-color-text);
    }

    table {
      width: 100%;
      border-collapse: collapse;
      font-size: var(--wa-font-size-s, 13px);
    }

    caption {
      text-align: start;
      color: var(--wa-color-muted);
      padding-bottom: var(--wa-space-xs, 6px);
    }

    th,
    td {
      text-align: start;
      padding: var(--wa-space-xs, 6px) var(--wa-space-s, 8px);
      border-bottom: 1px solid var(--wa-color-border);
    }

    :host([dense]) th,
    :host([dense]) td {
      padding: var(--wa-space-2xs, 4px) var(--wa-space-s, 8px);
    }

    thead th {
      color: var(--wa-color-muted);
      font-weight: var(--wa-font-weight-medium, 500);
      white-space: nowrap;
    }

    .date {
      font-family: var(--wa-font-family-mono);
      white-space: nowrap;
      width: 1%;
    }

    .description {
      overflow-wrap: anywhere;
    }

    .amount {
      text-align: end;
      white-space: nowrap;
      width: 1%;
    }

    .empty {
      margin: 0;
      color: var(--wa-color-muted);
    }
  `;

  @property({ attribute: false })
  rows: SampleTableRow[] = [];

  @property({ type: String })
  caption = '';

  @property({ type: String, attribute: 'empty-message' })
  emptyMessage = 'No rows to show.';

  @property({ type: Boolean, reflect: true })
  dense = false;

  render() {
    if (this.rows.length === 0) {
      return html`<p class="empty">${this.emptyMessage}</p>`;
    }

    return html`
      <table>
        ${this.caption ? html`<caption>${this.caption}</caption>` : nothing}
        <thead>
          <tr>
            <th scope="col" class="date">Date</th>
            <th scope="col" class="description">Description</th>
            <th scope="col" class="amount">Amount</th>
          </tr>
        </thead>
        <tbody>
          ${this.rows.map(
            (row) => html`
              <tr>
                <td class="date">${row.date}</td>
                <td class="description">${row.description}</td>
                <td class="amount">
                  <wc-money .amount=${row.amount} align="end"></wc-money>
                </td>
              </tr>
            `,
          )}
        </tbody>
      </table>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-sample-table': WcSampleTable;
  }
}
