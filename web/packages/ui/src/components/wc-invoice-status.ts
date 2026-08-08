import { LitElement, html, css } from 'lit';
import { customElement, property } from 'lit/decorators.js';

/**
 * The six derived statuses, in the order `refresh_status` reasons about them.
 *
 * Kept as data here rather than imported from the app: `@nigel/ui` never sees
 * an API type, and a status the server invents tomorrow still renders — as its
 * own word, with a neutral glyph.
 */
export const INVOICE_STATUS_WORDS = [
  'draft',
  'sent',
  'partial',
  'paid',
  'overdue',
  'void',
] as const;

export type InvoiceStatusWord = (typeof INVOICE_STATUS_WORDS)[number];

/**
 * A glyph per status, matching the wireframes and the TUI's own shorthand.
 *
 * The glyph is decorative — the word beside it is what carries the meaning.
 * Both are rendered because colour alone cannot be the only channel (WCAG
 * 1.4.1), which is the same reason `wc-money` always prints its sign.
 */
const GLYPHS: Record<string, string> = {
  draft: '◻',
  sent: '◆',
  partial: '◑',
  paid: '●',
  overdue: '▲',
  void: '⊘',
};

@customElement('wc-invoice-status')
export class WcInvoiceStatus extends LitElement {
  static styles = css`
    :host {
      display: inline-block;
    }

    .chip {
      display: inline-flex;
      align-items: center;
      gap: var(--wa-space-2xs, 4px);
      padding: 0 var(--wa-space-xs, 6px);
      border: 1px solid var(--wa-color-border);
      border-radius: var(--wa-radius-pill, 999px);
      font-family: var(--wa-font-family-sans);
      font-size: var(--wa-font-size-s, 13px);
      line-height: 1.7;
      white-space: nowrap;
      color: var(--wa-color-text);
    }

    .glyph {
      font-size: 0.9em;
    }

    .chip[data-status='draft'] {
      color: var(--wa-color-muted);
    }

    .chip[data-status='sent'] {
      color: var(--wa-color-brand);
      border-color: currentColor;
    }

    /*
     * Partial reads as the flagged token rather than a warning colour of its
     * own. Every value here must be a token @nigel/theme defines in both
     * schemes and holds to WCAG AA — a literal fallback that can actually win
     * is a colour the contrast test never sees.
     */
    .chip[data-status='partial'] {
      color: var(--nc-color-flagged);
      border-color: currentColor;
    }

    .chip[data-status='paid'] {
      color: var(--nc-color-income);
      border-color: currentColor;
    }

    .chip[data-status='overdue'] {
      color: var(--nc-color-expense);
      border-color: currentColor;
    }

    .chip[data-status='void'] {
      color: var(--wa-color-muted);
      text-decoration: line-through;
    }
  `;

  /** The status word as the server spelled it; unknown values render as-is. */
  @property({ type: String, reflect: true })
  status = 'draft';

  render() {
    const glyph = GLYPHS[this.status] ?? '•';
    return html`
      <span class="chip" part="chip" data-status=${this.status}>
        <span class="glyph" aria-hidden="true">${glyph}</span>
        <span class="word">${this.status}</span>
      </span>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-invoice-status': WcInvoiceStatus;
  }
}
