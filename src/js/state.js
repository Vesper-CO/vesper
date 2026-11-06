import { invoke } from './api.js';

export const S = {
  view: 'home',
  instances: [],
  accounts: [],
  settings: { theme: 'dark', default_ram_mb: 4096, ms_client_id: '', java_path: '', auto_update: true },
  versions: null,
  running: {}, // instance id -> stage text
  logs: {},    // instance id -> [{line, stream}]
  target: null, // instance chosen in the Mod Library
  homeInstance: null,
};

export async function loadAll() {
  [S.instances, S.accounts, S.settings] = await Promise.all([invoke('list_instances'), invoke('list_accounts'), invoke('get_settings')]);
  applyTheme();
  const running = await invoke('running_instances');
  running.forEach((id) => (S.running[id] = 'Running'));
}
export const applyTheme = () => (document.documentElement.dataset.theme = S.settings.theme || 'dark');
export const mcVersions = async () => (S.versions ??= await invoke('list_mc_versions'));
export const activeAccount = () => S.accounts.find((a) => a.active) || S.accounts[0];
export const refreshInstances = async () => (S.instances = await invoke('list_instances'));
export const refreshAccounts = async () => (S.accounts = await invoke('list_accounts'));
