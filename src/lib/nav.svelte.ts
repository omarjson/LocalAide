// Requests to open a part of the app. App.svelte watches `screenRequest`
// and Settings watches `settingsRequest`; `tick` changes on every request so
// a repeat still fires.

export type SettingsSectionId =
  | "general"
  | "online"
  | "tools"
  | "web"
  | "voice"
  | "share"
  | "integrations"
  | "privacy"
  | "backups"
  | "activity"
  | "developer"
  | "about";

export const screenRequest = $state<{ screen: "settings" | "models" | null; tick: number }>({ screen: null, tick: 0 });

export const settingsRequest = $state<{ section: SettingsSectionId | null; checkUpdates: boolean; tick: number }>({
  section: null,
  checkUpdates: false,
  tick: 0,
});

export function requestSettings(section: SettingsSectionId, checkUpdates = false): void {
  settingsRequest.section = section;
  settingsRequest.checkUpdates = checkUpdates;
  settingsRequest.tick += 1;
  screenRequest.screen = "settings";
  screenRequest.tick += 1;
}

/** The Settings section last shown, so coming back to Settings keeps it. */
export const settingsView = $state<{ section: SettingsSectionId; handledTick: number }>({ section: "general", handledTick: 0 });
