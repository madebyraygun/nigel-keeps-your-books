import { LitElement, html, css, nothing } from 'lit';
import { customElement, property } from 'lit/decorators.js';

/**
 * How far through the review queue you are.
 *
 * A labelled bar rather than a row of dots: a queue after a fresh import is
 * routinely fifty to two hundred transactions, and two hundred dots is noise
 * rather than progress. The TUI shows a `LineGauge` labelled `3/12`, so a
 * labelled bar is also the parity choice.
 *
 * The bar is a plain `role="progressbar"` element rather than a Web Awesome
 * one because the count, the running tally and the track have to be announced
 * as a single thing — `aria-valuetext` is what a screen reader reads, and it
 * needs to say "3 of 12", not "25%".
 */
@customElement('wc-review-progress')
export class WcReviewProgress extends LitElement {
  static styles = css`
    :host {
      display: block;
      font-family: var(--wa-font-family-sans);
      color: var(--wa-color-text);
    }

    .row {
      display: flex;
      align-items: baseline;
      justify-content: space-between;
      gap: var(--wa-space-m, 12px);
      margin-bottom: var(--wa-space-2xs, 4px);
      font-size: var(--wa-font-size-s, 13px);
    }

    .position {
      font-weight: var(--wa-font-weight-medium, 500);
    }

    .tally {
      color: var(--wa-color-muted);
    }

    .track {
      height: 6px;
      border-radius: 999px;
      background: var(--wa-color-neutral-fill, rgb(0 0 0 / 12%));
      overflow: hidden;
    }

    .fill {
      height: 100%;
      background: var(--wa-color-brand);
      transition: width var(--wa-transition-medium, 200ms) ease;
    }

    @media (prefers-reduced-motion: reduce) {
      .fill {
        transition: none;
      }
    }
  `;

  /** 1-based position in the queue. */
  @property({ type: Number })
  current = 1;

  @property({ type: Number })
  total = 1;

  @property({ type: Number })
  reviewed = 0;

  @property({ type: Number })
  skipped = 0;

  private get percent(): number {
    if (this.total <= 0) return 0;
    const done = Math.min(Math.max(this.current - 1, 0), this.total);
    return Math.round((done / this.total) * 100);
  }

  render() {
    const position = `${Math.min(this.current, this.total)} of ${this.total}`;
    const tally: string[] = [];
    if (this.reviewed > 0) tally.push(`${this.reviewed} reviewed`);
    if (this.skipped > 0) tally.push(`${this.skipped} skipped`);

    return html`
      <div class="row">
        <span class="position">${position}</span>
        ${tally.length > 0
          ? html`<span class="tally">${tally.join(' · ')}</span>`
          : nothing}
      </div>
      <div
        class="track"
        role="progressbar"
        aria-label="Review progress"
        aria-valuemin="0"
        aria-valuemax=${this.total}
        aria-valuenow=${Math.min(Math.max(this.current - 1, 0), this.total)}
        aria-valuetext=${position}
      >
        <div class="fill" style=${`width: ${this.percent}%`}></div>
      </div>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-review-progress': WcReviewProgress;
  }
}
