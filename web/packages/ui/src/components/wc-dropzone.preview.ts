import { html } from 'lit';
import './wc-dropzone.js';
import type { Preview } from '../../preview/types.js';

const preview: Preview = {
  id: 'wc-dropzone',
  title: 'Dropzone',
  group: 'Forms',
  description:
    'File well for statement uploads: drag and drop, or click to open the picker.',
  layout: 'stack',
  states: [
    {
      name: 'idle',
      render: () => html`<wc-dropzone></wc-dropzone>`,
    },
    {
      name: 'selected',
      render: () =>
        html`<wc-dropzone filename="april-2025.csv" .size=${8214}></wc-dropzone>`,
    },
    {
      name: 'error',
      render: () =>
        html`<wc-dropzone
          error="nigel reads .csv, .xlsx, .xls statements. That one is something else."
        ></wc-dropzone>`,
    },
    {
      name: 'selected-with-error',
      render: () =>
        html`<wc-dropzone
          filename="statement.csv"
          .size=${31457280}
          error="That file is over the 25 MB limit."
        ></wc-dropzone>`,
    },
    {
      name: 'busy',
      render: () =>
        html`<wc-dropzone busy filename="april-2025.csv" .size=${8214}></wc-dropzone>`,
    },
    {
      name: 'disabled',
      render: () => html`<wc-dropzone disabled></wc-dropzone>`,
    },
  ],
};

export default preview;
