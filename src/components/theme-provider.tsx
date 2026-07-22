import * as React from "react"
import { emit, listen } from "@tauri-apps/api/event"

import { isTauriReady, trackedInvoke, waitForTauriReady } from "@/lib/tauri"

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

export function ThemeProvider({
  children,
  defaultTheme = "dark",
}: ThemeProviderProps) {
  const [theme, setThemeState] = React.useState<Theme>(defaultTheme)

  React.useEffect(() => {
    applyThemeClass(defaultTheme)

    let cancelled = false
    let unlisten: (() => void) | undefined

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

      unlisten = await listen<Theme>("theme-changed", (event) => {
        if (!cancelled && (event.payload === "dark" || event.payload === "light")) {
          setThemeState(event.payload)
          applyThemeClass(event.payload)
        }
      })
    }

    void loadTheme()

    return () => {
      cancelled = true
      unlisten?.()
    }
  }, [defaultTheme])

  const setTheme = React.useCallback((nextTheme: Theme) => {
    setThemeState(nextTheme)
    applyThemeClass(nextTheme)
    if (isTauriReady()) {
      void trackedInvoke("set_setting", { key: "theme", value: nextTheme })
      void emit("theme-changed", nextTheme)
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
