import { useEffect, useMemo, useState } from "react";
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
import { Settings } from "lucide-react";
import "./App.css";
import { AccountCard } from "./components/AccountCard";
import { Logo } from "./components/Logo";
import { SettingsDialog } from "./components/SettingsDialog";
import { createDefaultState } from "./data/defaultState";
import { t } from "./i18n";
import { isTauriRuntime, saveDashboardState, scanProviderEnvironment, syncDashboardState } from "./services/storage";
import type { AppSettings, DashboardState, ProviderEnvironment } from "./types";
import { reorderAccounts } from "./utils/accounts";

function App() {
  const [state, setState] = useState<DashboardState>(() => createDefaultState());
  const [environments, setEnvironments] = useState<ProviderEnvironment[]>([]);
  const [ready, setReady] = useState(false);
  const [syncError, setSyncError] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [activeId, setActiveId] = useState<string | null>(null);
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

  useEffect(() => {
    let cancelled = false;

    const refresh = async () => {
      try {
        const [nextState, nextEnvironments] = await Promise.all([
          syncDashboardState(),
          scanProviderEnvironment(),
        ]);

        if (cancelled) {
          return;
        }

        setState(nextState);
        setEnvironments(nextEnvironments);
        setSyncError(false);
        setReady(true);
      } catch (error) {
        if (!cancelled) {
          console.error("Failed to refresh dashboard state", error);
          if (isTauriRuntime()) {
            setSyncError(true);
            setReady(false);
            return;
          }

          setState(createDefaultState());
          setReady(true);
        }
      }
    };

    void refresh();
    const timer = window.setInterval(() => {
      void refresh();
    }, 60_000);

    return () => {
      cancelled = true;
      window.clearInterval(timer);
    };
  }, []);

  useEffect(() => {
    if (ready) {
      void saveDashboardState(state);
    }
  }, [ready, state]);

  const updateSettings = (settings: AppSettings) => {
    setState((current) => ({ ...current, settings }));
  };

  const handleDragEnd = (event: DragEndEvent) => {
    setActiveId(null);

    if (!event.over) {
      return;
    }

    setState((current) => ({
      ...current,
      accounts: reorderAccounts(current.accounts, String(event.active.id), String(event.over?.id)),
    }));
  };

  const handleDragStart = (event: DragStartEvent) => {
    setActiveId(String(event.active.id));
  };

  return (
    <main className={`app-shell theme-${state.settings.theme}`}>
      <header className="top-bar">
        <Logo locale={state.settings.locale} />
        <button className="settings-button" type="button" onClick={() => setSettingsOpen(true)}>
          <Settings size={18} />
          {t(state.settings.locale, "settings")}
        </button>
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
      <section className="data-notice" aria-label={t(state.settings.locale, "dataNoticeTitle")}>
        <strong>{t(state.settings.locale, "dataNoticeTitle")}</strong>
        <span>{syncError ? t(state.settings.locale, "dataNoticeError") : t(state.settings.locale, "dataNoticeBody")}</span>
      </section>
      {settingsOpen ? (
        <SettingsDialog settings={state.settings} onChange={updateSettings} onClose={() => setSettingsOpen(false)} />
      ) : null}
    </main>
  );
}

export default App;
