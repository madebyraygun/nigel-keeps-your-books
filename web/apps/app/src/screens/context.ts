import type { ApiClient } from '../api/index.js';
import type { ScreenId } from './registry.js';

/**
 * What a screen is handed when it renders.
 *
 * Screens reach the server through `client` rather than importing a singleton,
 * which is what keeps the api seam swappable: a test drives a whole screen with
 * `FakeApiClient`, and a Tauri or remote-server client can take the same place
 * without a screen noticing.
 *
 * Kept deliberately small. It is the interface every screen task after this one
 * builds on, so anything added here is added for eleven screens at once —
 * per-screen state belongs to the screen, and app-wide state stays in the store.
 */
export interface ScreenContext {
  client: ApiClient;
  /** The query part of `#/<screen>?<params>`, for deep links. */
  params: URLSearchParams;
  /** Navigate by writing the hash — the only writer of route state. */
  navigate(screen: ScreenId, params?: URLSearchParams): void;
}
