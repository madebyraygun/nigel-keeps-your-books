import { LitElement, html, css, nothing } from 'lit';
import { customElement, property } from 'lit/decorators.js';
import '../icons/icons.js';

export interface NavItem {
  id: string;
  label: string;
  /** Tag name of a `wc-icon-*` element. */
  icon?: string;
  disabled?: boolean;
}

/**
 * Primary navigation. Presentational: it renders the items it is given and
 * announces intent. Which items exist and which one is active is the app's
 * screen registry's business.
 */
@customElement('wc-nav-sidebar')
export class WcNavSidebar extends LitElement {
  static styles = css`
    :host {
      display: block;
      width: var(--nc-sidebar-width, 232px);
      flex-shrink: 0;
      background: var(--wa-color-surface);
      border-right: 1px solid var(--wa-color-border);
      font-family: var(--wa-font-family-sans);
      overflow-y: auto;
    }

    :host([collapsed]) {
      width: var(--nc-sidebar-collapsed-width, 56px);
    }

    .brand {
      display: flex;
      align-items: center;
      gap: var(--wa-space-s, 8px);
      height: var(--nc-header-height, 48px);
      padding: 0 var(--wa-space-m, 12px);
      border-bottom: 1px solid var(--wa-color-border);
      box-sizing: border-box;
    }

    .brand-name {
      font-weight: var(--wa-font-weight-bold, 600);
      font-size: var(--wa-font-size-lg, 16px);
      background: var(--nc-grad-brand);
      -webkit-background-clip: text;
      background-clip: text;
      color: transparent;
    }

    :host([collapsed]) .brand-name {
      display: none;
    }

    ul {
      list-style: none;
      margin: 0;
      padding: var(--wa-space-s, 8px);
      display: grid;
      gap: 2px;
    }

    button {
      display: flex;
      align-items: center;
      gap: var(--wa-space-s, 8px);
      width: 100%;
      padding: 8px 10px;
      border: 0;
      border-radius: var(--wa-radius-sm, 6px);
      background: transparent;
      color: var(--wa-color-text);
      font: inherit;
      font-size: var(--wa-font-size-base, 14px);
      text-align: left;
      cursor: pointer;
      transition: background var(--nc-transition-fast, 120ms ease);
    }

    button:hover:not(.disabled) {
      background: var(--wa-color-surface-alt);
    }

    button.active {
      background: var(--nc-color-selected-bg);
      font-weight: var(--wa-font-weight-medium, 500);
    }

    button.disabled {
      opacity: 0.4;
      cursor: not-allowed;
    }

    :host([collapsed]) .label {
      display: none;
    }

    :host([collapsed]) button {
      justify-content: center;
      padding: 8px 0;
    }
  `;

  @property({ attribute: false })
  items: NavItem[] = [];

  @property({ type: String })
  active = '';

  @property({ type: Boolean, reflect: true })
  collapsed = false;

  @property({ type: String, attribute: 'app-name' })
  appName = 'Nigel';

  private handleClick(item: NavItem): void {
    if (item.disabled) return;
    this.dispatchEvent(
      new CustomEvent<{ id: string }>('nc-navigate', {
        detail: { id: item.id },
        bubbles: true,
        composed: true,
      }),
    );
  }

  private renderIcon(item: NavItem) {
    if (!item.icon) return nothing;
    // Icon tags come from data, so the element is created imperatively.
    return document.createElement(item.icon);
  }

  render() {
    return html`
      <div class="brand">
        <span class="brand-name">${this.appName}</span>
      </div>
      <nav aria-label="Primary">
        <ul>
          ${this.items.map((item) => {
            const isActive = item.id === this.active;
            const classes = [
              isActive ? 'active' : '',
              item.disabled ? 'disabled' : '',
            ]
              .filter(Boolean)
              .join(' ');
            return html`
              <li>
                <button
                  type="button"
                  class=${classes}
                  data-nav=${item.id}
                  aria-current=${isActive ? 'page' : 'false'}
                  aria-disabled=${item.disabled ? 'true' : 'false'}
                  title=${this.collapsed ? item.label : nothing}
                  @click=${() => this.handleClick(item)}
                >
                  ${this.renderIcon(item)}
                  <span class="label">${item.label}</span>
                </button>
              </li>
            `;
          })}
        </ul>
      </nav>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-nav-sidebar': WcNavSidebar;
  }
  interface HTMLElementEventMap {
    'nc-navigate': CustomEvent<{ id: string }>;
  }
}
