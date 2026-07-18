import { StrictMode } from "react"
import { createRoot } from "react-dom/client"

import "./index.css"
import App from "./App.tsx"
import { bootstrapLocale } from "@/i18n"
import { AuthProvider } from "@/hooks/use-auth"
import { ThemeProvider } from "@/components/theme-provider.tsx"
import { ExternalLinkGuard } from "./components/external-link-guard.tsx"
import { TooltipProvider } from "@/components/ui/tooltip"

const isMiniTimerView =
  new URLSearchParams(window.location.search).get("view") === "mini"

if (isMiniTimerView) {
  document.documentElement.classList.add("voowork-mini-surface")
  document.body.classList.add("voowork-mini-surface")
}

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <ThemeProvider defaultTheme="dark">
      <TooltipProvider>
        <ExternalLinkGuard />
        <AuthProvider>
          <App />
        </AuthProvider>
      </TooltipProvider>
    </ThemeProvider>
  </StrictMode>
)

void bootstrapLocale()
