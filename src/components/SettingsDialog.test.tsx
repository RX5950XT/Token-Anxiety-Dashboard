import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { SettingsDialog } from "./SettingsDialog";
import type { AppSettings } from "../types";

describe("SettingsDialog", () => {
  // Rust's Option<Vec<String>> serializes None as JSON `null`, so get_settings
  // can return visibleProviders: null (not undefined). The dialog must render
  // instead of throwing "Cannot read properties of null (reading 'includes')".
  it("renders when visibleProviders is null", () => {
    const settings = {
      locale: "zh-TW",
      theme: "aurora",
      visibleProviders: null,
    } as unknown as AppSettings;

    expect(() =>
      render(<SettingsDialog settings={settings} onChange={() => {}} onClose={() => {}} />),
    ).not.toThrow();
    expect(screen.getByText("顯示項目")).toBeInTheDocument();
    // All providers default to visible when not configured.
    const checkboxes = screen.getAllByRole("checkbox") as HTMLInputElement[];
    expect(checkboxes.length).toBe(4);
    expect(checkboxes.every((c) => c.checked)).toBe(true);
  });
});
