import { invoke } from "@tauri-apps/api/core";
import { createDefaultState, normalizeDashboardState } from "../data/defaultState";
import type { AppSettings, DashboardState, ProviderEnvironment } from "../types";

const STORAGE_KEY = "token-anxiety-dashboard-state";
const isTauriRuntime = () =>
  typeof window !== "undefined" &&
  (window.location.hostname.endsWith("tauri.localhost") || window.location.protocol === "tauri:");

const sleep = (ms: number) => new Promise((resolve) => window.setTimeout(resolve, ms));

async function invokeWithRetry<T>(command: string, payload?: Record<string, unknown>): Promise<T> {
  let lastError: unknown;

  for (let attempt = 0; attempt < 8; attempt += 1) {
    try {
      return await invoke<T>(command, payload);
    } catch (error) {
      lastError = error;
      if (!isTauriRuntime()) {
        break;
      }

      await sleep(150);
    }
  }

  throw lastError;
}

export async function loadDashboardState(): Promise<DashboardState> {
  try {
    const state = await invokeWithRetry<DashboardState>("load_dashboard_state");
    return normalizeDashboardState(state);
  } catch {
    const raw = window.localStorage.getItem(STORAGE_KEY);
    if (!raw) {
      const state = createDefaultState();
      window.localStorage.setItem(STORAGE_KEY, JSON.stringify(state));
      return state;
    }

    return normalizeDashboardState(JSON.parse(raw) as DashboardState);
  }
}

export async function syncDashboardState(): Promise<DashboardState> {
  try {
    const state = await invokeWithRetry<DashboardState>("sync_dashboard_state");
    return normalizeDashboardState(state);
  } catch {
    if (isTauriRuntime()) {
      throw new Error("Failed to sync Tauri dashboard state");
    }

    return loadDashboardState();
  }
}

export async function saveDashboardState(state: DashboardState): Promise<void> {
  try {
    await invokeWithRetry("save_dashboard_state", { state });
    return;
  } catch {
    if (isTauriRuntime()) {
      throw new Error("Failed to persist Tauri dashboard state");
    }

    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(state));
  }
}

export async function scanProviderEnvironment(): Promise<ProviderEnvironment[]> {
  try {
    return invokeWithRetry<ProviderEnvironment[]>("scan_provider_environment");
  } catch {
    if (isTauriRuntime()) {
      throw new Error("Failed to scan Tauri provider environment");
    }

    return [
      {
        provider: "claude-code",
        label: "Claude Code",
        detected: false,
        source: "~/.claude",
        detail: "瀏覽器預覽模式無法讀取本機 CLI。",
      },
      {
        provider: "codex",
        label: "Codex",
        detected: false,
        source: "~/.codex",
        detail: "瀏覽器預覽模式無法讀取本機 CLI。",
      },
      {
        provider: "antigravity",
        label: "Antigravity",
        detected: false,
        source: "Windows 憑證管理員: gemini:antigravity",
        detail: "瀏覽器預覽模式無法讀取本機憑證。",
      },
      {
        provider: "opencode-go",
        label: "OpenCode",
        detected: false,
        source: "~/.local/share/opencode/auth.json",
        detail: "瀏覽器預覽模式無法讀取 OpenCode auth。",
      },
    ];
  }
}

export async function toggleDevtools(): Promise<void> {
  if (!isTauriRuntime()) return;
  await invoke("toggle_devtools");
}

export async function getDebugLogs(): Promise<string[]> {
  if (!isTauriRuntime()) return [];
  return invoke<string[]>("get_debug_logs");
}

export async function loadSettings(): Promise<AppSettings> {
  try {
    return await invokeWithRetry<AppSettings>("get_settings");
  } catch {
    return {
      locale: "zh-TW",
      theme: "aurora",
    };
  }
}

export async function saveSettings(settings: AppSettings): Promise<void> {
  try {
    await invokeWithRetry("set_settings", { settings });
  } catch {
    // ignore
  }
}

export { isTauriRuntime };
