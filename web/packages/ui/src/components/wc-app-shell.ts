import { LitElement, html, css } from 'lit';
import { customElement, property } from 'lit/decorators.js';
import './wc-toast.js';

/**
 * The application frame: sidebar rail, header, banner slot, content area, and
 * the single toast region.
 *
 * Purely structural. Unlike boxcraft's app-shell it does not decide what to
 * render — nigel routes from a screen registry in the app, so the shell only
 * provides the slots and lets the container fill them.
 */
@customElement('wc-app-shell')
export class WcAppShell extends LitElement {
  static styles = css`
    :host {
      display: flex;
      height: 100vh;
      background: var(--wa-color-bg);
      color: var(--wa-color-text);
      font-family: var(--wa-font-family-sans);
      font-size: var(--wa-font-size-base);
      line-height: var(--wa-line-height);
    }

    .main {
      flex: 1;
      display: flex;
      flex-direction: column;
      overflow: hidden;
      min-width: 0;
    }

    header {
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: var(--wa-space-m, 12px);
      min-height: var(--nc-header-height, 48px);
      padding: 0 var(--wa-space-l, 16px);
      background: var(--wa-color-surface);
      border-bottom: 1px solid var(--wa-color-border);
      box-sizing: border-box;
    }

    .title {
      font-size: var(--wa-font-size-lg, 16px);
      font-weight: var(--wa-font-weight-medium, 500);
      margin: 0;
    }

    .actions {
      display: flex;
      align-items: center;
      gap: var(--wa-space-s, 8px);
    }

    .content {
      flex: 1;
      overflow: auto;
      padding: var(--wa-space-l, 16px);
    }

    .banner:not(:empty) {
      padding: var(--wa-space-s, 8px) var(--wa-space-l, 16px);
      background: var(--wa-color-surface-alt);
      border-bottom: 1px solid var(--wa-color-border);
    }
  `;

  @property({ type: String, attribute: 'screen-title' })
  screenTitle = '';

  @property({ type: Boolean, reflect: true, attribute: 'sidebar-collapsed' })
  sidebarCollapsed = false;

  render() {
    return html`
      <slot name="sidebar"></slot>
      <div class="main">
        <header>
          <h1 class="title">${this.screenTitle}</h1>
          <div class="actions"><slot name="header-actions"></slot></div>
        </header>
        <div class="banner"><slot name="banner"></slot></div>
        <main class="content"><slot></slot></main>
      </div>
      <wc-toast></wc-toast>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-app-shell': WcAppShell;
  }
}
