import { html } from 'lit';
import './wc-spinner.js';
import type { Preview } from '../../preview/types.js';

const preview: Preview = {
  id: 'wc-spinner',
  title: 'Spinner',
  group: 'Feedback',
  description:
    'Busy indicator. The label is always announced; show-label also renders it.',
  states: [
    { name: 'small', render: () => html`<wc-spinner size="s"></wc-spinner>` },
    { name: 'medium', render: () => html`<wc-spinner></wc-spinner>` },
    { name: 'large', render: () => html`<wc-spinner size="l"></wc-spinner>` },
    {
      name: 'with-label',
      render: () =>
        html`<wc-spinner show-label label="Connecting to nigel"></wc-spinner>`,
    },
    {
      name: 'inline',
      render: () =>
        html`<span
          >Importing <wc-spinner inline size="s" label="Importing"></wc-spinner
        ></span>`,
    },
  ],
};

export default preview;
