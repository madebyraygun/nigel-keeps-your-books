import { LitElement, html, css, nothing } from 'lit';
import { customElement, state } from 'lit/decorators.js';
import '@nigel/ui';
import { dispatchNcToast } from '@nigel/ui';

import { SignalWatcher } from '../mixins/signal-watcher.js';
import { FetchApiClient, appUnauthorized, type ApiClient } from '../api/index.js';
import {
  getAppStore,
  initializeAppStore,
  type AppStore,
} from '../state/app-store.js';
import { parseHash, screenToHash, type Route } from '../screens/hash-route.js';
import { DEFAULT_SCREEN, navItems, screenDef, type ScreenId } from '../screens/registry.js';
import type { ScreenContext } from '../screens/context.js';

/**
 * Root container: owns the api client, the store, and the route.
 *
 * Routing is one-directional. Navigation writes `location.hash` and nothing
 * else; the hashchange listener is the only thing that updates `route`. That
 * is what makes the back button and a pasted deep link behave the same as a
 * click.
 */
@customElement('nigel-app')
export class NigelApp extends SignalWatcher(LitElement) {
  static styles = css`
    :host {
      display: block;
      height: 100vh;
    }

    .boot {
      display: grid;
      place-items: center;
      height: 100vh;
      background: var(--wa-color-bg);
    }

    .gate {
      height: 100vh;
      overflow: auto;
    }

    .banner {
      color: var(--wa-color-text);
    }

    .banner strong {
      color: var(--wa-color-danger);
    }

    .retry {
      margin-left: var(--wa-space-s, 8px);
    }
  `;

  /** Overridable so tests can drive the app with a fake transport. */
  client: ApiClient = new FetchApiClient();

  @state()
  private route: Route = { screen: DEFAULT_SCREEN, params: new URLSearchParams() };

  private store: AppStore | null = null;
  private reportedError: string | null = null;

  connectedCallback(): void {
    super.connectedCallback();
    this.store = initializeAppStore(this.client);
    window.addEventListener('hashchange', this.handleHashChange);
    this.syncRouteFromHash();
    void this.store.refreshStatus();
  }

  disconnectedCallback(): void {
    window.removeEventListener('hashchange', this.handleHashChange);
    super.disconnectedCallback();
  }

  private handleHashChange = (): void => {
    this.syncRouteFromHash();
  };

  private syncRouteFromHash(): void {
    if (!window.location.hash) {
      // Seed the address bar so a reload lands on the same screen.
      window.location.hash = screenToHash(DEFAULT_SCREEN);
      return;
    }
    this.route = parseHash(window.location.hash);
  }

  private handleNavigate = (event: CustomEvent<{ id: string }>): void => {
    window.location.hash = `#/${event.detail.id}`;
  };

  /** Navigation for screens: the same one-directional path the sidebar takes. */
  private navigate = (screen: ScreenId, params?: URLSearchParams): void => {
    const query = params?.toString();
    window.location.hash = query ? `#/${screen}?${query}` : `#/${screen}`;
  };

  private screenContext(): ScreenContext {
    return {
      client: this.client,
      params: this.route.params,
      navigate: this.navigate,
    };
  }

  private handleRetry = (): void => {
    this.reportedError = null;
    void this.store?.refreshStatus();
  };

  /** Surface a status failure once per occurrence rather than every render. */
  private announceError(message: string): void {
    if (this.reportedError === message) return;
    this.reportedError = message;
    dispatchNcToast(this, { message, variant: 'danger', duration: 0 });
  }

  render() {
    const store = this.store ?? getAppStore();
    const error = store.statusError.get();

    if (error) this.announceError(error.message);

    const boot = store.boot.get();
    const ctx = this.screenContext();

    if (boot === 'starting') {
      return html`
        <div class="boot">
          <wc-spinner size="l" show-label label="Connecting to nigel"></wc-spinner>
        </div>
      `;
    }

    // The gate replaces the shell rather than sitting inside it: with no
    // sidebar and no screen rendered, nothing exists that could fetch data
    // before the password arrives. The hash is left alone, so unlocking returns
    // the user to wherever they were headed.
    if (boot === 'locked') {
      const gate = screenDef('unlock');
      document.title = `${gate.title} · ${store.companyName.get()}`;
      return html`<div class="gate">${gate.render(ctx)}</div>`;
    }

    const screen = screenDef(this.route.screen);
    document.title = `${screen.title} · ${store.companyName.get()}`;

    return html`
      <wc-app-shell screen-title=${screen.title}>
        <wc-nav-sidebar
          slot="sidebar"
          .items=${navItems()}
          active=${screen.id}
          app-name=${store.companyName.get()}
          @nc-navigate=${this.handleNavigate}
        ></wc-nav-sidebar>
        ${this.renderBanner(error?.message ?? null)} ${screen.render(ctx)}
      </wc-app-shell>
    `;
  }

  private renderBanner(errorMessage: string | null) {
    if (appUnauthorized.get()) {
      return html`
        <div slot="banner" class="banner">
          <strong>Session expired.</strong> Reopen the link
          <code>nigel serve</code> printed — the browser cannot mint a new
          session token on its own.
        </div>
      `;
    }
    if (errorMessage) {
      return html`
        <div slot="banner" class="banner">
          <strong>Could not load status.</strong> ${errorMessage}
          <button class="retry" type="button" @click=${this.handleRetry}>
            Retry
          </button>
        </div>
      `;
    }
    return nothing;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'nigel-app': NigelApp;
  }
}
