import * as React from "react"

export type Theme = "dark" | "light"

export type ThemeProviderState = {
  theme: Theme
  setTheme: (theme: Theme) => void
}

export const ThemeProviderContext = React.createContext<
  ThemeProviderState | undefined
>(undefined)
