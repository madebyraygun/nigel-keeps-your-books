import { LitElement, html, css, nothing } from 'lit';
import { customElement, property } from 'lit/decorators.js';

export type CountEmphasis = 'default' | 'good' | 'warn';

export interface CountItem {
  label: string;
  value: number;
  emphasis?: CountEmphasis;
  /** Secondary line under the number, for anything the label cannot carry. */
  hint?: string;
}

/**
 * A row of labelled whole numbers: "42 imported, 3 skipped, 1 malformed".
 *
 * Deliberately not `wc-stat-card`, which is a money figure — it formats through
 * `Intl.NumberFormat` with a currency style, and a count of malformed rows is
 * not $1.00. Two components rather than one prop on the money card, because the
 * two have nothing in common past "a number with a label above it".
 */
@customElement('wc-count-grid')
export class WcCountGrid extends LitElement {
  static styles = css`
    :host {
      display: block;
      font-family: var(--wa-font-family-sans);
      color: var(--wa-color-text);
    }

    dl {
      display: flex;
      flex-wrap: wrap;
      gap: var(--wa-space-l, 16px);
      margin: 0;
    }

    .item {
      display: grid;
      gap: var(--wa-space-3xs, 2px);
      min-width: 6rem;
    }

    dt {
      color: var(--wa-color-muted);
      font-size: var(--wa-font-size-s, 13px);
    }

    dd {
      margin: 0;
      font-size: var(--wa-font-size-xl, 20px);
      font-weight: var(--wa-font-weight-semibold, 600);
      font-variant-numeric: tabular-nums;
    }

    :host([dense]) dd {
      font-size: var(--wa-font-size-lg, 16px);
    }

    dd.good {
      color: var(--wa-color-success);
    }

    dd.warn {
      color: var(--wa-color-warning);
    }

    .hint {
      color: var(--wa-color-muted);
      font-size: var(--wa-font-size-s, 13px);
      font-weight: var(--wa-font-weight-normal, 400);
    }
  `;

  @property({ attribute: false })
  items: CountItem[] = [];

  @property({ type: Boolean, reflect: true })
  dense = false;

  render() {
    return html`
      <dl>
        ${this.items.map(
          (item) => html`
            <div class="item">
              <dt>${item.label}</dt>
              <dd class=${item.emphasis ?? 'default'}>
                ${item.value}
                ${item.hint ? html`<span class="hint">${item.hint}</span>` : nothing}
              </dd>
            </div>
          `,
        )}
      </dl>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-count-grid': WcCountGrid;
  }
}
