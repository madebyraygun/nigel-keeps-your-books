import { LitElement, html, css, type TemplateResult } from 'lit';
import { customElement, state, property } from 'lit/decorators.js';
import type { Preview, PreviewBackground } from '../types.js';
import { parseRoute, routeToUrl, type Route } from '../router.js';
import { runA11y, type A11yResult } from '../a11y.js';

const BG_MAP: Record<PreviewBackground, string> = {
  default: 'var(--wa-color-bg)',
  surface: 'var(--wa-color-surface)',
  inverse: '#17171d',
  transparent: 'transparent',
};

@customElement('preview-app')
export class PreviewApp extends LitElement {
  @property({ attribute: false }) previews: Preview[] = [];
  @state() private route: Route = parseRoute(window.location.href);
  @state() private a11y: A11yResult | null = null;
  @state() private inspectorOpen = false;

  static styles = css`
    :host {
      display: grid;
      grid-template-columns: 240px 1fr auto;
      height: 100vh;
      font-family: var(--wa-font-family-sans);
      color: var(--wa-color-text);
      background: var(--wa-color-bg);
    }
    nav {
      border-right: 1px solid var(--wa-color-border);
      overflow-y: auto;
      padding: var(--wa-space-l);
    }
    nav h3 {
      font-family: var(--wa-font-family-mono);
      font-size: 11px;
      letter-spacing: 0.08em;
      text-transform: uppercase;
      color: var(--wa-color-muted);
      margin: var(--wa-space-l) 0 var(--wa-space-xs);
    }
    nav a {
      display: block;
      padding: 6px 8px;
      border-radius: var(--wa-radius-sm);
      color: var(--wa-color-text);
      text-decoration: none;
      font-size: 13px;
    }
    nav a.active {
      background: var(--nc-color-selected-bg);
      font-weight: var(--wa-font-weight-medium);
    }
    main {
      padding: var(--wa-space-xl);
      overflow-y: auto;
    }
    h1 {
      font-size: var(--wa-font-size-xl);
      margin: 0 0 var(--wa-space-s);
    }
    .state-grid {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(280px, 1fr));
      gap: var(--wa-space-l);
    }
    .state-grid[data-layout='grid-wide'] {
      grid-template-columns: repeat(auto-fit, minmax(540px, 1fr));
    }
    .state-grid[data-layout='stack'] {
      grid-template-columns: 1fr;
    }
    .state-card {
      border: 1px solid var(--wa-color-border);
      border-radius: var(--wa-radius-md);
      padding: var(--wa-space-l);
    }
    .state-card[data-bg='inverse'] {
      color: #ece9f5;
    }
    .state-name {
      font-family: var(--wa-font-family-mono);
      font-size: 11px;
      letter-spacing: 0.08em;
      text-transform: uppercase;
      color: var(--wa-color-muted);
      margin-bottom: var(--wa-space-s);
    }
    aside {
      width: 320px;
      border-left: 1px solid var(--wa-color-border);
      padding: var(--wa-space-l);
      overflow-y: auto;
      background: var(--wa-color-surface);
    }
    aside[hidden] {
      display: none;
    }
    .violation {
      padding: var(--wa-space-s);
      border-left: 3px solid var(--wa-color-danger);
      margin-bottom: var(--wa-space-s);
    }
    button.inspector-toggle {
      position: fixed;
      top: 12px;
      right: 12px;
      background: var(--wa-color-surface);
      color: var(--wa-color-text);
      border: 1px solid var(--wa-color-border);
      padding: 6px 10px;
      border-radius: var(--wa-radius-sm);
      cursor: pointer;
      font-family: var(--wa-font-family-mono);
      font-size: 12px;
    }
  `;

  connectedCallback() {
    super.connectedCallback();
    window.addEventListener('popstate', this.onPopState);
  }

  disconnectedCallback() {
    super.disconnectedCallback();
    window.removeEventListener('popstate', this.onPopState);
  }

  private onPopState = () => {
    this.route = parseRoute(window.location.href);
  };

  private navigate(route: Route) {
    history.pushState({}, '', routeToUrl(route));
    this.route = route;
    this.a11y = null;
  }

  private get activePreview(): Preview | undefined {
    return this.previews.find((p) => p.id === this.route.previewId);
  }

  private runInspector = async () => {
    const container = this.renderRoot.querySelector('.state-grid');
    if (!container) return;
    this.a11y = await runA11y(container as HTMLElement);
  };

  render() {
    if (this.route.mode === 'preview') return this.renderEmbedded();
    return html`
      ${this.renderSidebar()} ${this.renderMain()} ${this.renderInspector()}
      <button
        class="inspector-toggle"
        @click=${() => (this.inspectorOpen = !this.inspectorOpen)}
      >
        ${this.inspectorOpen ? 'Hide' : 'a11y'}
      </button>
    `;
  }

  private renderSidebar(): TemplateResult {
    const groups = new Map<string, Preview[]>();
    for (const p of this.previews) {
      const list = groups.get(p.group) ?? [];
      list.push(p);
      groups.set(p.group, list);
    }
    return html`
      <nav>
        ${Array.from(
          groups,
          ([group, items]) => html`
            <h3>${group}</h3>
            ${items.map(
              (p) => html`
                <a
                  href=${routeToUrl({ previewId: p.id })}
                  class=${this.route.previewId === p.id ? 'active' : ''}
                  @click=${(e: Event) => {
                    e.preventDefault();
                    this.navigate({ previewId: p.id });
                  }}
                  >${p.title}</a
                >
              `,
            )}
          `,
        )}
      </nav>
    `;
  }

  private renderMain(): TemplateResult {
    const preview = this.activePreview;
    if (!preview) {
      return html`<main><p>Select a preview from the sidebar.</p></main>`;
    }
    const states = this.route.stateName
      ? preview.states.filter((s) => s.name === this.route.stateName)
      : preview.states;
    return html`
      <main>
        <h1>${preview.title}</h1>
        ${preview.description ? html`<p>${preview.description}</p>` : ''}
        <div
          class="state-grid"
          data-storyloaded
          data-layout=${preview.layout ?? 'grid'}
        >
          ${states.map(
            (s) => html`
              <div
                class="state-card"
                data-bg=${s.background ?? 'default'}
                style="background:${BG_MAP[s.background ?? 'default']}"
              >
                <div class="state-name">${s.name}</div>
                ${s.render()}
              </div>
            `,
          )}
        </div>
      </main>
    `;
  }

  private renderEmbedded(): TemplateResult {
    const preview = this.activePreview;
    if (!preview) return html`<div></div>`;
    const state =
      preview.states.find((s) => s.name === this.route.stateName) ??
      preview.states[0];
    return html`
      <div
        data-storyloaded
        style="display:grid;place-items:center;min-height:100vh;background:${BG_MAP[
          state.background ?? 'default'
        ]};"
      >
        ${state.render()}
      </div>
    `;
  }

  private renderInspector(): TemplateResult {
    return html`
      <aside ?hidden=${!this.inspectorOpen}>
        <h3>Accessibility</h3>
        <button @click=${this.runInspector}>Run axe</button>
        ${this.a11y === null
          ? html`<p>Run axe to check the active preview.</p>`
          : this.a11y.violations.length === 0
            ? html`<p>✓ Zero violations.</p>`
            : this.a11y.violations.map(
                (v) => html`
                  <div class="violation">
                    <strong>${v.id}</strong> (${v.impact})
                    <div>${v.description}</div>
                    <a href=${v.helpUrl} target="_blank" rel="noreferrer">More</a>
                  </div>
                `,
              )}
      </aside>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'preview-app': PreviewApp;
  }
}
