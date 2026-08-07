import { html } from 'lit';
import './wc-review-progress.js';
import type { Preview } from '../../preview/types.js';

const preview: Preview = {
  id: 'wc-review-progress',
  title: 'Review Progress',
  group: 'Data',
  description: 'Position in the review queue, with a running reviewed/skipped tally.',
  layout: 'stack',
  states: [
    {
      name: 'start',
      render: () =>
        html`<wc-review-progress .current=${1} .total=${12}></wc-review-progress>`,
    },
    {
      name: 'mid',
      render: () =>
        html`<wc-review-progress
          .current=${5}
          .total=${12}
          .reviewed=${4}
        ></wc-review-progress>`,
    },
    {
      name: 'with-skips',
      render: () =>
        html`<wc-review-progress
          .current=${8}
          .total=${12}
          .reviewed=${5}
          .skipped=${2}
        ></wc-review-progress>`,
    },
    {
      name: 'single',
      render: () =>
        html`<wc-review-progress .current=${1} .total=${1}></wc-review-progress>`,
    },
    {
      name: 'complete',
      render: () =>
        html`<wc-review-progress
          .current=${12}
          .total=${12}
          .reviewed=${11}
          .skipped=${1}
        ></wc-review-progress>`,
    },
  ],
};

export default preview;
