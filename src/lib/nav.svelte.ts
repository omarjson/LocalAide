// Requests from the command palette to open a part of a screen. Screens read
// these with $effect; `tick` changes on every request so a repeat still fires.

export type SettingsSectionId = "general" | "online" | "web" | "privacy" | "backups" | "activity" | "about";

export const settingsRequest = $state<{ section: SettingsSectionId | null; checkUpdates: boolean; tick: number }>({
  section: null,
  checkUpdates: false,
  tick: 0,
});

export function requestSettings(section: SettingsSectionId, checkUpdates = false): void {
  settingsRequest.section = section;
  settingsRequest.checkUpdates = checkUpdates;
  settingsRequest.tick += 1;
}
