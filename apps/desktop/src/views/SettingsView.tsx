import { Languages, MonitorCog, MoveDiagonal2 } from "lucide-react";
import type { Locale, MessageKey } from "../i18n/messages";
import type { Preferences, Theme } from "../state/preferences";

interface SettingsViewProps {
  preferences: Preferences;
  persistenceAvailable: boolean;
  setLocale: (locale: Locale) => void;
  setTheme: (theme: Theme) => void;
  setReducedMotion: (value: boolean) => void;
  t: (key: MessageKey) => string;
}

export function SettingsView({
  preferences,
  persistenceAvailable,
  setLocale,
  setTheme,
  setReducedMotion,
  t,
}: SettingsViewProps) {
  return (
    <div className="page settings-page">
      <header className="page-header">
        <h1 data-view-heading tabIndex={-1}>
          {t("settings.title")}
        </h1>
        {persistenceAvailable ? (
          <p>{t("settings.subtitle")}</p>
        ) : (
          <p className="persistence-warning" role="status">
            {t("settings.persistenceUnavailable")}
          </p>
        )}
      </header>

      <div className="settings-list">
        <label className="setting-row">
          <span className="setting-icon" aria-hidden="true">
            <Languages size={19} />
          </span>
          <span className="setting-copy">
            <strong>{t("settings.language")}</strong>
          </span>
          <select
            value={preferences.locale}
            onChange={(event) => setLocale(event.target.value as Locale)}
          >
            <option value="pt-BR">{t("settings.language.pt")}</option>
            <option value="en-US">{t("settings.language.en")}</option>
          </select>
        </label>

        <label className="setting-row">
          <span className="setting-icon" aria-hidden="true">
            <MonitorCog size={19} />
          </span>
          <span className="setting-copy">
            <strong>{t("settings.theme")}</strong>
          </span>
          <select
            value={preferences.theme}
            onChange={(event) => setTheme(event.target.value as Theme)}
          >
            <option value="system">{t("settings.theme.system")}</option>
            <option value="dark">{t("settings.theme.dark")}</option>
            <option value="light">{t("settings.theme.light")}</option>
          </select>
        </label>

        <label className="setting-row setting-row-checkbox">
          <span className="setting-icon" aria-hidden="true">
            <MoveDiagonal2 size={19} />
          </span>
          <span className="setting-copy">
            <strong>{t("settings.motion")}</strong>
            <small>{t("settings.motion.help")}</small>
          </span>
          <input
            type="checkbox"
            checked={preferences.reducedMotion}
            onChange={(event) => setReducedMotion(event.target.checked)}
          />
        </label>
      </div>
    </div>
  );
}
