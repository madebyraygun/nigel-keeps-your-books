import { describe, it, expect } from 'vitest';
import {
  DEFAULT_SCREEN,
  SCREENS,
  isScreenId,
  navItems,
  screenDef,
  type ScreenId,
} from './registry.js';
import { ICON_TAGS } from '@nigel/ui';
import { FakeApiClient } from '../__mocks__/fake-api-client.js';
import type { ScreenContext } from './context.js';

const ctx: ScreenContext = {
  client: new FakeApiClient(),
  params: new URLSearchParams(),
  navigate: () => {},
};

const ALL: ScreenId[] = [
  'dashboard',
  'register',
  'review',
  'import',
  'reports',
  'accounts',
  'categories',
  'rules',
  'clients',
  'invoices',
  'reconcile',
  'undo',
  'settings',
  'unlock',
];

describe('screen registry', () => {
  it('covers every screen the epic plans', () => {
    expect([...SCREENS.keys()].sort()).toEqual([...ALL].sort());
  });

  it.each(ALL)('%s resolves to a definition with its own id', (id) => {
    const def = screenDef(id);
    expect(def.id).toBe(id);
    expect(def.title.length).toBeGreaterThan(0);
    expect(def.navLabel.length).toBeGreaterThan(0);
  });

  it('names only icons the library actually registers', () => {
    for (const def of SCREENS.values()) {
      expect(ICON_TAGS, `${def.id} icon`).toContain(def.icon);
    }
  });

  it('gives every screen a distinct title', () => {
    const titles = [...SCREENS.values()].map((d) => d.title);
    expect(new Set(titles).size).toBe(titles.length);
  });

  it('renders a template for each screen', () => {
    for (const def of SCREENS.values()) {
      expect(def.render(ctx), `${def.id} render`).toBeTruthy();
    }
  });

  it('defaults to the dashboard', () => {
    expect(DEFAULT_SCREEN).toBe('dashboard');
    expect(SCREENS.has(DEFAULT_SCREEN)).toBe(true);
  });

  describe('isScreenId', () => {
    it.each(ALL)('accepts %s', (id) => expect(isScreenId(id)).toBe(true));

    it.each(['', 'nope', 'DASHBOARD', '../etc/passwd'])(
      'rejects %s',
      (value) => expect(isScreenId(value)).toBe(false),
    );
  });

  describe('navItems', () => {
    it('keeps unlock out of the sidebar', () => {
      // Unlock is reached through the locked gate, never by choice.
      expect(navItems().map((i) => i.id)).not.toContain('unlock');
    });

    it('lists every screen but the unlock gate, in registry order', () => {
      expect(navItems().map((i) => i.id)).toEqual(ALL.filter((id) => id !== 'unlock'));
    });

    it('carries the label and icon from the registry', () => {
      const item = navItems().find((i) => i.id === 'register');
      expect(item).toEqual({
        id: 'register',
        label: 'Register',
        icon: 'wc-icon-register',
        disabled: false,
      });
    });

    it('can disable everything at once for the locked state', () => {
      expect(navItems({ disabled: true }).every((i) => i.disabled)).toBe(true);
    });
  });

  it('throws on an unknown screen rather than rendering nothing', () => {
    expect(() => screenDef('nope' as ScreenId)).toThrow(/unknown screen/);
  });
});
