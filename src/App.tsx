import { useTranslation } from "react-i18next"

import { CompactLogin } from "@/components/compact-login"
import { TimerApp } from "@/components/timer-app"
import { useAuth } from "@/hooks/use-auth"
import { Toaster } from "@/components/ui/sonner"

export default function App() {
  const { t } = useTranslation()
  const { auth, loading } = useAuth()

  if (loading) {
    return (
      <div className="voowork-shell text-muted-foreground flex h-full items-center justify-center text-sm">
        {t("common.loading")}
      </div>
    )
  }

  return (
    <>
      <div className="h-full min-h-0">
        {auth.isAuthenticated ? (
          <TimerApp />
        ) : (
          <CompactLogin />
        )}
      </div>
      <Toaster />
    </>
  )
}
