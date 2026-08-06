export interface Route {
  previewId?: string;
  stateName?: string;
  mode?: 'preview';
}

export function parseRoute(url: string): Route {
  const u = new URL(url);
  const route: Route = {};
  const previewId = u.searchParams.get('preview');
  if (previewId) route.previewId = previewId;
  const stateName = u.searchParams.get('state');
  if (stateName) route.stateName = stateName;
  const mode = u.searchParams.get('mode');
  if (mode === 'preview') route.mode = 'preview';
  return route;
}

export function routeToUrl(route: Route): string {
  const params = new URLSearchParams();
  if (route.previewId) params.set('preview', route.previewId);
  if (route.stateName) params.set('state', route.stateName);
  if (route.mode) params.set('mode', route.mode);
  const qs = params.toString();
  return qs ? `?${qs}` : '/';
}
