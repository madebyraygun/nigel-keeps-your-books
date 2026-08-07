import { LitElement, html, css, nothing } from 'lit';
import { customElement, property } from 'lit/decorators.js';
import './wc-spinner.js';
import { roundHalfEven } from './round-half-even.js';

/** One period's pair of bars. Both figures are positive magnitudes. */
export interface BarBucket {
  label: string;
  income: number;
  expense: number;
}

/** A bar's height as a percentage of the tallest bar in the set. */
export interface BarHeights {
  income: number;
  expense: number;
}

/**
 * Bar heights as percentages of the largest figure anywhere in the set.
 *
 * Exported because it is the whole of the chart's arithmetic and deserves to be
 * tested without a DOM. An all-zero set scales to zero rather than dividing by
 * it, which draws a flat baseline — the honest picture of a period with no
 * money moving.
 */
export function barHeights(buckets: BarBucket[]): BarHeights[] {
  const max = buckets.reduce(
    (peak, b) => Math.max(peak, b.income, b.expense),
    0,
  );
  if (max <= 0) return buckets.map(() => ({ income: 0, expense: 0 }));
  return buckets.map((b) => ({
    income: (b.income / max) * 100,
    expense: (b.expense / max) * 100,
  }));
}

/**
 * Twelve months of income against expenses, drawn in CSS.
 *
 * No chart library: two divs with percentage heights say everything a
 * dashboard bar chart needs to, and a dependency that ships its own canvas
 * renderer would also ship its own colours, fonts and focus rings to fight with
 * the token layer.
 *
 * Accessibility is the reason for the hidden table. Bars are a picture, so the
 * chart names itself with `role="img"`, and the numbers behind it are repeated
 * as a real table that only assistive tech reads — a screen reader gets the
 * figures rather than a description of some rectangles.
 */
@customElement('wc-bar-chart')
export class WcBarChart extends LitElement {
  static styles = css`
    :host {
      display: block;
      font-family: var(--wa-font-family-sans);
      color: var(--wa-color-text);
      --nc-bar-chart-height: 180px;
    }

    .caption {
      margin: 0 0 var(--wa-space-xs, 6px);
      font-size: var(--wa-font-size-s, 13px);
      color: var(--wa-color-muted);
    }

    .plot {
      display: flex;
      align-items: flex-end;
      gap: var(--wa-space-s, 8px);
      height: var(--nc-bar-chart-height);
      padding-bottom: var(--wa-space-xs, 6px);
      border-bottom: 1px solid var(--wa-color-border);
    }

    .bucket {
      /* Grow to share the width, but never balloon: a three-month history
         should read as three months, not as three slabs. */
      flex: 1 1 0;
      max-width: 72px;
      display: flex;
      align-items: flex-end;
      justify-content: center;
      gap: 2px;
      height: 100%;
    }

    .bar {
      width: 44%;
      min-height: 1px;
      border-radius: var(--wa-radius-sm, 4px) var(--wa-radius-sm, 4px) 0 0;
      transition: height var(--nc-transition-fast, 120ms ease);
    }

    .bar.income {
      background: var(--nc-color-income);
    }

    .bar.expense {
      background: var(--nc-color-expense);
    }

    .labels {
      display: flex;
      gap: var(--wa-space-s, 8px);
      margin-top: var(--wa-space-2xs, 4px);
    }

    .tick {
      flex: 1 1 0;
      max-width: 72px;
      text-align: center;
      font-size: var(--wa-font-size-s, 13px);
      color: var(--wa-color-muted);
    }

    .legend {
      display: flex;
      gap: var(--wa-space-m, 12px);
      margin-top: var(--wa-space-s, 8px);
      font-size: var(--wa-font-size-s, 13px);
      color: var(--wa-color-muted);
    }

    .key {
      display: inline-flex;
      align-items: center;
      gap: var(--wa-space-2xs, 4px);
    }

    .swatch {
      width: 10px;
      height: 10px;
      border-radius: 2px;
    }

    .swatch.income {
      background: var(--nc-color-income);
    }

    .swatch.expense {
      background: var(--nc-color-expense);
    }

    .error {
      margin: 0;
      font-size: var(--wa-font-size-s, 13px);
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

    /* Present to assistive tech, absent from the picture. */
    .sr-only {
      position: absolute;
      width: 1px;
      height: 1px;
      padding: 0;
      margin: -1px;
      overflow: hidden;
      clip: rect(0, 0, 0, 0);
      white-space: nowrap;
      border: 0;
    }
  `;

  @property({ attribute: false })
  buckets: BarBucket[] = [];

  /** The period the chart covers, e.g. "2025 - 26". */
  @property({ type: String })
  caption = '';

  @property({ type: String })
  currency = 'USD';

  /** Undefined uses the runtime's default locale. */
  @property({ type: String })
  locale?: string;

  @property({ type: Boolean, reflect: true })
  loading = false;

  @property({ type: String })
  error = '';

  private handleRetry = () => {
    this.dispatchEvent(
      new CustomEvent('nc-retry', { bubbles: true, composed: true }),
    );
  };

  private money(amount: number): string {
    const format = new Intl.NumberFormat(this.locale, {
      style: 'currency',
      currency: this.currency,
    });
    return format.format(
      roundHalfEven(amount, format.resolvedOptions().maximumFractionDigits ?? 2),
    );
  }

  /** What a screen reader hears before it reaches the table. */
  private get summary(): string {
    const income = this.buckets.reduce((sum, b) => sum + b.income, 0);
    const expense = this.buckets.reduce((sum, b) => sum + b.expense, 0);
    const period = this.caption ? ` for ${this.caption}` : '';
    return `Income and expenses by month${period}: ${this.money(
      income,
    )} in, ${this.money(expense)} out across ${this.buckets.length} months.`;
  }

  private renderTable() {
    return html`
      <table class="sr-only">
        <caption>
          ${this.caption || 'Monthly income and expenses'}
        </caption>
        <thead>
          <tr>
            <th scope="col">Month</th>
            <th scope="col">Income</th>
            <th scope="col">Expenses</th>
          </tr>
        </thead>
        <tbody>
          ${this.buckets.map(
            (b) => html`
              <tr>
                <th scope="row">${b.label}</th>
                <td>${this.money(b.income)}</td>
                <td>${this.money(b.expense)}</td>
              </tr>
            `,
          )}
        </tbody>
      </table>
    `;
  }

  render() {
    if (this.loading) {
      return html`<wc-spinner show-label label="Loading cash flow"></wc-spinner>`;
    }

    if (this.error) {
      return html`
        <p class="error">${this.error}</p>
        <button class="retry" type="button" @click=${this.handleRetry}>
          Retry
        </button>
      `;
    }

    if (this.buckets.length === 0) {
      return html`<slot name="empty"></slot>`;
    }

    const heights = barHeights(this.buckets);

    return html`
      ${this.caption ? html`<p class="caption">${this.caption}</p>` : nothing}
      <div class="chart" role="img" aria-label=${this.summary}>
        <div class="plot">
          ${this.buckets.map(
            (bucket, i) => html`
              <div class="bucket">
                <div
                  class="bar income"
                  style="height:${heights[i]?.income ?? 0}%"
                  title="${bucket.label} income ${this.money(bucket.income)}"
                ></div>
                <div
                  class="bar expense"
                  style="height:${heights[i]?.expense ?? 0}%"
                  title="${bucket.label} expenses ${this.money(bucket.expense)}"
                ></div>
              </div>
            `,
          )}
        </div>
        <div class="labels">
          ${this.buckets.map((b) => html`<span class="tick">${b.label}</span>`)}
        </div>
      </div>
      <div class="legend" aria-hidden="true">
        <span class="key"><span class="swatch income"></span>Income</span>
        <span class="key"><span class="swatch expense"></span>Expenses</span>
      </div>
      ${this.renderTable()}
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-bar-chart': WcBarChart;
  }
}
