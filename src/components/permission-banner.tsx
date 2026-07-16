import { useCallback, useEffect, useState } from "react"
import { listen } from "@tauri-apps/api/event"
import { AlertTriangleIcon, SettingsIcon } from "lucide-react"
import { useTranslation } from "react-i18next"

import { isTauriReady, trackedInvoke } from "@/lib/tauri"
import { Button } from "@/components/ui/button"

type State = "loading" | "granted" | "denied"

export function PermissionBanner() {
  const { t } = useTranslation()
  const [state, setState] = useState<State>("loading")

  useEffect(() => {
    if (!isTauriReady()) {
      setState("granted")
      return
    }

    let cancelled = false

    const check = async () => {
      try {
        const granted = await trackedInvoke<boolean>(
          "check_input_monitoring_permission",
        )
        if (!cancelled) {
          setState(granted ? "granted" : "denied")
        }
      } catch {
        if (!cancelled) {
          setState("granted")
        }
      }
    }

    void check()

    const unlistenPromise = listen<never>(
      "permission:input-monitoring-denied",
      () => {
        if (!cancelled) {
          setState("denied")
        }
      },
    )

    return () => {
      cancelled = true
      unlistenPromise.then((dispose) => dispose()).catch(() => undefined)
    }
  }, [])

  const openSettings = useCallback(() => {
    void trackedInvoke("open_system_settings_input_monitoring")
  }, [])

  if (state !== "denied") {
    return null
  }

  return (
    <div className="mx-4 mb-2 mt-1 rounded-xl border border-amber-500/30 bg-amber-500/10 px-4 py-3">
      <div className="flex items-start gap-3">
        <AlertTriangleIcon className="mt-0.5 size-4 shrink-0 text-amber-500" />
        <div className="min-w-0 flex-1 space-y-1.5">
          <p className="text-amber-600 text-xs font-medium leading-tight dark:text-amber-400">
            {t("permission.inputMonitoring.title")}
          </p>
          <p className="text-muted-foreground text-xs leading-relaxed">
            {t("permission.inputMonitoring.description")}
          </p>
          <Button
            type="button"
            variant="outline"
            size="sm"
            className="mt-1 h-8 gap-1.5 rounded-lg border-amber-500/30 text-xs font-medium text-amber-600 hover:bg-amber-500/10 hover:text-amber-500 dark:text-amber-400"
            onClick={openSettings}
          >
            <SettingsIcon className="size-3.5" />
            {t("permission.inputMonitoring.openSettings")}
          </Button>
        </div>
      </div>
    </div>
  )
}
