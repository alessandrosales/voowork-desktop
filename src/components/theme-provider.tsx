/* eslint-disable react-refresh/only-export-components */
import * as React from "react"

import { isTauriReady, trackedInvoke } from "@/lib/tauri"

type Theme = "dark" | "light"

type ThemeProviderProps = Readonly<{
  children: React.ReactNode
  defaultTheme?: Theme
}>

type ThemeProviderState = {
  theme: Theme
  setTheme: (theme: Theme) => void
}

const ThemeProviderContext = React.createContext<
  ThemeProviderState | undefined
>(undefined)

function applyThemeClass(theme: Theme) {
  const root = document.documentElement
  root.classList.remove("light", "dark")
  root.classList.add(theme)
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

export function ThemeProvider({
  children,
  defaultTheme = "dark",
}: ThemeProviderProps) {
  const [theme, setThemeState] = React.useState<Theme>(defaultTheme)

  React.useEffect(() => {
    applyThemeClass(defaultTheme)

    let cancelled = false

    async function loadTheme() {
      const ready = await waitForTauriReady()
      if (!ready || cancelled) {
        return
      }

      try {
        const stored = await trackedInvoke<string | null>("get_setting", {
          key: "theme",
        })
        if (cancelled) {
          return
        }
        if (stored === "dark" || stored === "light") {
          setThemeState(stored)
          applyThemeClass(stored)
        }
      } catch {
        if (!cancelled) {
          applyThemeClass(defaultTheme)
        }
      }
    }

    void loadTheme()

    return () => {
      cancelled = true
    }
  }, [defaultTheme])

  const setTheme = React.useCallback((nextTheme: Theme) => {
    setThemeState(nextTheme)
    applyThemeClass(nextTheme)
    if (isTauriReady()) {
      void trackedInvoke("set_setting", { key: "theme", value: nextTheme })
    }
  }, [])

  React.useEffect(() => {
    applyThemeClass(theme)
  }, [theme])

  const value = React.useMemo(
    () => ({
      theme,
      setTheme,
    }),
    [theme, setTheme]
  )

  return (
    <ThemeProviderContext.Provider value={value}>
      {children}
    </ThemeProviderContext.Provider>
  )
}

export const useTheme = () => {
  const context = React.useContext(ThemeProviderContext)

  if (context === undefined) {
    throw new Error("useTheme must be used within a ThemeProvider")
  }

  return context
}
