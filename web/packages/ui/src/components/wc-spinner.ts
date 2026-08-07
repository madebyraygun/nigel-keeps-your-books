import { LitElement, html, css } from 'lit';
import { customElement, property } from 'lit/decorators.js';

export type WcSpinnerSize = 's' | 'm' | 'l';

/**
 * Busy indicator. The label is always present for assistive tech — a spinner
 * that announces nothing leaves a screen-reader user with silence where a
 * sighted user sees motion.
 */
@customElement('wc-spinner')
export class WcSpinner extends LitElement {
  static styles = css`
    :host {
      display: grid;
      place-items: center;
      gap: var(--wa-space-s, 8px);
      color: var(--wa-color-muted);
      font-family: var(--wa-font-family-sans);
      font-size: var(--wa-font-size-s, 12px);
      --size: 28px;
    }

    :host([size='s']) {
      --size: 18px;
    }

    :host([size='l']) {
      --size: 44px;
    }

    :host([inline]) {
      display: inline-grid;
      grid-auto-flow: column;
      align-items: center;
    }

    .ring {
      width: var(--size);
      height: var(--size);
      border-radius: 50%;
      border: 2px solid var(--wa-color-border);
      border-top-color: var(--wa-color-brand);
      animation: spin 720ms linear infinite;
    }

    @keyframes spin {
      to {
        transform: rotate(360deg);
      }
    }

    @media (prefers-reduced-motion: reduce) {
      .ring {
        animation: none;
        border-top-color: var(--wa-color-brand);
      }
    }

    .visually-hidden {
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
  `;

  @property({ type: String, reflect: true })
  size: WcSpinnerSize = 'm';

  @property({ type: String })
  label = 'Loading';

  /** Show the label as text rather than only to assistive tech. */
  @property({ type: Boolean, attribute: 'show-label' })
  showLabel = false;

  @property({ type: Boolean, reflect: true })
  inline = false;

  render() {
    return html`
      <div class="ring" part="ring"></div>
      <span
        role="status"
        aria-live="polite"
        class=${this.showLabel ? 'label' : 'visually-hidden'}
        >${this.label}</span
      >
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-spinner': WcSpinner;
  }
}
