import { html } from 'lit';
import './wc-invoice-preview.js';
import type { Preview } from '../../preview/types.js';

// A data: URL so the harness has something to frame without a server.
const page = `data:text/html;charset=utf-8,${encodeURIComponent(
  '<h1>Invoice #1251</h1><p>Acme Co — $1,850.00</p>',
)}`;

const preview: Preview = {
  id: 'wc-invoice-preview',
  title: 'Invoice preview',
  group: 'Invoicing',
  description:
    'The client-facing page in a sandboxed iframe, collapsed by default. The sandbox omits allow-same-origin, so the framed document cannot reach the app it is embedded in.',
  layout: 'stack',
  states: [
    {
      name: 'collapsed',
      render: () => html`
        <wc-invoice-preview src=${page} pdf-src="#pdf"></wc-invoice-preview>
      `,
    },
    {
      name: 'open',
      render: () => html`
        <wc-invoice-preview open src=${page} pdf-src="#pdf"></wc-invoice-preview>
      `,
    },
    {
      name: 'missing-config',
      render: () => html`
        <wc-invoice-preview
          open
          src=${page}
          pdf-src="#pdf"
          .missing=${['r2_bucket', 'public_base_url']}
        ></wc-invoice-preview>
      `,
    },
    {
      name: 'pdf-unavailable',
      render: () => html`
        <wc-invoice-preview open src=${page} .pdfAvailable=${false}></wc-invoice-preview>
      `,
    },
  ],
};

export default preview;
