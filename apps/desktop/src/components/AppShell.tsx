import {
  CircleHelp,
  DatabaseBackup,
  ScanSearch,
  Settings,
} from "lucide-react";
import { useEffect, useRef, type ReactNode } from "react";
import type { MessageKey } from "../i18n/messages";

export type Screen = "analysis" | "settings" | "help";

interface AppShellProps {
  screen: Screen;
  onNavigate: (screen: Screen) => void;
  t: (key: MessageKey) => string;
  children: ReactNode;
}

const navigation: Array<{
  screen: Screen;
  label: MessageKey;
  icon: typeof ScanSearch;
}> = [
  { screen: "analysis", label: "nav.analysis", icon: ScanSearch },
  { screen: "settings", label: "nav.settings", icon: Settings },
  { screen: "help", label: "nav.help", icon: CircleHelp },
];

export function AppShell({
  screen,
  onNavigate,
  t,
  children,
}: AppShellProps) {
  const mainContent = useRef<HTMLElement>(null);
  const previousScreen = useRef(screen);

  useEffect(() => {
    if (previousScreen.current === screen) {
      return;
    }
    previousScreen.current = screen;
    mainContent.current
      ?.querySelector<HTMLElement>("[data-view-heading]")
      ?.focus();
  }, [screen]);

  return (
    <div className="app-shell">
      <nav className="sidebar" aria-label={t("app.name")}>
        <div className="sidebar-brand">
          <span className="brand-icon" aria-hidden="true">
            <DatabaseBackup size={18} />
          </span>
          <span>{t("app.name")}</span>
        </div>

        <div className="nav-list">
          {navigation.map((item) => {
            const Icon = item.icon;
            return (
              <button
                key={item.screen}
                type="button"
                className="nav-item"
                aria-current={screen === item.screen ? "page" : undefined}
                onClick={() => onNavigate(item.screen)}
              >
                <Icon size={18} aria-hidden="true" />
                {t(item.label)}
              </button>
            );
          })}
        </div>
      </nav>
      <main ref={mainContent} className="app-main" id="main-content">
        {children}
      </main>
    </div>
  );
}
