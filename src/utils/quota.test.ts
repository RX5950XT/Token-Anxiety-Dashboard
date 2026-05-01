import { describe, expect, it } from "vitest";
import { createDefaultState } from "../data/defaultState";
import { deriveAccountStatus, formatDurationUntil, getWindowProgress, summarizeDashboard } from "./quota";

describe("quota utilities", () => {
  it("computes window progress and remaining labels", () => {
    const progress = getWindowProgress({
      id: "opencode-5h",
      label: "5h",
      kind: "rolling-5h",
      used: 4.2,
      limit: 12,
      resetAt: new Date(1_000_000).toISOString(),
    }, 0);

    expect(progress.percentage).toBe(35);
    expect(progress.status).toBe("available");
  });

  it("marks accounts as warning or limited by the most severe window", () => {
    const state = createDefaultState(0);
    const account = {
      ...state.accounts[0],
      windows: [
        { ...state.accounts[0].windows[0], used: 84, limit: 100 },
        { ...state.accounts[0].windows[1], used: 100, limit: 100 },
      ],
    };

    expect(deriveAccountStatus(account, 0)).toBe("limited");
  });

  it("formats reset countdowns in days, hours, and minutes", () => {
    expect(formatDurationUntil(new Date(30 * 60_000).toISOString(), 0)).toBe("30 分");
    expect(formatDurationUntil(new Date(2 * 60 * 60_000 + 5 * 60_000).toISOString(), 0)).toBe("2 小時 5 分");
    expect(formatDurationUntil(new Date(2 * 24 * 60 * 60_000 + 60 * 60_000).toISOString(), 0)).toBe("2 天 1 小時");
    expect(formatDurationUntil(new Date(2 * 60 * 60_000 + 5 * 60_000).toISOString(), 0, "en")).toBe("2h 5m");
  });

  it("summarizes account counts and next reset", () => {
    const state = createDefaultState(0);
    const summary = summarizeDashboard(state, 0);

    expect(summary.accountCount).toBe(4);
    expect(summary.nextResetLabel).toBe("1 小時 36 分");
  });
});
