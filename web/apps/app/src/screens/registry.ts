import type { TemplateResult } from 'lit';
import type { NavItem } from '@nigel/ui';

import type { ScreenContext } from './context.js';

import { renderDashboard } from './dashboard.js';
import { renderRegister } from './register.js';
import { renderReview } from './review.js';
import { renderImport } from './import.js';
import { renderReports } from './reports.js';
import { renderAccounts } from './accounts.js';
import { renderCategories } from './categories.js';
import { renderRules } from './rules.js';
import { renderReconcile } from './reconcile.js';
import { renderUndo } from './undo.js';
import { renderSettings } from './settings.js';
import { renderUnlock } from './unlock.js';

export type ScreenId =
  | 'dashboard'
  | 'register'
  | 'review'
  | 'import'
  | 'reports'
  | 'accounts'
  | 'categories'
  | 'rules'
  | 'reconcile'
  | 'undo'
  | 'settings'
  | 'unlock';

export interface ScreenDef {
  id: ScreenId;
  /** Header and document title. */
  title: string;
  /** Sidebar label — usually shorter than the title. */
  navLabel: string;
  /** Tag name of a `wc-icon-*` element. */
  icon: string;
  /** Whether the screen appears in the sidebar. */
  inNav: boolean;
  /**
   * Render the screen. The context carries the api client, the route's query
   * parameters, and navigation — everything a screen needs from the shell,
   * handed over rather than imported, so a screen can be driven by a fake.
   */
  render: (ctx: ScreenContext) => TemplateResult;
}

/**
 * The one place a screen is described.
 *
 * boxcraft keeps screen identity in three places — a union, a title map, and a
 * render switch — which drift. Here the sidebar, the header title and the
 * content area all read this object, so adding a screen is one entry. Typing
 * it as `Record<ScreenId, ScreenDef>` makes a missing entry a compile error
 * rather than a blank page.
 */
const DEFS: Record<ScreenId, ScreenDef> = {
  dashboard: {
    id: 'dashboard',
    title: 'Dashboard',
    navLabel: 'Dashboard',
    icon: 'wc-icon-dashboard',
    inNav: true,
    render: renderDashboard,
  },
  register: {
    id: 'register',
    title: 'Register',
    navLabel: 'Register',
    icon: 'wc-icon-register',
    inNav: true,
    render: renderRegister,
  },
  review: {
    id: 'review',
    title: 'Review',
    navLabel: 'Review',
    icon: 'wc-icon-review',
    inNav: true,
    render: renderReview,
  },
  import: {
    id: 'import',
    title: 'Import',
    navLabel: 'Import',
    icon: 'wc-icon-import',
    inNav: true,
    render: renderImport,
  },
  reports: {
    id: 'reports',
    title: 'Reports',
    navLabel: 'Reports',
    icon: 'wc-icon-report',
    inNav: true,
    render: renderReports,
  },
  accounts: {
    id: 'accounts',
    title: 'Accounts',
    navLabel: 'Accounts',
    icon: 'wc-icon-account',
    inNav: true,
    render: renderAccounts,
  },
  categories: {
    id: 'categories',
    title: 'Categories',
    navLabel: 'Categories',
    icon: 'wc-icon-category',
    inNav: true,
    render: renderCategories,
  },
  rules: {
    id: 'rules',
    title: 'Rules',
    navLabel: 'Rules',
    icon: 'wc-icon-rule',
    inNav: true,
    render: renderRules,
  },
  reconcile: {
    id: 'reconcile',
    title: 'Reconcile',
    navLabel: 'Reconcile',
    icon: 'wc-icon-reconcile',
    inNav: true,
    render: renderReconcile,
  },
  undo: {
    id: 'undo',
    title: 'Undo last import',
    navLabel: 'Undo',
    icon: 'wc-icon-undo',
    inNav: true,
    render: renderUndo,
  },
  settings: {
    id: 'settings',
    title: 'Settings',
    navLabel: 'Settings',
    icon: 'wc-icon-settings',
    inNav: true,
    render: renderSettings,
  },
  unlock: {
    id: 'unlock',
    title: 'Unlock',
    navLabel: 'Unlock',
    icon: 'wc-icon-lock',
    // Reached only through the locked gate, never by choice.
    inNav: false,
    render: renderUnlock,
  },
};

export const SCREENS: ReadonlyMap<ScreenId, ScreenDef> = new Map(
  (Object.keys(DEFS) as ScreenId[]).map((id) => [id, DEFS[id]]),
);

export const DEFAULT_SCREEN: ScreenId = 'dashboard';

export function isScreenId(value: string): value is ScreenId {
  return SCREENS.has(value as ScreenId);
}

export function screenDef(id: ScreenId): ScreenDef {
  const def = SCREENS.get(id);
  if (!def) throw new Error(`unknown screen: ${id}`);
  return def;
}

/** Sidebar items, derived from the same registry the content area reads. */
export function navItems(options: { disabled?: boolean } = {}): NavItem[] {
  return [...SCREENS.values()]
    .filter((def) => def.inNav)
    .map((def) => ({
      id: def.id,
      label: def.navLabel,
      icon: def.icon,
      disabled: options.disabled ?? false,
    }));
}
