import { html } from 'lit';
import './wc-link-grid.js';
import type { Preview } from '../../preview/types.js';
import type { LinkGridItem } from './wc-link-grid.js';

const reports: LinkGridItem[] = [
  {
    href: '#/reports?report=pnl',
    label: 'Profit and loss',
    description: 'Income and expenses by category, with the net for the period.',
    icon: 'wc-icon-report',
  },
  {
    href: '#/reports?report=expenses',
    label: 'Expenses',
    description: 'Spending by category, with each share of the total and the top vendors.',
    icon: 'wc-icon-report',
  },
  {
    href: '#/reports?report=tax',
    label: 'Tax summary',
    description: 'Categories grouped by the tax line they map to.',
    icon: 'wc-icon-report',
  },
  {
    href: '#/reports?report=cashflow',
    label: 'Cash flow',
    description: 'Money in and out by month, with a running balance.',
    icon: 'wc-icon-report',
  },
];

const preview: Preview = {
  id: 'wc-link-grid',
  title: 'Link grid',
  group: 'Navigation',
  layout: 'stack',
  description:
    'A directory of links as cards. Real anchors, so middle-click and open-in-new-tab work and the component stays ignorant of the router.',
  states: [
    {
      name: 'eight items',
      render: () => html`
        <wc-link-grid
          label="Reports"
          .items=${[
            ...reports,
            {
              href: '#/reports?report=balance',
              label: 'Cash position',
              description: 'Balance per account, with year-to-date net income.',
              icon: 'wc-icon-report',
            },
            {
              href: '#/reports?report=flagged',
              label: 'Flagged',
              description: 'Transactions marked for a second look.',
              icon: 'wc-icon-flag',
            },
            {
              href: '#/reports?report=register',
              label: 'Register',
              description: 'Every transaction for the period, read only.',
              icon: 'wc-icon-register',
            },
            {
              href: '#/reports?report=k1',
              label: 'K-1 worksheet',
              description: 'Form 1120-S preparation figures.',
              icon: 'wc-icon-report',
            },
          ] satisfies LinkGridItem[]}
        ></wc-link-grid>
      `,
    },
    {
      name: 'three items',
      render: () =>
        html`<wc-link-grid label="Reports" .items=${reports.slice(0, 3)}></wc-link-grid>`,
    },
    {
      name: 'no descriptions',
      render: () => html`
        <wc-link-grid
          label="Reports"
          .items=${reports.map(({ href, label }) => ({ href, label }))}
        ></wc-link-grid>
      `,
    },
    {
      name: 'compact',
      render: () =>
        html`<wc-link-grid compact label="Reports" .items=${reports}></wc-link-grid>`,
    },
  ],
};

export default preview;
