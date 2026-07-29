import {
  CircleOff,
  FileCheck2,
  Info,
  ShieldCheck,
} from "lucide-react";
import type { MessageKey } from "../i18n/messages";

interface HelpViewProps {
  t: (key: MessageKey) => string;
}

const sections: Array<{
  title: MessageKey;
  body: MessageKey;
  icon: typeof FileCheck2;
}> = [
  {
    title: "help.available.title",
    body: "help.available.body",
    icon: FileCheck2,
  },
  {
    title: "help.safety.title",
    body: "help.safety.body",
    icon: ShieldCheck,
  },
  {
    title: "help.unavailable.title",
    body: "help.unavailable.body",
    icon: CircleOff,
  },
  {
    title: "help.interpretation.title",
    body: "help.interpretation.body",
    icon: Info,
  },
];

export function HelpView({ t }: HelpViewProps) {
  return (
    <div className="page help-page">
      <header className="page-header">
        <h1 data-view-heading tabIndex={-1}>
          {t("help.title")}
        </h1>
        <p>{t("help.subtitle")}</p>
      </header>

      <div className="help-list">
        {sections.map((section) => {
          const Icon = section.icon;
          return (
            <section key={section.title} className="help-section">
              <Icon size={20} aria-hidden="true" />
              <div>
                <h2>{t(section.title)}</h2>
                <p>{t(section.body)}</p>
              </div>
            </section>
          );
        })}
      </div>
    </div>
  );
}
