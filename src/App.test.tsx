import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import App from "./App";

describe("App", () => {
  it("renders the dashboard shell and seed provider cards", async () => {
    window.localStorage.clear();
    render(<App />);

    expect(await screen.findByText("Token Anxiety")).toBeInTheDocument();
    expect((await screen.findAllByText("Claude Code")).length).toBeGreaterThan(0);
    expect((await screen.findAllByText("OpenCode")).length).toBeGreaterThan(0);
    expect(screen.queryByLabelText("新增帳號")).not.toBeInTheDocument();
    expect(screen.queryByText("Desktop quota dashboard")).not.toBeInTheDocument();
    expect(screen.queryByText("Main workspace")).not.toBeInTheDocument();
    expect(screen.getByText("Codex 與 OpenCode 已改為讀取本機真實額度資料；Claude Code 與 Gemini CLI 目前先讀取真實登入與設定狀態。")).toBeInTheDocument();
  });

  it("switches visible UI copy to English", async () => {
    window.localStorage.clear();
    render(<App />);

    await userEvent.click(await screen.findByRole("button", { name: "設定" }));
    await userEvent.click(screen.getByRole("button", { name: "英文" }));

    expect(await screen.findByRole("button", { name: "Settings" })).toBeInTheDocument();
    expect(screen.getByText("Codex and OpenCode now read real local quota data. Claude Code and Gemini CLI currently read real sign-in and local settings only.")).toBeInTheDocument();
  });
});
