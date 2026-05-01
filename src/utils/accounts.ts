export function reorderAccounts<T extends { id: string; order: number }>(
  accounts: T[],
  activeId: string,
  overId: string,
): T[] {
  const oldIndex = accounts.findIndex((account) => account.id === activeId);
  const newIndex = accounts.findIndex((account) => account.id === overId);

  if (oldIndex < 0 || newIndex < 0 || oldIndex === newIndex) {
    return accounts;
  }

  const nextAccounts = [...accounts];
  const [moved] = nextAccounts.splice(oldIndex, 1);
  nextAccounts.splice(newIndex, 0, moved);

  return nextAccounts.map((account, index) => ({ ...account, order: index }));
}
