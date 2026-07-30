import { isTauri } from "@tauri-apps/api/core";
import { useMemo, useState } from "react";
import { AppShell, type Screen } from "./components/AppShell";
import { translate, type MessageKey } from "./i18n/messages";
import { usePreferences } from "./state/preferences";
import { useStorageScan } from "./state/storageScan";
import { AnalysisView } from "./views/AnalysisView";
import { HelpView } from "./views/HelpView";
import { SettingsView } from "./views/SettingsView";

export function App() {
  const [screen, setScreen] = useState<Screen>("analysis");
  const {
    preferences,
    persistenceAvailable,
    setLocale,
    setTheme,
    setReducedMotion,
  } = usePreferences();
  const runtimeAvailable = isTauri();
  const storage = useStorageScan(runtimeAvailable);
  const t = useMemo(
    () => (key: MessageKey) => translate(preferences.locale, key),
    [preferences.locale],
  );

  return (
    <AppShell screen={screen} onNavigate={setScreen} t={t}>
      {screen === "analysis" && (
        <AnalysisView
          runtimeAvailable={runtimeAvailable}
          locale={preferences.locale}
          t={t}
          state={storage.state}
          refreshInventory={storage.refreshInventory}
          selectVolume={storage.selectVolume}
          selectFolder={storage.selectFolder}
          clearFolder={storage.clearFolder}
          startScan={storage.startScan}
          loadMore={storage.loadMore}
          resetScan={storage.resetScan}
        />
      )}
      {screen === "settings" && (
        <SettingsView
          preferences={preferences}
          persistenceAvailable={persistenceAvailable}
          setLocale={setLocale}
          setTheme={setTheme}
          setReducedMotion={setReducedMotion}
          t={t}
        />
      )}
      {screen === "help" && <HelpView t={t} />}
    </AppShell>
  );
}
