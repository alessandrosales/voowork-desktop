import i18n from "i18next"
import { initReactI18next } from "react-i18next"

import { isTauriReady, trackedInvoke } from "@/lib/tauri"

import en from "./locales/en.json"
import es from "./locales/es.json"
import ptBR from "./locales/pt-BR.json"

export const SUPPORTED_LOCALES = ["pt-BR", "en", "es"] as const
export type AppLocale = (typeof SUPPORTED_LOCALES)[number]

export const LOCALE_SETTING_KEY = "locale"

const resources = {
  "pt-BR": { translation: ptBR },
  en: { translation: en },
  es: { translation: es },
} as const

function detectBrowserLocale(): AppLocale {
  const candidates =
    navigator.languages.length > 0
      ? navigator.languages
      : [navigator.language]

  for (const candidate of candidates) {
    const resolved = resolveLocale(candidate)
    if (resolved) {
      return resolved
    }
  }

  return "pt-BR"
}

export function resolveLocale(input?: string | null): AppLocale | null {
  if (!input) {
    return null
  }

  const normalized = input.trim().toLowerCase()

  if (normalized === "pt-br" || normalized.startsWith("pt")) {
    return "pt-BR"
  }
  if (normalized.startsWith("es")) {
    return "es"
  }
  if (normalized.startsWith("en")) {
    return "en"
  }

  if (SUPPORTED_LOCALES.includes(input as AppLocale)) {
    return input as AppLocale
  }

  return null
}

export function applyDocumentLocale(locale: AppLocale) {
  document.documentElement.lang = locale
}

async function waitForTauriReady(maxAttempts = 30): Promise<boolean> {
  for (let attempt = 0; attempt < maxAttempts; attempt += 1) {
    if (isTauriReady()) {
      return true
    }
    await new Promise((resolve) => setTimeout(resolve, 100))
  }
  return false
}

void i18n.use(initReactI18next).init({
  resources,
  lng: detectBrowserLocale(),
  fallbackLng: "pt-BR",
  interpolation: {
    escapeValue: false,
  },
})

applyDocumentLocale(i18n.language as AppLocale)

export async function loadPersistedLocale(): Promise<AppLocale | null> {
  const ready = await waitForTauriReady()
  if (!ready) {
    return null
  }

  try {
    const stored = await trackedInvoke<string | null>("get_setting", {
      key: LOCALE_SETTING_KEY,
    })
    return resolveLocale(stored)
  } catch {
    return null
  }
}

export async function changeAppLocale(locale: AppLocale) {
  await i18n.changeLanguage(locale)
  applyDocumentLocale(locale)

  if (isTauriReady()) {
    await trackedInvoke("set_setting", {
      key: LOCALE_SETTING_KEY,
      value: locale,
    })
  }
}

export async function bootstrapLocale() {
  const persisted = await loadPersistedLocale()
  if (persisted && persisted !== i18n.language) {
    await changeAppLocale(persisted)
  }
}

export default i18n
