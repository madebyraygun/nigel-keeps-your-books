import { html } from 'lit';
import './wc-panel.js';
import type { Preview } from '../../preview/types.js';

const preview: Preview = {
  id: 'wc-panel',
  title: 'Panel',
  group: 'Layout',
  description: 'Titled section card: heading, description, body, footer actions.',
  layout: 'stack',
  states: [
    {
      name: 'default',
      render: () => html`<wc-panel heading="Business name">
        <p>Raygun LLC</p>
      </wc-panel>`,
    },
    {
      name: 'with-description',
      render: () =>
        html`<wc-panel
          heading="Data directory"
          description="Where this set of books lives. Switching reloads the app."
        >
          <code>/home/you/Documents/nigel</code>
        </wc-panel>`,
    },
    {
      name: 'with-actions',
      render: () =>
        html`<wc-panel heading="Business name" description="Shown in the sidebar and on reports.">
          <p>Raygun LLC</p>
          <button slot="actions" type="button">Save</button>
        </wc-panel>`,
    },
    {
      name: 'dense',
      render: () => html`<wc-panel dense heading="Auto-update">
        <p>Check for new versions on launch.</p>
      </wc-panel>`,
    },
  ],
};

export default preview;
