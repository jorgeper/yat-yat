// Permission checks/requests: routes to tauri-plugin-macos-permissions in the
// real app, to the mock's permission state machine in plain browsers (E2E).

import { api, isTauri } from "./api";

export interface Permissions {
  checkMicrophone(): Promise<boolean>;
  requestMicrophone(): Promise<void>;
  checkAccessibility(): Promise<boolean>;
  requestAccessibility(): Promise<void>;
  /** Ground truth from the Rust CGWindowList probe (SPEC2 FR-O2c). */
  checkTrayVisible(): Promise<boolean>;
  openMenuBarSettings(): Promise<void>;
  /** Is the global hotkey listener actually armed? */
  checkCaptureReady(): Promise<boolean>;
}

async function tauriPermissions(): Promise<Permissions> {
  const { invoke } = await import("@tauri-apps/api/core");
  const shared = {
    checkTrayVisible: () => invoke<boolean>("tray_item_visible"),
    openMenuBarSettings: async () => {
      await invoke("open_menu_bar_settings");
    },
    checkCaptureReady: async () => (await api.getAppInfo()).capture_ready,
  };

  // The macos-permissions plugin is only registered on macOS — calling it
  // elsewhere rejects, which blanked the whole wizard on Windows (SPEC10
  // fallout: readSnapshot's Promise.all never resolved).
  const { platform } = await api.getAppInfo();
  if (platform !== "macos") {
    // Windows: no per-app mic prompt exists for unpackaged desktop apps
    // (the global privacy toggle governs it — README points there), and
    // Accessibility/menu-bar gating are macOS concepts.
    return {
      ...shared,
      checkMicrophone: async () => true,
      requestMicrophone: async () => {},
      checkAccessibility: async () => true,
      requestAccessibility: async () => {},
    };
  }

  const plugin = await import("tauri-plugin-macos-permissions-api");
  return {
    ...shared,
    checkMicrophone: () => plugin.checkMicrophonePermission(),
    requestMicrophone: async () => {
      await plugin.requestMicrophonePermission();
    },
    checkAccessibility: () => plugin.checkAccessibilityPermission(),
    requestAccessibility: async () => {
      await plugin.requestAccessibilityPermission();
    },
  };
}

async function mockPermissions(): Promise<Permissions> {
  const { mockPerms } = await import("./mock");
  return mockPerms;
}

let cached: Promise<Permissions> | null = null;
export function permissions(): Promise<Permissions> {
  if (!cached) cached = isTauri() ? tauriPermissions() : mockPermissions();
  return cached;
}
