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
  });

  it("switches visible UI copy to English", async () => {
    window.localStorage.clear();
    render(<App />);

    await userEvent.click(await screen.findByRole("button", { name: "設定" }));
    await userEvent.click(screen.getByRole("button", { name: "英文" }));

    expect(await screen.findByRole("button", { name: "Settings" })).toBeInTheDocument();
  });
});
