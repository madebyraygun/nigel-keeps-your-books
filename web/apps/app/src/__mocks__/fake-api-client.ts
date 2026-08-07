import type { ApiClient } from '../api/client.js';
import type {
  AppSettings,
  ChangePasswordRequest,
  CompanyNameResponse,
  PasswordStateResponse,
  PingResponse,
  RemovePasswordRequest,
  SetPasswordRequest,
  StatusResponse,
  UnlockResponse,
  UpdateAppSettingsRequest,
} from '../api/types.js';

export const DEFAULT_APP_SETTINGS: AppSettings = {
  userName: 'Tester',
  updateCheck: true,
  lastUpdateCheck: null,
};

export const UNLOCKED_STATUS: StatusResponse = {
  initialized: true,
  encrypted: false,
  locked: false,
  companyName: 'Test Consultancy',
  version: '0.0.0-test',
  dataDir: '/tmp/nigel',
};

export const LOCKED_STATUS: StatusResponse = {
  ...UNLOCKED_STATUS,
  encrypted: true,
  locked: true,
  companyName: null,
};

/**
 * An ApiClient that never touches the network.
 *
 * Its existence is the point: if a screen can be driven by this, the api
 * interface really is a seam, and a Tauri or remote-server implementation can
 * take the same place without a component noticing.
 */
export class FakeApiClient implements ApiClient {
  status: StatusResponse = UNLOCKED_STATUS;
  statusError: Error | null = null;
  unlockError: Error | null = null;

  readonly calls: string[] = [];

  async ping(): Promise<PingResponse> {
    this.calls.push('ping');
    return { ok: true, version: this.status.version };
  }

  async getStatus(): Promise<StatusResponse> {
    this.calls.push('getStatus');
    if (this.statusError) throw this.statusError;
    return this.status;
  }

  async unlock(password: string): Promise<UnlockResponse> {
    this.calls.push(`unlock:${password}`);
    if (this.unlockError) throw this.unlockError;
    this.status = { ...this.status, locked: false };
    return { locked: false };
  }

  // -- settings -------------------------------------------------------------
  //
  // These record the method name only. `unlock` above records its argument
  // because the boot-order test asserts on it; there is no reason to put a
  // password anywhere else, even in a test double.

  appSettings: AppSettings = DEFAULT_APP_SETTINGS;
  settingsError: Error | null = null;

  async getAppSettings(): Promise<AppSettings> {
    this.calls.push('getAppSettings');
    if (this.settingsError) throw this.settingsError;
    return this.appSettings;
  }

  async updateAppSettings(input: UpdateAppSettingsRequest): Promise<AppSettings> {
    this.calls.push('updateAppSettings');
    if (this.settingsError) throw this.settingsError;
    this.appSettings = { ...this.appSettings, updateCheck: input.updateCheck };
    return this.appSettings;
  }

  async setCompanyName(name: string): Promise<CompanyNameResponse> {
    this.calls.push('setCompanyName');
    if (this.settingsError) throw this.settingsError;
    this.status = { ...this.status, companyName: name };
    return { companyName: name };
  }

  async setDataDir(path: string): Promise<StatusResponse> {
    this.calls.push('setDataDir');
    if (this.settingsError) throw this.settingsError;
    this.status = { ...this.status, dataDir: path };
    return this.status;
  }

  async setPassword(_input: SetPasswordRequest): Promise<PasswordStateResponse> {
    this.calls.push('setPassword');
    if (this.settingsError) throw this.settingsError;
    this.status = { ...this.status, encrypted: true, locked: false };
    return { encrypted: true, locked: false };
  }

  async changePassword(_input: ChangePasswordRequest): Promise<PasswordStateResponse> {
    this.calls.push('changePassword');
    if (this.settingsError) throw this.settingsError;
    return { encrypted: true, locked: false };
  }

  async removePassword(_input: RemovePasswordRequest): Promise<PasswordStateResponse> {
    this.calls.push('removePassword');
    if (this.settingsError) throw this.settingsError;
    this.status = { ...this.status, encrypted: false, locked: false };
    return { encrypted: false, locked: false };
  }
}
