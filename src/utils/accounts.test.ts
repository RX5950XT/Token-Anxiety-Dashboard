import { describe, expect, it } from "vitest";
import { reorderAccounts } from "./accounts";

describe("account utilities", () => {
  it("reorders accounts immutably", () => {
    const accounts = [
      { id: "a", order: 0 },
      { id: "b", order: 1 },
      { id: "c", order: 2 },
    ];

    const nextAccounts = reorderAccounts(accounts, "c", "a");

    expect(nextAccounts.map((account) => account.id)).toEqual(["c", "a", "b"]);
    expect(accounts.map((account) => account.id)).toEqual(["a", "b", "c"]);
  });
});
