import { html } from 'lit';
import './wc-notice-bar.js';
import type { Preview } from '../../preview/types.js';

const preview: Preview = {
  id: 'wc-notice-bar',
  title: 'Notice bar',
  group: 'Feedback',
  description:
    'A standing condition rather than an event — an available update, a missing build feature. It stays until dealt with, which is what separates it from a toast.',
  layout: 'stack',
  states: [
    {
      name: 'info',
      render: () =>
        html`<wc-notice-bar
          message="Reports are exported from the viewer."
        ></wc-notice-bar>`,
    },
    {
      name: 'update available',
      render: () =>
        html`<wc-notice-bar
          variant="warning"
          icon="wc-icon-download"
          message="Nigel v1.0.2 is available. Run nigel update to install."
          dismissible
        ></wc-notice-bar>`,
    },
    {
      name: 'success',
      render: () =>
        html`<wc-notice-bar
          variant="success"
          icon="wc-icon-check"
          message="Reconciled through March 2026."
        ></wc-notice-bar>`,
    },
    {
      name: 'danger',
      render: () =>
        html`<wc-notice-bar
          variant="danger"
          message="This build cannot render PDFs."
        ></wc-notice-bar>`,
    },
    {
      name: 'with action',
      render: () =>
        html`<wc-notice-bar
          variant="warning"
          message="12 transactions need review."
          action-label="Review now"
        ></wc-notice-bar>`,
    },
    {
      name: 'action and dismiss',
      render: () =>
        html`<wc-notice-bar
          variant="warning"
          icon="wc-icon-flag"
          message="12 transactions need review."
          action-label="Review now"
          dismissible
        ></wc-notice-bar>`,
    },
    {
      name: 'slotted content',
      render: () =>
        html`<wc-notice-bar variant="info"
          >Open the URL <code>nigel serve</code> printed.</wc-notice-bar
        >`,
    },
  ],
};

export default preview;
