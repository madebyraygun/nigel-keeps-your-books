import { html } from 'lit';
import './wc-invoice-status.js';
import { INVOICE_STATUS_WORDS } from './wc-invoice-status.js';
import type { Preview } from '../../preview/types.js';

const preview: Preview = {
  id: 'wc-invoice-status',
  title: 'Invoice status',
  group: 'Invoicing',
  description:
    'The six derived statuses as a glyph and a word. Colour is a third cue, never the only one.',
  states: [
    ...INVOICE_STATUS_WORDS.map((status) => ({
      name: status,
      render: () => html`<wc-invoice-status status=${status}></wc-invoice-status>`,
    })),
    {
      name: 'unknown',
      render: () => html`<wc-invoice-status status="imported"></wc-invoice-status>`,
    },
    {
      name: 'row',
      render: () => html`
        <div style="display:flex;gap:8px;flex-wrap:wrap;">
          ${INVOICE_STATUS_WORDS.map(
            (status) => html`<wc-invoice-status status=${status}></wc-invoice-status>`,
          )}
        </div>
      `,
    },
  ],
};

export default preview;
