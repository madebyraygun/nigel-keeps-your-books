import { html } from 'lit';
import './wc-export-links.js';
import type { Preview } from '../../preview/types.js';

const text = '/api/exports/pnl?format=text&year=2025';
const pdf = '/api/exports/pnl?format=pdf&year=2025';

const preview: Preview = {
  id: 'wc-export-links',
  title: 'Export links',
  group: 'Actions',
  description:
    'Download links for one report, in both formats. PDF turns into a disabled control with a stated reason on a build compiled without the pdf feature, because a download link cannot tell a file from an error.',
  states: [
    {
      name: 'default',
      render: () =>
        html`<wc-export-links text-href=${text} pdf-href=${pdf}></wc-export-links>`,
    },
    {
      name: 'pdf unavailable',
      render: () =>
        html`<wc-export-links text-href=${text} .pdfAvailable=${false}></wc-export-links>`,
    },
    {
      name: 'busy',
      render: () =>
        html`<wc-export-links busy text-href=${text} pdf-href=${pdf}></wc-export-links>`,
    },
    {
      name: 'in a toolbar row',
      render: () => html`
        <div
          style="display:flex;align-items:center;justify-content:space-between;gap:12px;padding:8px;border:1px solid var(--wa-color-border);border-radius:8px;"
        >
          <strong style="font-family:var(--wa-font-family-sans);">Profit and loss</strong>
          <wc-export-links text-href=${text} pdf-href=${pdf}></wc-export-links>
        </div>
      `,
    },
  ],
};

export default preview;
