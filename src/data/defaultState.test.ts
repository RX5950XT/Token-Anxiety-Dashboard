import { describe, expect, it } from "vitest";
import { normalizeDashboardState } from "./defaultState";
import type { DashboardState } from "../types";

describe("normalizeDashboardState", () => {
  it("preserves backend-provided windows and notes", () => {
    const state: DashboardState = {
      settings: {
        locale: "zh-TW",
        theme: "aurora",
      },
      accounts: [
        {
          id: "claude-main",
          provider: "claude-code",
          accountName: "Claude Code",
          planName: "Claude Pro",
          status: "available",
          accuracy: "local",
          lastUpdated: new Date().toISOString(),
          windows: [],
          notes: "real backend state",
          order: 0,
        },
      ],
    };

    const normalized = normalizeDashboardState(state);
    const claude = normalized.accounts.find((account) => account.provider === "claude-code");

    expect(claude?.windows).toEqual([]);
    expect(claude?.notes).toBe("real backend state");
    expect(claude?.planName).toBe("Claude Pro");
  });
});
