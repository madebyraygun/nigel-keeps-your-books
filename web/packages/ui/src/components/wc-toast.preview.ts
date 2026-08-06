import { html } from 'lit';
import './wc-toast.js';
import type { Preview } from '../../preview/types.js';
import type { NcToastDetail } from './wc-toast.js';

const seeded = (initial: NcToastDetail) =>
  html`<wc-toast .initial=${{ duration: 0, ...initial }}></wc-toast>`;

const preview: Preview = {
  id: 'wc-toast',
  title: 'Toast',
  group: 'Feedback',
  description:
    'The single aria-live region terminating the nc-toast bus. States seed a toast via .initial with auto-dismiss disabled so they stay visible.',
  layout: 'stack',
  states: [
    { name: 'info', render: () => seeded({ message: 'Rules re-applied.' }) },
    {
      name: 'success',
      render: () =>
        seeded({ message: '42 transactions imported.', variant: 'success' }),
    },
    {
      name: 'danger',
      render: () =>
        seeded({ message: 'Could not reach the nigel server.', variant: 'danger' }),
    },
    {
      name: 'with-action',
      render: () =>
        seeded({
          message: 'Import undone.',
          action: { label: 'Redo', onClick: () => {} },
        }),
    },
    {
      name: 'long-message',
      render: () =>
        seeded({
          message:
            'The category "Office Supplies" could not be deleted because 37 transactions still reference it.',
          variant: 'danger',
        }),
    },
  ],
};

export default preview;
