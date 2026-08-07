import { html } from 'lit';
import './wc-confirm.js';
import type { Preview } from '../../preview/types.js';

const preview: Preview = {
  id: 'wc-confirm',
  title: 'Confirm',
  group: 'Overlays',
  description:
    'wa-dialog wrapper standing in for window.confirm. Focus lands on Cancel so a stray Enter takes the harmless path.',
  layout: 'stack',
  states: [
    {
      name: 'closed',
      render: () =>
        html`<wc-confirm message="Nothing to see — the dialog is closed."></wc-confirm>`,
    },
    {
      name: 'open-default',
      render: () =>
        html`<wc-confirm
          open
          heading="Re-run categorization?"
          message="Rules will be applied to every uncategorized transaction."
          confirm-label="Run"
        ></wc-confirm>`,
    },
    {
      name: 'open-danger',
      render: () =>
        html`<wc-confirm
          open
          variant="danger"
          heading="Undo the last import?"
          message="This deletes the 42 transactions that import created. It cannot be undone."
          confirm-label="Undo import"
        ></wc-confirm>`,
    },
    {
      name: 'long-message',
      render: () =>
        html`<wc-confirm
          open
          heading="Delete this category?"
          message="Deleting “Office Supplies” will fail while 37 transactions still reference it. Reassign those transactions first, then try again."
        ></wc-confirm>`,
    },
  ],
};

export default preview;
