import { useEffect, useState } from "react";
import type { Locale } from "../i18n/messages";

export type Theme = "system" | "dark" | "light";

export interface Preferences {
  locale: Locale;
  theme: Theme;
  reducedMotion: boolean;
}

const STORAGE_KEY = "undelete-master.preferences.v1";
const DEFAULT_PREFERENCES: Preferences = {
  locale: "pt-BR",
  theme: "system",
  reducedMotion: false,
};

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function parsePreferences(value: string | null): Preferences {
  if (value === null) {
    return DEFAULT_PREFERENCES;
  }
  try {
    const parsed: unknown = JSON.parse(value);
    if (!isRecord(parsed)) {
      return DEFAULT_PREFERENCES;
    }
    const locale =
      parsed.locale === "pt-BR" || parsed.locale === "en-US"
        ? parsed.locale
        : DEFAULT_PREFERENCES.locale;
    const theme =
      parsed.theme === "system" ||
      parsed.theme === "dark" ||
      parsed.theme === "light"
        ? parsed.theme
        : DEFAULT_PREFERENCES.theme;
    const reducedMotion =
      typeof parsed.reducedMotion === "boolean"
        ? parsed.reducedMotion
        : DEFAULT_PREFERENCES.reducedMotion;
    return { locale, theme, reducedMotion };
  } catch {
    return DEFAULT_PREFERENCES;
  }
}

interface PreferenceState {
  preferences: Preferences;
  persistenceAvailable: boolean;
}

function loadPreferences(): PreferenceState {
  try {
    return {
      preferences: parsePreferences(window.localStorage.getItem(STORAGE_KEY)),
      persistenceAvailable: true,
    };
  } catch {
    return {
      preferences: DEFAULT_PREFERENCES,
      persistenceAvailable: false,
    };
  }
}

function systemUsesDarkTheme(): boolean {
  return window.matchMedia?.("(prefers-color-scheme: dark)").matches ?? true;
}

function applyPreferences(preferences: Preferences): void {
  const resolvedTheme =
    preferences.theme === "system"
      ? systemUsesDarkTheme()
        ? "dark"
        : "light"
      : preferences.theme;
  document.documentElement.dataset.theme = resolvedTheme;
  document.documentElement.dataset.themePreference = preferences.theme;
  document.documentElement.dataset.reducedMotion = String(
    preferences.reducedMotion,
  );
  document.documentElement.lang = preferences.locale;
}

export function usePreferences() {
  const [state, setState] = useState<PreferenceState>(loadPreferences);
  const { preferences, persistenceAvailable } = state;

  useEffect(() => {
    applyPreferences(preferences);
    try {
      window.localStorage.setItem(STORAGE_KEY, JSON.stringify(preferences));
      setState((current) =>
        current.persistenceAvailable
          ? current
          : { ...current, persistenceAvailable: true },
      );
    } catch {
      setState((current) =>
        current.persistenceAvailable
          ? { ...current, persistenceAvailable: false }
          : current,
      );
    }
  }, [preferences]);

  useEffect(() => {
    if (preferences.theme !== "system" || !window.matchMedia) {
      return;
    }
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const handleChange = () => applyPreferences(preferences);
    media.addEventListener?.("change", handleChange);
    return () => media.removeEventListener?.("change", handleChange);
  }, [preferences]);

  return {
    preferences,
    persistenceAvailable,
    setLocale: (locale: Locale) =>
      setState((current) => ({
        ...current,
        preferences: { ...current.preferences, locale },
      })),
    setTheme: (theme: Theme) =>
      setState((current) => ({
        ...current,
        preferences: { ...current.preferences, theme },
      })),
    setReducedMotion: (reducedMotion: boolean) =>
      setState((current) => ({
        ...current,
        preferences: { ...current.preferences, reducedMotion },
      })),
  };
}

export { STORAGE_KEY };
