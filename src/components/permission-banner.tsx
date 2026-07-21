import { useCallback, useEffect, useState } from "react"
import { AlertTriangleIcon, InfoIcon, MonitorIcon, SettingsIcon } from "lucide-react"
import { useTranslation } from "react-i18next"

import { isTauriReady, trackedInvoke } from "@/lib/tauri"
import { Button } from "@/components/ui/button"
import { getCurrentWindow } from "@tauri-apps/api/window"

type PlatformInfo = {
  os: string
  desktopEnv: string | null
  needsInputMonitoringPermission: boolean
  needsScreenRecordingPermission: boolean
  alwaysAllowsWindowTracking: boolean
  note: string | null
}

type PermissionBannerProps = {
  /** When true, also checks Screen Recording / active-window permission */
  checkActiveWindow?: boolean
}

type State = "loading" | "granted" | "denied"

export function PermissionBanner({ checkActiveWindow }: PermissionBannerProps) {
  const { t } = useTranslation()
  const [platform, setPlatform] = useState<PlatformInfo | null>(null)
  const [inputState, setInputState] = useState<State>("loading")
  const [windowState, setWindowState] = useState<State>("loading")

  useEffect(() => {
    if (!isTauriReady()) {
      return
    }

    let cancelled = false

    const checkPermissions = async () => {
      // Only check input-monitoring permission on platforms that need it
      // (macOS).  On Linux + Windows the Rust backend always returns true.
      const needsInput = await trackedInvoke<boolean>(
        "check_input_monitoring_permission",
      ).catch(() => true)
      if (!cancelled) {
        setInputState(needsInput ? "granted" : "denied")
      }

      // Only check active-window permission on platforms that need it
      // (macOS).  On Linux + Windows the Rust backend now always returns true.
      if (checkActiveWindow) {
        const windowOk = await trackedInvoke<boolean>(
          "check_active_window_permission",
        ).catch(() => true)
        if (!cancelled) {
          setWindowState(windowOk ? "granted" : "denied")
        }
      }
    }

    const init = async () => {
      // Load platform info first
      try {
        const info = await trackedInvoke<PlatformInfo>("get_platform_info")
        if (!cancelled) {
          setPlatform(info)
        }
      } catch {
        // if command fails, assume default behaviour (macOS-like)
        if (!cancelled) {
          setPlatform({
            os: "macos",
            desktopEnv: null,
            needsInputMonitoringPermission: true,
            needsScreenRecordingPermission: true,
            alwaysAllowsWindowTracking: false,
            note: null,
          })
        }
      }

      await checkPermissions()
    }

    void init()

    // Re-check permissions on window focus (e.g. user granted permission
    // in System Settings and returned to the app).
    let unlistenFocus: (() => void) | undefined
    const setupFocusListener = async () => {
      try {
        const appWindow = getCurrentWindow()
        unlistenFocus = await appWindow.onFocusChanged(({ payload: focused }) => {
          if (focused && !cancelled) {
            void checkPermissions()
          }
        })
      } catch {
        // Focus events not available outside Tauri (browser dev, etc.)
      }
    }
    void setupFocusListener()

    return () => {
      cancelled = true
      unlistenFocus?.()
    }
  }, [checkActiveWindow])

  const openInputSettings = useCallback(() => {
    void trackedInvoke("open_system_settings_input_monitoring")
  }, [])

  const openScreenRecording = useCallback(() => {
    void trackedInvoke("open_system_settings_screen_recording")
  }, [])

  const isMacOS = platform?.os === "macos"

  // ----- Platform note (Wayland limitation, etc.) -----
  const showPlatformNote =
    platform?.note &&
    !isMacOS &&
    inputState === "granted" &&
    windowState === "granted"

  return (
    <>
      {/* Input Monitoring — only relevant on macOS */}
      {inputState === "denied" && isMacOS ? (
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
                onClick={openInputSettings}
              >
                <SettingsIcon className="size-3.5" />
                {t("permission.inputMonitoring.openSettings")}
              </Button>
            </div>
          </div>
        </div>
      ) : null}

      {/* Screen Recording — only relevant on macOS */}
      {windowState === "denied" && isMacOS ? (
        <div className="mx-4 mb-2 mt-1 rounded-xl border border-amber-500/30 bg-amber-500/10 px-4 py-3">
          <div className="flex items-start gap-3">
            <MonitorIcon className="mt-0.5 size-4 shrink-0 text-amber-500" />
            <div className="min-w-0 flex-1 space-y-1.5">
              <p className="text-amber-600 text-xs font-medium leading-tight dark:text-amber-400">
                {t("permission.activeWindow.title")}
              </p>
              <p className="text-muted-foreground text-xs leading-relaxed">
                {t("permission.activeWindow.description")}
              </p>
              <Button
                type="button"
                variant="outline"
                size="sm"
                className="mt-1 h-8 gap-1.5 rounded-lg border-amber-500/30 text-xs font-medium text-amber-600 hover:bg-amber-500/10 hover:text-amber-500 dark:text-amber-400"
                onClick={openScreenRecording}
              >
                <SettingsIcon className="size-3.5" />
                {t("permission.activeWindow.openSettings")}
              </Button>
            </div>
          </div>
        </div>
      ) : null}

      {/* Platform-aware informational note (e.g. Wayland limitations) */}
      {showPlatformNote ? (
        <div className="mx-4 mb-2 mt-1 rounded-xl border border-sky-500/30 bg-sky-500/10 px-4 py-3">
          <div className="flex items-start gap-3">
            <InfoIcon className="mt-0.5 size-4 shrink-0 text-sky-500" />
            <div className="min-w-0 flex-1 space-y-1.5">
              <p className="text-sky-600 text-xs font-medium leading-tight dark:text-sky-400">
                {t("permission.platformNote.title")}
              </p>
              <p className="text-muted-foreground text-xs leading-relaxed">
                {platform?.note}
              </p>
            </div>
          </div>
        </div>
      ) : null}
    </>
  )
}
