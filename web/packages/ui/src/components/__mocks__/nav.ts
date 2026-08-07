import type { NavItem } from '../wc-nav-sidebar.js';

/** A nav list shaped like the app's real one, for previews and tests. */
export const NAV_ITEMS: NavItem[] = [
  { id: 'dashboard', label: 'Dashboard', icon: 'wc-icon-dashboard' },
  { id: 'register', label: 'Register', icon: 'wc-icon-register' },
  { id: 'review', label: 'Review', icon: 'wc-icon-review' },
  { id: 'import', label: 'Import', icon: 'wc-icon-import' },
  { id: 'reports', label: 'Reports', icon: 'wc-icon-report' },
  { id: 'settings', label: 'Settings', icon: 'wc-icon-settings' },
];

export const NAV_ITEMS_WITH_DISABLED: NavItem[] = NAV_ITEMS.map((item) =>
  item.id === 'dashboard' ? item : { ...item, disabled: true },
);
