import { LitElement, html, css, nothing } from 'lit';
import { customElement, property } from 'lit/decorators.js';
import './wc-money.js';
import './wc-spinner.js';
import { roundHalfEven } from './round-half-even.js';

/**
 * How a cell is formatted, which also decides its default alignment.
 *
 * `money` keeps the sign and the income/expense colouring; `moneyAbs` prints
 * the magnitude in plain ink, which is the shape `cli/report/text.rs` uses for
 * expense and deduction columns where the column heading already says which
 * direction the money went.
 */
export type ReportCellKind = 'text' | 'money' | 'moneyAbs' | 'percent' | 'count';

export interface ReportColumn {
  key: string;
  label: string;
  kind: ReportCellKind;
  /** Defaults to `end` for every numeric kind. */
  align?: 'start' | 'end';
  width?: string;
}

/**
 * `section` is a heading row spanning the table (the INCOME/EXPENSES bands in
 * the P&L); `subtotal` and `total` are the ruled rows beneath a band.
 */
export type ReportRowEmphasis = 'normal' | 'section' | 'subtotal' | 'total';

export interface ReportTableRow {
  id?: string;
  cells: Record<string, string | number | null>;
  /**
   * Formatting for this row's cells, overriding the column's, keyed by column.
   *
   * The P&L needs it: `format_pnl` prints its income lines and its net signed
   * and its expense band as magnitudes, in one two-column table.
   */
  cellKinds?: Record<string, ReportCellKind>;
  emphasis?: ReportRowEmphasis;
  tone?: 'income' | 'expense' | 'neutral';
  indent?: 0 | 1;
  /** Makes the whole row a link — the flagged report points at the review screen. */
  href?: string;
  /** A quiet note after the first cell, e.g. the K-1 worksheet's "(50%)". */
  note?: string;
}

const NUMERIC: ReportCellKind[] = ['money', 'moneyAbs', 'percent', 'count'];

/**
 * Every name-and-amount section of every report.
 *
 * Eight reports could have meant eight table components, eight axe suites and
 * eight places for a column to drift out of step with the CLI. They are all the
 * same shape underneath — `text.rs` builds each of them out of comfy_table rows
 * — so this takes the shape as data and the report screens keep their
 * per-report knowledge in pure mapper functions instead of in markup.
 *
 * The model is deliberately closed: no per-cell slots, no render callbacks. A
 * table that can be handed arbitrary templates is a rendering framework, and
 * the point of putting it here was to have one set of table manners.
 */
@customElement('wc-report-table')
export class WcReportTable extends LitElement {
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
      font-size: var(--wa-font-size-m, 15px);
    }

    caption {
      text-align: start;
      font-weight: var(--wa-font-weight-medium, 500);
      padding-bottom: var(--wa-space-xs, 6px);
    }

    caption.visually-hidden {
      position: absolute;
      width: 1px;
      height: 1px;
      padding: 0;
      margin: -1px;
      overflow: hidden;
      clip: rect(0 0 0 0);
      white-space: nowrap;
      border: 0;
    }

    th,
    td {
      padding: var(--wa-space-xs, 6px) var(--wa-space-s, 8px);
      border-bottom: 1px solid var(--wa-color-border);
      text-align: start;
      vertical-align: baseline;
    }

    th {
      font-size: var(--wa-font-size-s, 13px);
      font-weight: var(--wa-font-weight-medium, 500);
      color: var(--wa-color-muted);
      text-transform: uppercase;
      letter-spacing: 0.04em;
      white-space: nowrap;
    }

    th.end,
    td.end {
      text-align: end;
    }

    :host([dense]) th,
    :host([dense]) td {
      padding: var(--wa-space-2xs, 4px) var(--wa-space-xs, 6px);
    }

    tr[data-emphasis='section'] td {
      font-weight: var(--wa-font-weight-bold, 700);
      text-transform: uppercase;
      letter-spacing: 0.04em;
      font-size: var(--wa-font-size-s, 13px);
      padding-top: var(--wa-space-m, 12px);
      border-bottom-color: transparent;
    }

    tr[data-emphasis='subtotal'] td,
    tr[data-emphasis='total'] td {
      font-weight: var(--wa-font-weight-medium, 500);
    }

    tr[data-emphasis='total'] td {
      font-weight: var(--wa-font-weight-bold, 700);
      border-top: 2px solid var(--wa-color-border);
    }

    tr[data-tone='income'] td.label {
      color: var(--nc-color-income, #1a7f5a);
    }

    tr[data-tone='expense'] td.label {
      color: var(--nc-color-expense, #b3261e);
    }

    td.indent-1 {
      padding-inline-start: var(--wa-space-l, 16px);
    }

    tr[data-link] {
      cursor: pointer;
    }

    tr[data-link]:hover td {
      background: var(--wa-color-surface-alt, rgba(0, 0, 0, 0.03));
    }

    td a {
      color: inherit;
      text-decoration: none;
    }

    td a:hover {
      text-decoration: underline;
    }

    td a:focus-visible {
      outline: 2px solid var(--wa-color-focus);
      outline-offset: 2px;
    }

    .note {
      color: var(--wa-color-muted);
      font-size: var(--wa-font-size-s, 13px);
    }

    .state {
      padding: var(--wa-space-l, 16px) 0;
      color: var(--wa-color-muted);
      font-size: var(--wa-font-size-s, 13px);
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
  columns: ReportColumn[] = [];

  @property({ attribute: false })
  rows: ReportTableRow[] = [];

  /** The table's accessible name; always rendered, visibly unless hidden. */
  @property({ type: String })
  caption = '';

  @property({ type: Boolean, attribute: 'caption-hidden' })
  captionHidden = false;

  @property({ type: Boolean, reflect: true })
  dense = false;

  @property({ type: Boolean, reflect: true })
  loading = false;

  @property({ type: String })
  error = '';

  @property({ type: String, attribute: 'empty-message' })
  emptyMessage = 'Nothing to show for this period.';

  /** Locale for the money columns; undefined uses the runtime default. */
  @property({ type: String })
  locale?: string;

  private handleRetry = () => {
    this.dispatchEvent(new CustomEvent('nc-retry', { bubbles: true, composed: true }));
  };

  private alignOf(column: ReportColumn): 'start' | 'end' {
    return column.align ?? (NUMERIC.includes(column.kind) ? 'end' : 'start');
  }

  private static kindOf(row: ReportTableRow, column: ReportColumn): ReportCellKind {
    return row.cellKinds?.[column.key] ?? column.kind;
  }

  private renderValue(kind: ReportCellKind, value: string | number | null) {
    if (value === null || value === undefined || value === '') return nothing;

    switch (kind) {
      case 'money':
        return html`<wc-money
          .amount=${Number(value)}
          .locale=${this.locale}
          align="end"
        ></wc-money>`;
      case 'moneyAbs':
        return html`<wc-money
          .amount=${Math.abs(Number(value))}
          .locale=${this.locale}
          variant="plain"
          align="end"
        ></wc-money>`;
      case 'percent':
        // `{:.1}%`, tie and all — see `roundHalfEven`.
        return `${roundHalfEven(Number(value), 1).toFixed(1)}%`;
      case 'count':
        return String(value);
      default:
        return String(value);
    }
  }

  private renderRow(row: ReportTableRow, index: number) {
    const emphasis = row.emphasis ?? 'normal';
    const [first, ...rest] = this.columns;
    if (!first) return nothing;

    // A section heading is one label across the table, exactly as the text
    // report prints it: a band title with no figure of its own.
    if (emphasis === 'section') {
      return html`
        <tr data-emphasis="section" data-tone=${row.tone ?? nothing}>
          <td class="label" colspan=${this.columns.length}>
            ${this.renderValue(
              WcReportTable.kindOf(row, first),
              row.cells[first.key] ?? null,
            )}
          </td>
        </tr>
      `;
    }

    // The separating space is a text node, not a margin: a screen reader reads
    // the text, and "Meals(50%)" is not what the report says.
    const label = html`${this.renderValue(
      WcReportTable.kindOf(row, first),
      row.cells[first.key] ?? null,
    )}${row.note ? html`<span class="note"> ${row.note}</span>` : nothing}`;

    return html`
      <tr
        data-emphasis=${emphasis}
        data-tone=${row.tone ?? nothing}
        data-link=${row.href ? 'true' : nothing}
        data-index=${index}
      >
        <td
          class="label ${this.alignOf(first) === 'end' ? 'end' : ''} indent-${row.indent ?? 0}"
        >
          ${row.href ? html`<a href=${row.href}>${label}</a>` : label}
        </td>
        ${rest.map(
          (column) => html`
            <td class=${this.alignOf(column) === 'end' ? 'end' : ''}>
              ${this.renderValue(
                WcReportTable.kindOf(row, column),
                row.cells[column.key] ?? null,
              )}
            </td>
          `,
        )}
      </tr>
    `;
  }

  render() {
    if (this.loading) {
      return html`<div class="state">
        <wc-spinner show-label label=${`Loading ${this.caption || 'report'}`}></wc-spinner>
      </div>`;
    }

    if (this.error) {
      return html`
        <div class="state">
          <p class="error">${this.error}</p>
          <button class="retry" type="button" @click=${this.handleRetry}>Retry</button>
        </div>
      `;
    }

    if (this.rows.length === 0) {
      return html`<p class="state">${this.emptyMessage}</p>`;
    }

    return html`
      <div class="scroll">
        <table>
          <caption class=${this.captionHidden ? 'visually-hidden' : ''}>
            ${this.caption}
          </caption>
          <thead>
            <tr>
              ${this.columns.map(
                (column) => html`
                  <th
                    scope="col"
                    class=${this.alignOf(column) === 'end' ? 'end' : ''}
                    style=${column.width ? `width:${column.width}` : nothing}
                  >
                    ${column.label}
                  </th>
                `,
              )}
            </tr>
          </thead>
          <tbody>
            ${this.rows.map((row, index) => this.renderRow(row, index))}
          </tbody>
        </table>
      </div>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-report-table': WcReportTable;
  }
}
