import { html } from 'lit';
import './wc-review-card.js';
import type { Preview } from '../../preview/types.js';

const preview: Preview = {
  id: 'wc-review-card',
  title: 'Review Card',
  group: 'Data',
  description: 'The transaction under review — the TUI detail pane, as a card.',
  layout: 'stack',
  states: [
    {
      name: 'default',
      render: () =>
        html`<wc-review-card
          date="2025-03-04"
          description="ADOBE CREATIVE CLOUD"
          .amount=${-54.99}
          account-name="BofA Credit Card"
        ></wc-review-card>`,
    },
    {
      name: 'income',
      render: () =>
        html`<wc-review-card
          date="2025-03-01"
          description="ACME CORP INVOICE 1042"
          .amount=${7500}
          account-name="BofA Checking"
        ></wc-review-card>`,
    },
    {
      name: 'long-description',
      render: () =>
        html`<wc-review-card
          date="2025-02-18"
          description="SQ *THE VERY LONG COFFEE COMPANY NAME THAT BANKS LOVE TO TRUNCATE 0000123456789"
          .amount=${-18.4}
          account-name="BofA Credit Card"
        ></wc-review-card>`,
    },
    {
      name: 're-review',
      render: () =>
        html`<wc-review-card
          date="2025-03-04"
          description="ADOBE CREATIVE CLOUD"
          .amount=${-54.99}
          account-name="BofA Credit Card"
          current-category="Software / Subscriptions"
          current-vendor="Adobe Inc"
        ></wc-review-card>`,
    },
  ],
};

export default preview;
