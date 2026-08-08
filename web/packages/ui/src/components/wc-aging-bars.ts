import { LitElement, html, css, nothing } from 'lit';
import { customElement, property } from 'lit/decorators.js';
import './wc-money.js';

/** One aging bucket, in the shape `ar_aging` reports it. */
export interface AgingBucketView {
  label: string;
  count: number;
  total: number;
}

/**
 * Bar heights as percentages of the largest bucket.
 *
 * Relative to the largest rather than to the total, so a single non-empty
 * bucket still draws a full bar instead of one that says "100% of everything"
 * in a bar the reader has to compare against nothing. All-zero draws nothing.
 */
export function agingBarHeights(buckets: AgingBucketView[]): number[] {
  const largest = Math.max(0, ...buckets.map((bucket) => Math.abs(bucket.total)));
  if (largest === 0) return buckets.map(() => 0);
  return buckets.map((bucket) => Math.round((Math.abs(bucket.total) / largest) * 100));
}

/**
 * The A/R aging strip: five buckets, their bars, and the outstanding total.
 *
 * The bars are decoration over a real `<table>` — the same arrangement
 * `wc-bar-chart` uses, and for the same reason: a bar has no accessible value,
 * so the figures live in a table and the bars sit beside them with
 * `aria-hidden`. Every figure this component shows is a second view of the
 * aging report, which is why the parity walk skips it whole.
 */
@customElement('wc-aging-bars')
export class WcAgingBars extends LitElement {
  static styles = css`
    :host {
      display: block;
      font-family: var(--wa-font-family-sans);
      color: var(--wa-color-text);
    }

    .frame {
      border: 1px solid var(--wa-color-border);
      border-radius: var(--wa-radius-m, 8px);
      padding: var(--wa-space-m, 12px);
    }

    header {
      display: flex;
      flex-wrap: wrap;
      align-items: baseline;
      justify-content: space-between;
      gap: var(--wa-space-s, 8px);
      margin-bottom: var(--wa-space-s, 8px);
    }

    h3 {
      margin: 0;
      font-size: var(--wa-font-size-s, 13px);
      font-weight: var(--wa-font-weight-medium, 500);
      text-transform: uppercase;
      letter-spacing: 0.04em;
      color: var(--wa-color-muted);
    }

    .as-of {
      color: var(--wa-color-muted);
      font-size: var(--wa-font-size-s, 13px);
    }

    a {
      color: var(--wa-color-brand);
      font-size: var(--wa-font-size-s, 13px);
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
      padding: var(--wa-space-2xs, 4px) var(--wa-space-s, 8px);
      text-align: start;
      border: 0;
    }

    th {
      color: var(--wa-color-muted);
      font-weight: var(--wa-font-weight-medium, 500);
      white-space: nowrap;
    }

    td.amount {
      text-align: end;
    }

    tr.total td,
    tr.total th {
      border-top: 2px solid var(--wa-color-border);
      font-weight: var(--wa-font-weight-bold, 700);
    }

    .bars {
      display: grid;
      grid-auto-flow: column;
      grid-auto-columns: 1fr;
      align-items: end;
      gap: var(--wa-space-xs, 6px);
      height: 44px;
      margin-top: var(--wa-space-s, 8px);
    }

    .bar {
      background: var(--wa-color-brand, #2f6feb);
      border-radius: var(--wa-radius-sm, 4px) var(--wa-radius-sm, 4px) 0 0;
      min-height: 2px;
      opacity: 0.75;
    }

    .bar[data-empty='true'] {
      background: var(--wa-color-border);
      opacity: 1;
    }

    .empty {
      margin: 0;
      color: var(--wa-color-muted);
      font-size: var(--wa-font-size-s, 13px);
    }
  `;

  @property({ attribute: false })
  buckets: AgingBucketView[] = [];

  /** The outstanding total across the buckets. */
  @property({ type: Number })
  total = 0;

  /** `YYYY-MM-DD`, rendered in the heading the way the report titles it. */
  @property({ type: String, attribute: 'as-of' })
  asOf = '';

  /** Where "View aging report" points. Empty hides the link. */
  @property({ type: String })
  href = '';

  render() {
    if (this.buckets.length === 0) {
      return html`<div class="frame">
        <p class="empty">No open receivables.</p>
      </div>`;
    }

    const heights = agingBarHeights(this.buckets);

    return html`
      <div class="frame">
        <header>
          <h3>
            A/R aging
            ${this.asOf ? html`<span class="as-of">— as of ${this.asOf}</span>` : nothing}
          </h3>
          ${this.href
            ? html`<a href=${this.href}>View aging report →</a>`
            : nothing}
        </header>

        <div class="scroll">
          <table>
            <caption>
              Accounts receivable aging${this.asOf ? ` as of ${this.asOf}` : ''}
            </caption>
            <thead>
              <tr>
                ${this.buckets.map(
                  (bucket) => html`<th scope="col">${bucket.label}</th>`,
                )}
                <th scope="col">Total</th>
              </tr>
            </thead>
            <tbody>
              <tr>
                ${this.buckets.map(
                  (bucket) => html`
                    <td class="amount">
                      <wc-money .amount=${bucket.total} variant="plain"></wc-money>
                    </td>
                  `,
                )}
                <td class="amount">
                  <wc-money .amount=${this.total} variant="plain"></wc-money>
                </td>
              </tr>
            </tbody>
          </table>
        </div>

        <div class="bars" aria-hidden="true">
          ${heights.map(
            (height) => html`
              <div
                class="bar"
                data-empty=${height === 0 ? 'true' : 'false'}
                style="height:${Math.max(height, 2)}%"
              ></div>
            `,
          )}
        </div>
      </div>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-aging-bars': WcAgingBars;
  }
}
