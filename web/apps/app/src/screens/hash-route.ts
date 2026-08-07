import { DEFAULT_SCREEN, isScreenId, type ScreenId } from './registry.js';

export interface Route {
  screen: ScreenId;
  params: URLSearchParams;
}

/**
 * Parse `#/<screen>?<params>`.
 *
 * Params are carried even though the scaffold has no use for them yet: screens
 * land later wanting deep links like `#/register?account=BofA%20Checking`, and
 * having the shape settled now means those tasks add a screen rather than
 * reopen the routing seam. Anything unrecognized falls back to the default
 * screen — a bad hash should show the dashboard, not a blank page.
 */
export function parseHash(hash: string): Route {
  const raw = hash.replace(/^#\/?/, '');
  const [path, query = ''] = raw.split('?');
  const screen = isScreenId(path) ? path : DEFAULT_SCREEN;
  return { screen, params: new URLSearchParams(query) };
}

export function routeToHash(route: Route): string {
  const query = route.params.toString();
  return query ? `#/${route.screen}?${query}` : `#/${route.screen}`;
}

export function screenToHash(screen: ScreenId): string {
  return `#/${screen}`;
}
