import { useCallback, useEffect, useMemo, useState } from "react";
import {
  closestCenter,
  DndContext,
  DragOverlay,
  PointerSensor,
  pointerWithin,
  useSensor,
  useSensors,
  type DragEndEvent,
  type DragStartEvent,
} from "@dnd-kit/core";
import { rectSortingStrategy, SortableContext } from "@dnd-kit/sortable";
import { RefreshCw, Settings } from "lucide-react";
import "./App.css";
import { AccountCard } from "./components/AccountCard";
import { Logo } from "./components/Logo";
import { SettingsDialog } from "./components/SettingsDialog";
import { createDefaultState } from "./data/defaultState";
import { t } from "./i18n";
import { getDebugLogs, isTauriRuntime, loadSettings, saveDashboardState, saveSettings, scanProviderEnvironment, syncDashboardState, toggleDevtools } from "./services/storage";
import type { AppSettings, DashboardState, ProviderEnvironment } from "./types";
import { reorderAccounts } from "./utils/accounts";



function App() {
  const [state, setState] = useState<DashboardState>(() => createDefaultState());
  const [environments, setEnvironments] = useState<ProviderEnvironment[]>([]);
  const [ready, setReady] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [activeId, setActiveId] = useState<string | null>(null);
  const [syncing, setSyncing] = useState(false);
  const [lastSyncedAt, setLastSyncedAt] = useState<number | null>(null);
  const sensors = useSensors(useSensor(PointerSensor, { activationConstraint: { distance: 1 } }));
  const sortedAccounts = useMemo(() => [...state.accounts].sort((a, b) => a.order - b.order), [state.accounts]);
  const activeAccount = useMemo(
    () => sortedAccounts.find((account) => account.id === activeId) ?? null,
    [activeId, sortedAccounts],
  );
  const environmentByProvider = useMemo(
    () => new Map(environments.map((environment) => [environment.provider, environment])),
    [environments],
  );

  const handleSync = useCallback(async () => {
    if (syncing) return;
    setSyncing(true);
    try {
      const [nextState, nextEnvironments] = await Promise.all([
        syncDashboardState(),
        scanProviderEnvironment(),
      ]);
      setState(nextState);
      setEnvironments(nextEnvironments);
      setReady(true);
      setLastSyncedAt(Date.now());
    } catch (error) {
      console.error("Failed to refresh dashboard state", error);
      if (!isTauriRuntime()) {
        setState(createDefaultState());
        setReady(true);
      }
    } finally {
      setSyncing(false);
      // Print backend diagnostic logs to browser console
      try {
        const logs = await getDebugLogs();
        if (logs.length > 0) {
          console.group("[Backend Diagnostics]");
          for (const line of logs) {
            console.log(line);
          }
          console.groupEnd();
        }
      } catch {
        // ignore
      }
    }
  }, [syncing, lastSyncedAt]);

  useEffect(() => {
    void handleSync();
    // Load persisted settings
    loadSettings().then((settings) => {
      setState((current) => ({ ...current, settings }));
    }).catch(() => {
      // ignore
    });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // F12 to toggle devtools
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === "F12") {
        e.preventDefault();
        void toggleDevtools();
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, []);

  useEffect(() => {
    if (ready) {
      void saveDashboardState(state);
    }
  }, [ready, state]);

  const updateSettings = useCallback((settings: AppSettings) => {
    setState((current) => ({ ...current, settings }));
    void saveSettings(settings);
  }, []);

  const handleDragEnd = useCallback((event: DragEndEvent) => {
    setActiveId(null);

    if (!event.over) {
      return;
    }

    setState((current) => ({
      ...current,
      accounts: reorderAccounts(current.accounts, String(event.active.id), String(event.over?.id)),
    }));
  }, []);

  const handleDragStart = useCallback((event: DragStartEvent) => {
    setActiveId(String(event.active.id));
  }, []);

  return (
    <main className={`app-shell theme-${state.settings.theme}`}>
      <header className="top-bar">
        <Logo locale={state.settings.locale} />
        <div style={{ display: "flex", alignItems: "center", gap: "12px" }}>
          <button
            className="settings-button"
            type="button"
            onClick={() => void handleSync()}
            disabled={syncing}
            title={lastSyncedAt ? new Date(lastSyncedAt).toLocaleTimeString() : ""}
          >
            <RefreshCw size={18} style={{ animation: syncing ? "spin 1s linear infinite" : "none" }} />
            {t(state.settings.locale, syncing ? "syncing" : "sync")}
          </button>
          <button className="settings-button" type="button" onClick={() => setSettingsOpen(true)}>
            <Settings size={18} />
            {t(state.settings.locale, "settings")}
          </button>
        </div>
      </header>
      <DndContext
        sensors={sensors}
        collisionDetection={(args) => {
          const pointerHits = pointerWithin(args);
          return pointerHits.length > 0 ? pointerHits : closestCenter(args);
        }}
        onDragStart={handleDragStart}
        onDragEnd={handleDragEnd}
        onDragCancel={() => setActiveId(null)}
      >
        <SortableContext items={sortedAccounts.map((account) => account.id)} strategy={rectSortingStrategy}>
          <section className="card-grid" aria-label={t(state.settings.locale, "cardsRegion")}>
            {sortedAccounts.map((account) => (
              <AccountCard
                account={account}
                environment={environmentByProvider.get(account.provider)}
                locale={state.settings.locale}
                key={account.id}
              />
            ))}
          </section>
        </SortableContext>
        <DragOverlay dropAnimation={null}>
          {activeAccount ? (
            <AccountCard
              account={activeAccount}
              environment={environmentByProvider.get(activeAccount.provider)}
              locale={state.settings.locale}
              overlay
            />
          ) : null}
        </DragOverlay>
      </DndContext>
      {settingsOpen ? (
        <SettingsDialog settings={state.settings} onChange={updateSettings} onClose={() => setSettingsOpen(false)} />
      ) : null}
    </main>
  );
}

export default App;
