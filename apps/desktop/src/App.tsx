import { useMemo, useState } from "react";
import { desktopRuntimeAvailable } from "./api/desktop";
import { AppShell, type Screen } from "./components/AppShell";
import { translate, type MessageKey } from "./i18n/messages";
import { useImageScan } from "./state/imageScan";
import { usePreferences } from "./state/preferences";
import { AnalysisView } from "./views/AnalysisView";
import { HelpView } from "./views/HelpView";
import { SettingsView } from "./views/SettingsView";

export function App() {
  const [screen, setScreen] = useState<Screen>("analysis");
  const { state: scanState, start: startScan } = useImageScan();
  const {
    preferences,
    persistenceAvailable,
    setLocale,
    setTheme,
    setReducedMotion,
  } = usePreferences();
  const runtimeAvailable = desktopRuntimeAvailable();
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
          scanState={scanState}
          startScan={startScan}
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
