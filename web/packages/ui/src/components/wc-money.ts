import { LitElement, html, css } from 'lit';
import { customElement, property } from 'lit/decorators.js';

export type WcMoneyAlign = 'start' | 'end';

/**
 * A signed cash amount — the web counterpart of `tui::money_span`.
 *
 * One deliberate difference from the TUI: `money_span` prints the absolute
 * value and lets red-versus-green carry the sign, which a terminal can get
 * away with. Here the minus sign is always rendered, because color alone
 * cannot be the only channel conveying meaning (WCAG 1.4.1) and a red/green
 * distinction is exactly the pair most color-vision deficiencies flatten.
 *
 * Formatting goes through Intl, which for USD produces the same text as
 * `fmt::money` ("$1,234.56", "-$500.00").
 */
@customElement('wc-money')
export class WcMoney extends LitElement {
  static styles = css`
    :host {
      display: inline-block;
      font-family: var(--nc-font-money, monospace);
      /* Digits share a width so columns of amounts line up. */
      font-variant-numeric: tabular-nums;
      white-space: nowrap;
    }

    :host([align='end']) {
      text-align: right;
      width: 100%;
    }

    .amount[data-sign='positive'] {
      color: var(--nc-color-income);
    }

    .amount[data-sign='negative'] {
      color: var(--nc-color-expense);
    }

    .amount[data-sign='zero'] {
      color: var(--wa-color-muted);
    }
  `;

  @property({ type: Number })
  amount = 0;

  @property({ type: String })
  currency = 'USD';

  /** Undefined uses the runtime's default locale. */
  @property({ type: String })
  locale?: string;

  /** `plain` drops the income/expense coloring; the sign is still rendered. */
  @property({ type: String, reflect: true })
  variant: 'signed' | 'plain' = 'signed';

  /** Render zero in the muted color rather than as income. */
  @property({ type: Boolean, attribute: 'zero-neutral' })
  zeroNeutral = true;

  @property({ type: String, reflect: true })
  align: WcMoneyAlign = 'start';

  private get sign(): 'positive' | 'negative' | 'zero' {
    if (this.amount < 0) return 'negative';
    if (this.amount === 0 && this.zeroNeutral) return 'zero';
    return 'positive';
  }

  /** The formatted text, exposed so callers can reuse it (titles, exports). */
  get formatted(): string {
    return new Intl.NumberFormat(this.locale, {
      style: 'currency',
      currency: this.currency,
    }).format(this.amount);
  }

  render() {
    const sign = this.variant === 'plain' ? 'plain' : this.sign;
    return html`<span class="amount" part="amount" data-sign=${sign}
      >${this.formatted}</span
    >`;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-money': WcMoney;
  }
}
