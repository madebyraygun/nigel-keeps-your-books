import { html } from 'lit';
import './wc-app-shell.js';
import './wc-nav-sidebar.js';
import './wc-empty-state.js';
import { NAV_ITEMS } from './__mocks__/nav.js';
import type { Preview } from '../../preview/types.js';

const shell = (extra = html``, attrs = {}) => html`
  <div style="height:420px;border:1px solid var(--wa-color-border);overflow:hidden;">
    <wc-app-shell
      screen-title=${(attrs as { title?: string }).title ?? 'Dashboard'}
      style="height:100%;"
    >
      <wc-nav-sidebar
        slot="sidebar"
        .items=${NAV_ITEMS}
        active="dashboard"
      ></wc-nav-sidebar>
      ${extra}
      <wc-empty-state
        heading="Dashboard"
        message="Screen content goes here."
      ></wc-empty-state>
    </wc-app-shell>
  </div>
`;

const preview: Preview = {
  id: 'wc-app-shell',
  title: 'App Shell',
  group: 'Layout',
  description:
    'Structural frame: sidebar slot, header, banner slot, content, and the single toast region.',
  layout: 'stack',
  states: [
    { name: 'default', render: () => shell() },
    {
      name: 'with-header-actions',
      render: () =>
        shell(html`<button slot="header-actions" type="button">Export</button>`),
    },
    {
      name: 'with-banner',
      render: () =>
        shell(
          html`<span slot="banner"
            >Session expired — reopen the URL nigel serve printed.</span
          >`,
        ),
    },
  ],
};

export default preview;
