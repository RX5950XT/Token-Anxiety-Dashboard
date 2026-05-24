import { X } from "lucide-react";
import { t } from "../i18n";
import { providerOptions } from "../data/providers";
import type { AppSettings, LocaleId, ProviderId, ThemeId } from "../types";
import type { MessageKey } from "../i18n";

interface SettingsDialogProps {
  settings: AppSettings;
  onClose: () => void;
  onChange: (settings: AppSettings) => void;
}

const DAYS_OF_WEEK: { value: number; label: MessageKey }[] = [
  { value: 0, label: "sunday" },
  { value: 1, label: "monday" },
  { value: 2, label: "tuesday" },
  { value: 3, label: "wednesday" },
  { value: 4, label: "thursday" },
  { value: 5, label: "friday" },
  { value: 6, label: "saturday" },
];

export function SettingsDialog({ settings, onClose, onChange }: SettingsDialogProps) {
  const setLocale = (locale: LocaleId) => onChange({ ...settings, locale });
  const setTheme = (theme: ThemeId) => onChange({ ...settings, theme });
  const locales: LocaleId[] = ["zh-TW", "en"];
  const themes: ThemeId[] = ["aurora", "dark", "graphite", "light"];

  const allProviderIds = providerOptions.map((p) => p.id);
  const isProviderVisible = (id: ProviderId) =>
    settings.visibleProviders == null || settings.visibleProviders.includes(id);
  const setProviderVisible = (id: ProviderId, visible: boolean) => {
    const current = settings.visibleProviders ?? allProviderIds;
    const next = visible
      ? allProviderIds.filter((p) => current.includes(p) || p === id)
      : current.filter((p) => p !== id);
    onChange({ ...settings, visibleProviders: next });
  };

  const setWeeklyReset = (day: number, hour: number, minute: number) => {
    onChange({
      ...settings,
      opencodeWeeklyReset: { day, hour, minute },
    });
  };

  const setMonthlyReset = (day: number, hour: number, minute: number) => {
    onChange({
      ...settings,
      opencodeMonthlyReset: { day, hour, minute },
    });
  };

  const formatTime = (hour: number, minute: number) => {
    return `${String(hour).padStart(2, "0")}:${String(minute).padStart(2, "0")}`;
  };

  const parseTime = (timeStr: string) => {
    const [h, m] = timeStr.split(":").map(Number);
    return { hour: h || 0, minute: m || 0 };
  };

  return (
    <div className="modal-backdrop" role="presentation">
      <section className="modal" role="dialog" aria-modal="true" aria-label={t(settings.locale, "settings")}>
        <header>
          <h2>{t(settings.locale, "settings")}</h2>
          <button className="icon-button" type="button" onClick={onClose} aria-label={t(settings.locale, "closeSettings")}>
            <X size={18} />
          </button>
        </header>
        <div className="settings-group">
          <span className="settings-label">{t(settings.locale, "language")}</span>
          <div className="segmented-control" role="tablist" aria-label={t(settings.locale, "language")}>
            {locales.map((locale) => (
              <button
                key={locale}
                className={`segment-button ${settings.locale === locale ? "active" : ""}`}
                type="button"
                onClick={() => setLocale(locale)}
              >
                {locale === "zh-TW" ? t(settings.locale, "traditionalChinese") : t(settings.locale, "english")}
              </button>
            ))}
          </div>
        </div>
        <div className="settings-group">
          <span className="settings-label">{t(settings.locale, "theme")}</span>
          <div className="theme-grid" role="tablist" aria-label={t(settings.locale, "theme")}>
            {themes.map((theme) => (
              <button
                key={theme}
                className={`theme-tile theme-${theme} ${settings.theme === theme ? "active" : ""}`}
                type="button"
                onClick={() => setTheme(theme)}
              >
                <span className="theme-swatch" />
                <span>
                  {theme === "aurora" && t(settings.locale, "themeAurora")}
                  {theme === "dark" && t(settings.locale, "themeDark")}
                  {theme === "graphite" && t(settings.locale, "themeGraphite")}
                  {theme === "light" && t(settings.locale, "themeLight")}
                </span>
              </button>
            ))}
          </div>
        </div>

        {/* Displayed items */}
        <div className="settings-group">
          <span className="settings-label">{t(settings.locale, "displaySettings")}</span>
          <span className="reset-setting-value">{t(settings.locale, "displaySettingsHint")}</span>
          {providerOptions.map((provider) => (
            <label className="reset-setting-row" key={provider.id}>
              <span className="reset-setting-label">{provider.label}</span>
              <input
                type="checkbox"
                className="provider-toggle-checkbox"
                checked={isProviderVisible(provider.id)}
                onChange={(e) => setProviderVisible(provider.id, e.target.checked)}
              />
            </label>
          ))}
        </div>

        {/* OpenCode Reset Settings */}
        <div className="settings-group">
          <span className="settings-label">{t(settings.locale, "opencodeResetSettings")}</span>
          
          {/* 5h Rolling - Read Only */}
          <div className="reset-setting-row">
            <span className="reset-setting-label">{t(settings.locale, "fiveHourRolling")}</span>
            <span className="reset-setting-value readonly">
              {t(settings.locale, "autoCalculate")}
            </span>
          </div>

          {/* Weekly Reset */}
          <div className="reset-setting-row">
            <span className="reset-setting-label">{t(settings.locale, "weeklyReset")}</span>
            <div className="reset-setting-controls">
              <select
                className="reset-select"
                value={settings.opencodeWeeklyReset?.day ?? 1}
                onChange={(e) => {
                  const day = Number(e.target.value);
                  const current = settings.opencodeWeeklyReset;
                  setWeeklyReset(day, current?.hour ?? 7, current?.minute ?? 0);
                }}
              >
                {DAYS_OF_WEEK.map((d) => (
                  <option key={d.value} value={d.value}>
                    {t(settings.locale, d.label)}
                  </option>
                ))}
              </select>
              <input
                type="time"
                className="reset-time-input"
                value={formatTime(
                  settings.opencodeWeeklyReset?.hour ?? 7,
                  settings.opencodeWeeklyReset?.minute ?? 0
                )}
                onChange={(e) => {
                  const { hour, minute } = parseTime(e.target.value);
                  const current = settings.opencodeWeeklyReset;
                  setWeeklyReset(current?.day ?? 1, hour, minute);
                }}
              />
            </div>
          </div>

          {/* Monthly Reset */}
          <div className="reset-setting-row">
            <span className="reset-setting-label">{t(settings.locale, "monthlyReset")}</span>
            <div className="reset-setting-controls">
              <select
                className="reset-select"
                value={settings.opencodeMonthlyReset?.day ?? 1}
                onChange={(e) => {
                  const day = Number(e.target.value);
                  const current = settings.opencodeMonthlyReset;
                  setMonthlyReset(day, current?.hour ?? 0, current?.minute ?? 0);
                }}
              >
                {Array.from({ length: 31 }, (_, i) => i + 1).map((d) => (
                  <option key={d} value={d}>
                    {d}{t(settings.locale, "daySuffix")}
                  </option>
                ))}
              </select>
              <input
                type="time"
                className="reset-time-input"
                value={formatTime(
                  settings.opencodeMonthlyReset?.hour ?? 0,
                  settings.opencodeMonthlyReset?.minute ?? 0
                )}
                onChange={(e) => {
                  const { hour, minute } = parseTime(e.target.value);
                  const current = settings.opencodeMonthlyReset;
                  setMonthlyReset(current?.day ?? 1, hour, minute);
                }}
              />
            </div>
          </div>
        </div>
      </section>
    </div>
  );
}
