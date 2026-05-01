import { X } from "lucide-react";
import { t } from "../i18n";
import type { AppSettings, LocaleId, ThemeId } from "../types";

interface SettingsDialogProps {
  settings: AppSettings;
  onClose: () => void;
  onChange: (settings: AppSettings) => void;
}

export function SettingsDialog({ settings, onClose, onChange }: SettingsDialogProps) {
  const setLocale = (locale: LocaleId) => onChange({ ...settings, locale });
  const setTheme = (theme: ThemeId) => onChange({ ...settings, theme });
  const locales: LocaleId[] = ["zh-TW", "en"];
  const themes: ThemeId[] = ["aurora", "dark", "graphite", "light"];

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
      </section>
    </div>
  );
}
