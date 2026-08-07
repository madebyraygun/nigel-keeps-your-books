import { html } from 'lit';
import './wc-empty-state.js';
import type { Preview } from '../../preview/types.js';

const preview: Preview = {
  id: 'wc-empty-state',
  title: 'Empty State',
  group: 'Feedback',
  description: 'Nothing-here panel for empty result sets and unbuilt screens.',
  states: [
    {
      name: 'default',
      render: () =>
        html`<wc-empty-state
          heading="No transactions"
          message="Import a statement to get started."
        ></wc-empty-state>`,
    },
    {
      name: 'with-icon',
      render: () =>
        html`<wc-empty-state
          icon="wc-icon-register"
          heading="No transactions"
          message="Nothing matches the current filters."
        ></wc-empty-state>`,
    },
    {
      name: 'with-action',
      render: () =>
        html`<wc-empty-state
          icon="wc-icon-import"
          heading="No imports yet"
          message="Bring in a bank CSV or XLSX to populate the register."
        >
          <button slot="actions" type="button">Import a statement</button>
        </wc-empty-state>`,
    },
    {
      name: 'compact',
      render: () =>
        html`<wc-empty-state compact message="No results."></wc-empty-state>`,
    },
  ],
};

export default preview;
