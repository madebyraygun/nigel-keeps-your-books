import { LitElement, html, css, nothing } from 'lit';
import { customElement, property } from 'lit/decorators.js';

/**
 * A titled section card: heading, optional description, body, footer actions.
 *
 * The settings screen is four of these stacked. It is a generic panel rather
 * than a settings-specific one because the reconcile and import screens want
 * the same box, and a second implementation of "card with a heading" is how
 * two of them drift apart.
 */
@customElement('wc-panel')
export class WcPanel extends LitElement {
  static styles = css`
    :host {
      display: block;
      background: var(--wa-color-surface);
      border: 1px solid var(--wa-color-border);
      border-radius: var(--wa-radius-l, 12px);
      padding: var(--wa-space-l, 16px);
      font-family: var(--wa-font-family-sans);
      color: var(--wa-color-text);
    }

    :host([dense]) {
      padding: var(--wa-space-m, 12px);
    }

    .heading {
      margin: 0;
      font-size: var(--wa-font-size-lg, 16px);
      font-weight: var(--wa-font-weight-semibold, 600);
    }

    .description {
      margin: var(--wa-space-2xs, 4px) 0 0;
      color: var(--wa-color-muted);
      font-size: var(--wa-font-size-s, 13px);
      max-width: 68ch;
    }

    .body {
      margin-top: var(--wa-space-m, 12px);
    }

    .actions {
      display: flex;
      justify-content: flex-end;
      gap: var(--wa-space-s, 8px);
      margin-top: var(--wa-space-m, 12px);
    }

    /* An empty actions slot should not leave a gap behind it. */
    .actions:not(:has(*)) {
      display: none;
    }
  `;

  @property({ type: String })
  heading = '';

  @property({ type: String })
  description = '';

  @property({ type: Boolean, reflect: true })
  dense = false;

  render() {
    return html`
      ${this.heading ? html`<h2 class="heading">${this.heading}</h2>` : nothing}
      ${this.description
        ? html`<p class="description">${this.description}</p>`
        : nothing}
      <div class="body"><slot></slot></div>
      <div class="actions"><slot name="actions"></slot></div>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-panel': WcPanel;
  }
}
