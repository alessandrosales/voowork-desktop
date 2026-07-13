import { StrictMode } from "react"
import { createRoot } from "react-dom/client"

import "./index.css"
import App from "./App.tsx"
import { bootstrapLocale } from "@/i18n"
import { AuthProvider } from "@/hooks/use-auth"
import { ThemeProvider } from "@/components/theme-provider.tsx"
import { ExternalLinkGuard } from "./components/external-link-guard.tsx"
import { DebugPanel } from "./components/debug-panel.tsx"
import { TooltipProvider } from "@/components/ui/tooltip"

await bootstrapLocale()

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <ThemeProvider defaultTheme="dark">
      <TooltipProvider>
        <ExternalLinkGuard />
        {import.meta.env.DEV ? <DebugPanel /> : null}
        <AuthProvider>
          <App />
        </AuthProvider>
      </TooltipProvider>
    </ThemeProvider>
  </StrictMode>
)
