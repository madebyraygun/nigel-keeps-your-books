import { LitElement, html, css, nothing } from 'lit';
import { customElement, property } from 'lit/decorators.js';
import '../icons/icons.js';

export interface LinkGridItem {
  href: string;
  label: string;
  description?: string;
  /** Tag name of a `wc-icon-*` element, e.g. "wc-icon-report". */
  icon?: string;
}

/**
 * A directory of links as cards — the reports landing, and any other page whose
 * job is to point at its own sub-pages.
 *
 * Anchors, not click handlers: the routes are real hashes, so the browser's own
 * navigation, middle-click and "open in new tab" all keep working, and the
 * component needs to know nothing about the router.
 */
@customElement('wc-link-grid')
export class WcLinkGrid extends LitElement {
  static styles = css`
    :host {
      display: block;
      font-family: var(--wa-font-family-sans);
      color: var(--wa-color-text);
    }

    ul {
      list-style: none;
      margin: 0;
      padding: 0;
      display: grid;
      gap: var(--wa-space-m, 12px);
      grid-template-columns: repeat(auto-fill, minmax(16rem, 1fr));
    }

    :host([compact]) ul {
      grid-template-columns: 1fr;
      gap: var(--wa-space-xs, 6px);
    }

    a {
      display: grid;
      gap: var(--wa-space-2xs, 4px);
      height: 100%;
      box-sizing: border-box;
      padding: var(--wa-space-m, 12px);
      border: 1px solid var(--wa-color-border);
      border-radius: var(--wa-radius-lg, 12px);
      background: var(--wa-color-surface);
      color: inherit;
      text-decoration: none;
    }

    a:hover {
      border-color: var(--wa-color-brand);
    }

    a:focus-visible {
      outline: 2px solid var(--wa-color-focus);
      outline-offset: 2px;
    }

    .label {
      display: inline-flex;
      align-items: center;
      gap: var(--wa-space-xs, 6px);
      font-weight: var(--wa-font-weight-medium, 500);
    }

    .description {
      margin: 0;
      font-size: var(--wa-font-size-s, 13px);
      color: var(--wa-color-muted);
    }
  `;

  @property({ attribute: false })
  items: LinkGridItem[] = [];

  /** The list's accessible name. */
  @property({ type: String })
  label = '';

  @property({ type: Boolean, reflect: true })
  compact = false;

  private renderIcon(icon?: string) {
    if (!icon) return nothing;
    // The icon names a custom element at runtime, so it is created
    // imperatively. It repeats the label beside it, so it is decoration as far
    // as a screen reader is concerned.
    const el = document.createElement(icon);
    el.setAttribute('aria-hidden', 'true');
    return el;
  }

  render() {
    return html`
      <nav aria-label=${this.label || nothing}>
        <ul>
          ${this.items.map(
            (item) => html`
              <li>
                <a href=${item.href}>
                  <span class="label">${this.renderIcon(item.icon)}${item.label}</span>
                  ${item.description
                    ? html`<p class="description">${item.description}</p>`
                    : nothing}
                </a>
              </li>
            `,
          )}
        </ul>
      </nav>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-link-grid': WcLinkGrid;
  }
}
