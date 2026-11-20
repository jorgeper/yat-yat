// Browser mock of the Tauri IPC surface. Drives the E2E suite (SPEC §8.3):
// simulated download state machine, hotkey capture, enhancement test, and a
// window.__mock handle so Playwright can steer failure paths and overlay events.

import type { Ipc } from "./api";
import type {
  DownloadProgress,
  HistoryEntry,
  ModelStatus,
  Settings,
} from "./types";
import { defaultSettings } from "./types";

type Handler = (payload: unknown) => void;

const listeners = new Map<string, Set<Handler>>();

function emit(event: string, payload: unknown) {
  listeners.get(event)?.forEach((h) => h(payload));
}

const CATALOG: Omit<ModelStatus, "downloaded" | "downloading" | "partial_bytes" | "active">[] = [
  {
    id: "parakeet-tdt-0.6b-v3",
    display_name: "Parakeet V3",
    engine: "parakeet",
    url: "https://huggingface.co/istupakov/parakeet-tdt-0.6b-v3-onnx/resolve/8f23f0c03c8761650bdb5b40aaf3e40d2c15f1ce/",
    sha256: "5ee7d8e77fb0d19d720d157d69844be39051f112488360afeec628b109242c93",
    size_bytes: 670619803,
    languages: "25 European languages",
    description: "Fast and accurate — the best balance of speed and quality. Recommended.",
    recommended: true,
    kind: "files",
    filename: "",
    files: [
      { name: "encoder-model.int8.onnx", sha256: "6139d2fa7e1b086097b277c7149725edbab89cc7c7ae64b23c741be4055aff09", size_bytes: 652183999 },
      { name: "decoder_joint-model.int8.onnx", sha256: "eea7483ee3d1a30375daedc8ed83e3960c91b098812127a0d99d1c8977667a70", size_bytes: 18202004 },
      { name: "nemo128.onnx", sha256: "a9fde1486ebfcc08f328d75ad4610c67835fea58c73ba57e3209a6f6cf019e9f", size_bytes: 139764 },
      { name: "vocab.txt", sha256: "d58544679ea4bc6ac563d1f545eb7d474bd6cfa467f0a6e2c1dc1c7d37e3c35d", size_bytes: 93939 },
      { name: "config.json", sha256: "666903c76b9798caf2c210afd4f6cd60b08a8dbf9800ec8d7a3bc0d2148ac466", size_bytes: 97 },
    ],
  },
  {
    id: "whisper-large-v3-turbo",
    display_name: "Whisper Large v3 Turbo",
    engine: "whisper",
    url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo.bin",
    sha256: "1fc70f774d38eb169993ac391eea357ef47c88757ef72ee5943879b7e8e2bc69",
    size_bytes: 1624555275,
    languages: "99 languages",
    description: "Best multilingual coverage. Larger download, slower than Parakeet.",
    recommended: false,
    kind: "file",
    filename: "ggml-large-v3-turbo.bin",
  },
  {
    id: "whisper-small",
    display_name: "Whisper Small",
    engine: "whisper",
    url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin",
    sha256: "1be3a9b2063867b937e64e2ec7483364a79917e157fa98c5d94b5c1fffea987b",
    size_bytes: 487601967,
    languages: "99 languages",
    description: "Mid-size multilingual Whisper. Good quality on modest hardware.",
    recommended: false,
    kind: "file",
    filename: "ggml-small.bin",
  },
  {
    id: "whisper-tiny",
    display_name: "Whisper Tiny",
    engine: "whisper",
    url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.bin",
    sha256: "be07e048e1e599ad46341c8d2a135645097a538221678b7acdd1b1919c6e1b21",
    size_bytes: 77691713,
    languages: "99 languages",
    description: "Quick start — small download, fastest transcription, lowest accuracy.",
    recommended: false,
    kind: "file",
    filename: "ggml-tiny.bin",
  },
  {
    id: "parakeet-tdt-0.6b-v3-full",
    display_name: "Parakeet V3 (full precision)",
    engine: "parakeet",
    url: "https://huggingface.co/istupakov/parakeet-tdt-0.6b-v3-onnx/resolve/8f23f0c03c8761650bdb5b40aaf3e40d2c15f1ce/",
    sha256: "8b6affd8668c4c7c71e7fb6ec90fc780c402177c26800bf4d1927f73b5c5fbec",
    size_bytes: 2549945719,
    languages: "25 European languages",
    description:
      "The uncompressed Parakeet V3. Steadier than the standard one — it does not skip sentences on quiet or noisy recordings — for a larger download and somewhat more time and memory.",
    recommended: false,
    kind: "files",
    filename: "",
    precision: "fp32",
    files: [
      { name: "encoder-model.onnx.data", sha256: "9a22d372c51455c34f13405da2520baefb7125bd16981397561423ed32d24f36", size_bytes: 2435420160 },
      { name: "decoder_joint-model.onnx", sha256: "e978ddf6688527182c10fde2eb4b83068421648985ef23f7a86be732be8706c1", size_bytes: 72520893 },
      { name: "encoder-model.onnx", sha256: "98a74b21b4cc0017c1e7030319a4a96f4a9506e50f0708f3a516d02a77c96bb1", size_bytes: 41770866 },
      { name: "nemo128.onnx", sha256: "a9fde1486ebfcc08f328d75ad4610c67835fea58c73ba57e3209a6f6cf019e9f", size_bytes: 139764 },
      { name: "vocab.txt", sha256: "d58544679ea4bc6ac563d1f545eb7d474bd6cfa467f0a6e2c1dc1c7d37e3c35d", size_bytes: 93939 },
      { name: "config.json", sha256: "666903c76b9798caf2c210afd4f6cd60b08a8dbf9800ec8d7a3bc0d2148ac466", size_bytes: 97 },
    ],
  },
];

interface MockModelState {
  downloaded: boolean;
  downloading: boolean;
  partial_bytes: number;
  timer: ReturnType<typeof setInterval> | null;
}

const params = new URLSearchParams(window.location.search);
const onboardingMode = params.get("onboarding") === "1";
// ?grant=microphone,accessibility,capture,menubar pre-grants permissions
// (E9e resume tests).
const preGranted = new Set((params.get("grant") ?? "").split(",").filter(Boolean));

// Settings survive a reload within a test tab (E13a) via sessionStorage.
function loadPersistedSettings(): Partial<Settings> {
  try {
    return JSON.parse(sessionStorage.getItem("mock-settings") ?? "{}");
  } catch {
    return {};
  }
}

const state = {
  settings: {
    ...defaultSettings(),
    onboarding_complete: !onboardingMode,
    // Onboarding mode starts with no model at all (the model step is a gate).
    active_model: onboardingMode || params.get("noModel") === "1" ? null : "whisper-tiny",
    ...loadPersistedSettings(),
  } as Settings,
  // Onboarding mode starts ungranted; the normal mode simulates a healthy,
  // fully configured app (recovery tests break it via __mock.revoke).
  permissions: {
    microphone: !onboardingMode || preGranted.has("microphone"),
    accessibility: !onboardingMode || preGranted.has("accessibility"),
    captureReady: !onboardingMode || preGranted.has("capture"),
    menubar: !onboardingMode || preGranted.has("menubar"),
  },
  models: new Map<string, MockModelState>(
    CATALOG.map((m) => [
      m.id,
      {
        downloaded:
          m.id === "whisper-tiny" && !onboardingMode && params.get("noModel") !== "1",
        downloading: false,
        partial_bytes: 0,
        timer: null,
      },
    ]),
  ),
  history: [
    "Ship the release notes on Wednesday.",
    "Remember to water the ficus before standup.",
    "The quarterly numbers look better than expected.",
    "Draft an intro paragraph for the design doc.",
    "Call the dentist about rescheduling Thursday.",
  ].map(
    (text, i): HistoryEntry => ({
      text,
      timestamp: new Date(Date.now() - i * 3_600_000).toISOString(),
      model_id: "whisper-tiny",
    }),
  ),
  failNextDownload: null as string | null,
  calls: [] as { command: string; args: Record<string, unknown> | undefined }[],
  captureTimer: null as ReturnType<typeof setTimeout> | null,
};

function modelList(): ModelStatus[] {
  return CATALOG.map((m) => {
    const s = state.models.get(m.id)!;
    return {
      ...m,
      downloaded: s.downloaded,
      downloading: s.downloading,
      partial_bytes: s.partial_bytes,
      active: state.settings.active_model === m.id,
    };
  });
}

function startDownload(modelId: string): Promise<void> {
  const meta = CATALOG.find((m) => m.id === modelId);
  const s = state.models.get(modelId);
  if (!meta || !s) return Promise.reject(new Error(`unknown model '${modelId}'`));
  if (s.downloading) return Promise.reject(new Error("already downloading"));
  s.downloading = true;
  const shouldFail = state.failNextDownload === modelId;

  return new Promise<void>((resolve, reject) => {
    let pct = Math.round((s.partial_bytes / meta.size_bytes) * 100);
    s.timer = setInterval(() => {
      pct += 10;
      s.partial_bytes = Math.round((pct / 100) * meta.size_bytes);
      if (shouldFail && pct >= 40) {
        clearInterval(s.timer!);
        s.timer = null;
        s.downloading = false;
        state.failNextDownload = null;
        emit("model-download-failed", {
          model_id: modelId,
          error: "checksum mismatch — the download was corrupt and has been removed",
        });
        reject(new Error("checksum mismatch"));
        return;
      }
      if (pct >= 100) {
        clearInterval(s.timer!);
        s.timer = null;
        emit("model-download-progress", {
          model_id: modelId,
          downloaded: meta.size_bytes,
          total: meta.size_bytes,
          percentage: 100,
        } satisfies DownloadProgress);
        emit("model-verification-started", modelId);
        setTimeout(() => {
          emit("model-verification-completed", modelId);
          s.downloading = false;
          s.downloaded = true;
          s.partial_bytes = 0;
          emit("model-download-complete", modelId);
          resolve();
        }, 600);
        return;
      }
      emit("model-download-progress", {
        model_id: modelId,
        downloaded: s.partial_bytes,
        total: meta.size_bytes,
        percentage: pct,
      } satisfies DownloadProgress);
    }, 60);
  });
}

async function invoke(command: string, args?: Record<string, unknown>): Promise<unknown> {
  state.calls.push({ command, args });
  switch (command) {
    case "get_settings":
      return structuredClone(state.settings);
    case "set_settings": {
      // Mirrors the Rust command: active_model and last_run_version are
      // server-owned and a whole-object UI write never clobbers them.
      const incoming = structuredClone(args!.settings as Settings);
      incoming.active_model = state.settings.active_model;
      incoming.last_run_version = state.settings.last_run_version;
      state.settings = incoming;
      try {
        sessionStorage.setItem("mock-settings", JSON.stringify(state.settings));
      } catch {
        /* storage unavailable */
      }
      return;
    }
    case "list_models":
      return modelList();
    case "download_model":
      return startDownload(args!.modelId as string);
    case "cancel_download": {
      const s = state.models.get(args!.modelId as string);
      if (s?.timer) {
        clearInterval(s.timer);
        s.timer = null;
        s.downloading = false;
        emit("model-download-cancelled", args!.modelId);
        return true;
      }
      return false;
    }
    case "delete_model": {
      const s = state.models.get(args!.modelId as string)!;
      s.downloaded = false;
      s.partial_bytes = 0;
      if (state.settings.active_model === args!.modelId) {
        state.settings.active_model = null;
      }
      emit("model-deleted", args!.modelId);
      return;
    }
    case "set_active_model": {
      const s = state.models.get(args!.modelId as string);
      if (!s?.downloaded) throw new Error("model is not downloaded");
      state.settings.active_model = args!.modelId as string;
      return;
    }
    case "get_history":
      return structuredClone(state.history);
    case "clear_history":
      state.history = [];
      emit("history-changed", null);
      return;
    case "copy_text":
      return;
    case "retry_last_transcription":
    case "toggle_dictation":
    case "cancel_dictation":
    case "resolve_focus_prompt":
    case "uninstall_app":
    case "wiggle_tray":
      return;
    case "get_uninstall_plan": {
      // Mirrors the Rust plan shape (SPEC11 §1) for E16.
      const keep = args?.keepData === true;
      const items = [
        { path: "/mock/Application Support/com.yatyat.app/models", kind: "models", bytes: 2040109466 },
        ...(keep
          ? []
          : [{ path: "/mock/Application Support/com.yatyat.app", kind: "app_data", bytes: 153600 }]),
        { path: "/mock/Preferences/com.yatyat.app.plist", kind: "preferences", bytes: 4096 },
        { path: "/mock/Caches/com.yatyat.app", kind: "caches", bytes: 1048576 },
      ];
      return items;
    }
    case "test_enhancement": {
      const sample = (args?.sample as string) || "";
      if (!state.settings.enhancement.endpoint.match(/^https?:\/\/(localhost|127\.0\.0\.1)/)) {
        throw new Error("enhancement endpoint must be localhost");
      }
      await new Promise((r) => setTimeout(r, 150));
      return sample
        ? "I think we should ship it on Wednesday."
        : "Cleaned sample text.";
    }
    case "list_input_devices":
      return ["MacBook Pro Microphone", "External USB Mic"];
    case "start_hotkey_capture":
      if (!state.permissions.captureReady) {
        throw new Error("keyboard capture is not initialized (grant Accessibility first)");
      }
      state.captureTimer = setTimeout(() => {
        emit("hotkey-capture-event", { hotkey_string: "command_right", is_key_down: true });
        setTimeout(() => {
          emit("hotkey-capture-event", { hotkey_string: "command_right", is_key_down: false });
        }, 120);
      }, 250);
      return;
    case "stop_hotkey_capture":
      if (state.captureTimer) clearTimeout(state.captureTimer);
      return;
    case "initialize_capture":
      return state.permissions.captureReady;
    case "get_app_info":
      return {
        version: "0.1.0-mock",
        can_paste: state.permissions.accessibility,
        capture_ready: state.permissions.captureReady,
        platform: "macos",
      };
    case "open_settings":
      return;
    case "tray_item_visible":
      return state.permissions.menubar;
    case "open_menu_bar_settings":
      return;
    case "list_user_themes":
      return [
        {
          id: "user:midnight-ocean",
          name: "Midnight Ocean",
          variant: "dark",
          css: ".nh-theme { --nh-pill-bg: rgba(11, 22, 34, 0.92); --nh-fx-primary: rgba(74, 168, 255, 1); --nh-fx-accent: rgba(159, 208, 255, 1); --nh-fx-glow: rgba(74, 168, 255, 0.8); --nh-text: rgba(216, 226, 236, 1); --nh-pill-border: rgba(74, 168, 255, 0.25); --nh-tentative-opacity: 0.55; --nh-placeholder: rgba(216, 226, 236, 0.45); --nh-timer: rgba(216, 226, 236, 0.7); --nh-rec-dot: rgba(255, 92, 119, 1); --nh-font: system-ui; }",
          reason: null,
        },
        {
          id: "user:broken-remote",
          name: "Broken Remote",
          variant: "dark",
          css: "",
          reason: "references a remote url() — Yat Yat themes are offline-only",
        },
      ];
    default:
      throw new Error(`mock: unhandled command '${command}'`);
  }
}

export const mockIpc: Ipc = {
  invoke: invoke as Ipc["invoke"],
  listen: async (event, handler) => {
    const set = listeners.get(event) ?? new Set();
    set.add(handler as Handler);
    listeners.set(event, set);
    return () => set.delete(handler as Handler);
  },
};

// The permission facade used in browser mode (SPEC2 §4).
export const mockPerms = {
  checkMicrophone: async () => state.permissions.microphone,
  requestMicrophone: async () => {},
  checkAccessibility: async () => state.permissions.accessibility,
  requestAccessibility: async () => {},
  checkTrayVisible: async () => state.permissions.menubar,
  openMenuBarSettings: async () => {
    state.calls.push({ command: "open_menu_bar_settings", args: undefined });
  },
  checkCaptureReady: async () => state.permissions.captureReady,
};

export type Grantable = "microphone" | "accessibility" | "capture" | "menubar";

// Playwright drives failure paths, permission grants, and overlay events
// through this handle.
declare global {
  interface Window {
    __mock?: {
      emit: (event: string, payload: unknown) => void;
      failNextDownload: (modelId: string) => void;
      calls: () => { command: string; args: Record<string, unknown> | undefined }[];
      captureKeys: (hotkeyString: string) => void;
      grant: (what: Grantable) => void;
      revoke: (what: Grantable) => void;
      seedDeferrals: (steps: string[]) => void;
    };
  }
}

window.__mock = {
  emit,
  failNextDownload: (modelId: string) => {
    state.failNextDownload = modelId;
  },
  calls: () => state.calls,
  captureKeys: (hotkeyString: string) => {
    emit("hotkey-capture-event", { hotkey_string: hotkeyString, is_key_down: true });
    setTimeout(() => {
      emit("hotkey-capture-event", { hotkey_string: hotkeyString, is_key_down: false });
    }, 50);
  },
  grant: (what: Grantable) => {
    if (what === "microphone") state.permissions.microphone = true;
    if (what === "accessibility") state.permissions.accessibility = true;
    if (what === "menubar") state.permissions.menubar = true;
    if (what === "capture") {
      state.permissions.captureReady = true;
      emit("capture-ready", true);
    }
  },
  revoke: (what: Grantable) => {
    if (what === "microphone") state.permissions.microphone = false;
    if (what === "accessibility") state.permissions.accessibility = false;
    if (what === "menubar") state.permissions.menubar = false;
    if (what === "capture") state.permissions.captureReady = false;
  },
  seedDeferrals: (steps: string[]) => {
    state.settings.onboarding_skips = [...steps];
  },
};
