import { useTranslation } from "react-i18next"

import { CompactLogin } from "@/components/compact-login"
import { MiniTimerWidget } from "@/components/mini-timer-widget"
import { TimerApp } from "@/components/timer-app"
import { PermissionBanner } from "@/components/permission-banner"
import { useAuth } from "@/hooks/use-auth"
function isMiniTimerView() {
  return new URLSearchParams(window.location.search).get("view") === "mini"
}

export default function App() {
  const { t } = useTranslation()
  const isMini = isMiniTimerView()
  const { auth, initializing } = useAuth()

  if (isMini) {
    return <MiniTimerWidget />
  }

  if (initializing) {
    return (
      <div className="voowork-shell text-muted-foreground flex h-full items-center justify-center text-sm">
        {t("common.loading")}
      </div>
    )
  }

  return (
    <>
      <div className="flex h-full min-h-0 flex-col">
        <PermissionBanner checkActiveWindow={true} />
        <div className="min-h-0 flex-1">
          {auth.isAuthenticated ? (
            <TimerApp />
          ) : (
            <CompactLogin />
          )}
        </div>
      </div>
    </>
  )
}
