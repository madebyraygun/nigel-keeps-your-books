import { LitElement, html, css, nothing } from 'lit';
import { customElement, property } from 'lit/decorators.js';
import '../icons/icons.js';

/**
 * The "nothing here yet" panel: an empty result set, an unbuilt screen, a
 * filter that matched no rows.
 */
@customElement('wc-empty-state')
export class WcEmptyState extends LitElement {
  static styles = css`
    :host {
      display: grid;
      place-items: center;
      gap: var(--wa-space-s, 8px);
      padding: var(--wa-space-2xl, 32px);
      text-align: center;
      color: var(--wa-color-muted);
      font-family: var(--wa-font-family-sans);
    }

    :host([compact]) {
      padding: var(--wa-space-l, 16px);
    }

    .icon {
      --nc-icon-size: 32px;
      color: var(--wa-color-border);
    }

    .heading {
      font-size: var(--wa-font-size-lg, 16px);
      font-weight: var(--wa-font-weight-medium, 500);
      color: var(--wa-color-text);
      margin: 0;
    }

    .message {
      margin: 0;
      max-width: 44ch;
    }

    .actions {
      margin-top: var(--wa-space-s, 8px);
      display: flex;
      gap: var(--wa-space-s, 8px);
    }
  `;

  @property({ type: String })
  heading = '';

  @property({ type: String })
  message = '';

  /** Tag name of a `wc-icon-*` element, e.g. "wc-icon-register". */
  @property({ type: String })
  icon = '';

  @property({ type: Boolean, reflect: true })
  compact = false;

  private renderIcon() {
    if (!this.icon) return nothing;
    // The icon names a custom element at runtime, so it is created
    // imperatively rather than through a static template tag.
    const el = document.createElement(this.icon);
    el.classList.add('icon');
    return el;
  }

  render() {
    return html`
      ${this.renderIcon()}
      ${this.heading ? html`<p class="heading">${this.heading}</p>` : nothing}
      ${this.message ? html`<p class="message">${this.message}</p>` : nothing}
      <slot></slot>
      <div class="actions"><slot name="actions"></slot></div>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-empty-state': WcEmptyState;
  }
}
