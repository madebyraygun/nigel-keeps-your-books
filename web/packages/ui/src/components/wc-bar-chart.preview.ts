import { html } from 'lit';
import './wc-bar-chart.js';
import type { BarBucket } from './wc-bar-chart.js';
import type { Preview } from '../../preview/types.js';

const MONTHS = [
  'Jan',
  'Feb',
  'Mar',
  'Apr',
  'May',
  'Jun',
  'Jul',
  'Aug',
  'Sep',
  'Oct',
  'Nov',
  'Dec',
];

const year: BarBucket[] = MONTHS.map((label, i) => ({
  label,
  income: 12000 + Math.round(Math.sin(i / 2) * 5200) + i * 240,
  expense: 8200 + Math.round(Math.cos(i / 3) * 3100) + i * 130,
}));

const preview: Preview = {
  id: 'wc-bar-chart',
  title: 'Bar chart',
  group: 'Data',
  description:
    'Monthly income against expenses, drawn in CSS. Bars are a picture, so the figures are repeated in a table only assistive tech reads.',
  layout: 'stack',
  states: [
    {
      name: 'twelve months',
      render: () =>
        html`<wc-bar-chart
          .buckets=${year}
          caption="2025 - 26"
        ></wc-bar-chart>`,
    },
    {
      name: 'short history',
      render: () =>
        html`<wc-bar-chart
          .buckets=${year.slice(0, 3)}
          caption="2026"
        ></wc-bar-chart>`,
    },
    {
      name: 'single month',
      render: () =>
        html`<wc-bar-chart .buckets=${year.slice(0, 1)} caption="2026"></wc-bar-chart>`,
    },
    {
      name: 'all zero',
      render: () =>
        html`<wc-bar-chart
          .buckets=${MONTHS.slice(0, 6).map((label) => ({
            label,
            income: 0,
            expense: 0,
          }))}
          caption="2026"
        ></wc-bar-chart>`,
    },
    {
      name: 'lopsided',
      render: () =>
        html`<wc-bar-chart
          .buckets=${[
            { label: 'Jan', income: 240000, expense: 1200 },
            { label: 'Feb', income: 900, expense: 800 },
            { label: 'Mar', income: 1500, expense: 96000 },
          ]}
          caption="2026"
        ></wc-bar-chart>`,
    },
    {
      name: 'empty',
      render: () =>
        html`<wc-bar-chart
          ><p slot="empty" style="color:var(--wa-color-muted);">
            No transactions yet.
          </p></wc-bar-chart
        >`,
    },
    { name: 'loading', render: () => html`<wc-bar-chart loading></wc-bar-chart>` },
    {
      name: 'error',
      render: () =>
        html`<wc-bar-chart
          error="Could not reach the nigel server."
        ></wc-bar-chart>`,
    },
  ],
};

export default preview;
