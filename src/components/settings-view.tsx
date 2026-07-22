import { ArrowLeftIcon } from "lucide-react"
import { useCallback, useEffect, useState } from "react"
import { useTranslation } from "react-i18next"

import { AppMeta } from "@/components/app-meta"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { trackedInvoke } from "@/lib/tauri"
import { cn } from "@/lib/utils"

const VALID_PROFILES = [
  "standard",
  "data_entry",
  "knowledge",
  "meeting_heavy",
] as const

type Profile = (typeof VALID_PROFILES)[number]

function ToggleSwitch({
  id,
  checked,
  onChange,
}: Readonly<{
  id: string
  checked: boolean
  onChange: (checked: boolean) => void
}>) {
  return (
    <button
      id={id}
      type="button"
      role="switch"
      aria-checked={checked}
      onClick={() => onChange(!checked)}
      className={cn(
        "relative inline-flex h-6 w-11 shrink-0 cursor-pointer items-center rounded-full border-2 border-transparent transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background",
        checked ? "bg-primary" : "bg-input"
      )}
    >
      <span
        className={cn(
          "pointer-events-none block size-5 rounded-full bg-white shadow-lg ring-0 transition-transform",
          checked ? "translate-x-5" : "translate-x-0"
        )}
      />
    </button>
  )
}

type SettingsViewProps = Readonly<{
  onBack: () => void
}>

export function SettingsView({ onBack }: SettingsViewProps) {
  const { t } = useTranslation()
  const [loading, setLoading] = useState(true)

  const [blurEnabled, setBlurEnabled] = useState(false)
  const [inactivityProfile, setInactivityProfile] = useState<Profile | "custom">("standard")
  const [inactivityThreshold, setInactivityThreshold] = useState("2")
  const [miniWidgetEnabled, setMiniWidgetEnabled] = useState(true)
  const [appVersion, setAppVersion] = useState("")
  const [countdownSecs, setCountdownSecs] = useState(60)

  useEffect(() => {
    Promise.all([
      trackedInvoke<string | null>("get_setting", { key: "screenshot_blur_enabled" }).catch(() => null),
      trackedInvoke<string | null>("get_setting", { key: "tracking_inactivity_profile" }).catch(() => null),
      trackedInvoke<string | null>("get_setting", { key: "tracking_inactivity_threshold_minutes" }).catch(() => null),
      trackedInvoke<string | null>("get_setting", { key: "mini_widget_enabled" }).catch(() => null),
      trackedInvoke<string>("get_app_version").catch(() => ""),
      trackedInvoke<{
        inactivity: { threshold_minutes: number; profile: string; countdown_secs: number }
      }>("get_tracking_config").catch(() => null),
    ]).then(
      ([
        blur, profile, threshold, widget, version, config,
      ]: [
        string | null,
        string | null,
        string | null,
        string | null,
        string,
        { inactivity: { threshold_minutes: number; profile: string; countdown_secs: number } } | null,
      ]) => {
        setBlurEnabled(blur === "true" || blur === "1")
        if (profile) {
          setInactivityProfile(
            VALID_PROFILES.includes(profile as Profile)
              ? (profile as Profile)
              : "standard"
          )
        }
        setInactivityThreshold(threshold ?? "2")
        setMiniWidgetEnabled(widget !== "false" && widget !== "0")
        setAppVersion(version)
        if (config) {
          setCountdownSecs(config.inactivity.countdown_secs)
          if (!profile) {
            const cfgProfile = config.inactivity.profile
            if (VALID_PROFILES.includes(cfgProfile as Profile)) {
              setInactivityProfile(cfgProfile as Profile)
            }
          }
          if (!threshold) {
            setInactivityThreshold(String(config.inactivity.threshold_minutes))
          }
        }
        setLoading(false)
      },
    )
  }, [])

  const saveSetting = useCallback((key: string, value: string) => {
    trackedInvoke("set_setting", { key, value }).catch((err) => {
      console.error(`Failed to save setting ${key}:`, err)
    })
  }, [])

  const handleToggleBlur = useCallback(
    (checked: boolean) => {
      setBlurEnabled(checked)
      saveSetting("screenshot_blur_enabled", checked ? "true" : "false")
    },
    [saveSetting],
  )

  const handleProfileChange = useCallback(
    (value: string | null) => {
      const v = value ?? "standard"
      if (v === "custom") {
        setInactivityProfile("custom")
        return
      }
      setInactivityProfile(v as Profile)
      saveSetting("tracking_inactivity_profile", v)
    },
    [saveSetting],
  )

  const handleThresholdChange = useCallback(
    (value: string) => {
      const num = Math.max(1, Math.min(120, Number.parseInt(value, 10) || 2))
      setInactivityThreshold(String(num))
      saveSetting("tracking_inactivity_threshold_minutes", String(num))
    },
    [saveSetting],
  )

  const handleToggleWidget = useCallback(
    (checked: boolean) => {
      setMiniWidgetEnabled(checked)
      saveSetting("mini_widget_enabled", checked ? "true" : "false")
    },
    [saveSetting],
  )

  const isProfileCustom = inactivityProfile === "custom"
  const delayWarning = t("settings.takesEffectNextTracking")

  return (
    <div className="voowork-shell flex h-full min-h-0 flex-col">
      <header className="flex items-center px-3 py-2">
        <button
          type="button"
          onClick={onBack}
          aria-label={t("workspace.back")}
          className="text-muted-foreground hover:text-foreground flex size-8 items-center justify-center rounded-lg transition-colors"
        >
          <ArrowLeftIcon className="size-5" />
        </button>
      </header>

      <div className="flex min-h-0 flex-1 flex-col overflow-y-auto">
        <div className="flex flex-col px-6 py-4">
          {loading ? (
            <div className="flex items-center justify-center py-12">
              <p className="text-muted-foreground text-sm">{t("common.loading")}</p>
            </div>
          ) : (
            <div className="flex flex-col gap-12">
              <h1 className="text-lg font-semibold">{t("settings.title")}</h1>

              <section>
                <h2 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground mb-4">
                  {t("settings.sectionGeneral")}
                </h2>
                <div className="space-y-2">
                  <Label htmlFor="app-version" className="text-sm font-medium">
                    {t("settings.appVersion")}
                  </Label>
                  <p
                    id="app-version"
                    className="text-muted-foreground text-sm tabular-nums"
                  >
                    {appVersion}
                  </p>
                </div>
              </section>

              <section>
                <h2 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground mb-4">
                  {t("settings.sectionScreenshots")}
                </h2>
                <div className="space-y-4">
                  <Label htmlFor="screenshot-blur" className="text-sm font-medium">
                    {t("settings.screenshotBlur")}
                  </Label>
                  <p className="text-muted-foreground text-xs leading-relaxed">
                    {t("settings.screenshotBlurDesc")}
                  </p>
                  <ToggleSwitch
                    id="screenshot-blur"
                    checked={blurEnabled}
                    onChange={handleToggleBlur}
                  />
                </div>
              </section>

              <section>
                <h2 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground mb-4">
                  {t("settings.sectionInactivity")}
                </h2>
                <div className="space-y-8">
                  <div className="space-y-4">
                    <div>
                      <Label htmlFor="inactivity-profile" className="text-sm font-medium">
                        {t("settings.inactivityProfile")}
                      </Label>
                      <p className="text-muted-foreground text-xs leading-relaxed mt-1.5">
                        {t("settings.inactivityProfileDesc")}
                      </p>
                    </div>
                    <Select
                      value={isProfileCustom ? "custom" : inactivityProfile}
                      onValueChange={handleProfileChange}
                    >
                      <SelectTrigger id="inactivity-profile" className="w-full">
                        <SelectValue />
                      </SelectTrigger>
                      <SelectContent>
                        <SelectItem value="standard">{t("settings.profileStandard")}</SelectItem>
                        <SelectItem value="data_entry">{t("settings.profileDataEntry")}</SelectItem>
                        <SelectItem value="knowledge">{t("settings.profileKnowledge")}</SelectItem>
                        <SelectItem value="meeting_heavy">{t("settings.profileMeetingHeavy")}</SelectItem>
                        <SelectItem value="custom">{t("settings.profileCustomPlaceholder")}</SelectItem>
                      </SelectContent>
                    </Select>
                    <p className="text-muted-foreground/60 text-[10px] leading-relaxed">
                      {delayWarning}
                    </p>
                  </div>

                  <div className="space-y-4">
                    <div>
                      <Label htmlFor="inactivity-threshold" className="text-sm font-medium">
                        {t("settings.inactivityThreshold")}
                      </Label>
                      <p className="text-muted-foreground text-xs leading-relaxed mt-1.5">
                        {isProfileCustom
                          ? t("settings.inactivityThresholdDesc")
                          : t("settings.profileNote")}
                      </p>
                    </div>
                    <div className="flex items-center gap-2">
                      <Input
                        id="inactivity-threshold"
                        type="number"
                        min={1}
                        max={120}
                        value={inactivityThreshold}
                        onChange={(e) => handleThresholdChange(e.target.value)}
                        disabled={!isProfileCustom}
                        className={cn(
                          "w-24 text-center tabular-nums",
                          !isProfileCustom && "opacity-50"
                        )}
                      />
                      <span className="text-muted-foreground text-xs">{t("settings.minutesLabel")}</span>
                    </div>
                    <p className="text-muted-foreground/60 text-[10px] leading-relaxed">
                      {delayWarning}
                    </p>
                  </div>

                  <div className="space-y-4">
                    <div>
                      <Label htmlFor="inactivity-countdown" className="text-sm font-medium">
                        {t("settings.inactivityCountdown")}
                      </Label>
                      <p className="text-muted-foreground text-xs leading-relaxed mt-1.5">
                        {t("settings.inactivityCountdownDesc")}
                      </p>
                    </div>
                    <p
                      id="inactivity-countdown"
                      className="text-muted-foreground text-sm tabular-nums"
                    >
                      {t("common.seconds", { count: countdownSecs })}
                    </p>
                  </div>
                </div>
              </section>

              <section>
                <h2 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground mb-4">
                  {t("settings.sectionWidget")}
                </h2>
                <div className="space-y-4">
                  <Label htmlFor="mini-widget" className="text-sm font-medium">
                    {t("settings.miniWidgetEnabled")}
                  </Label>
                  <p className="text-muted-foreground text-xs leading-relaxed">
                    {t("settings.miniWidgetEnabledDesc")}
                  </p>
                  <ToggleSwitch
                    id="mini-widget"
                    checked={miniWidgetEnabled}
                    onChange={handleToggleWidget}
                  />
                </div>
              </section>
            </div>
          )}
        </div>
      </div>

      <footer className="shrink-0 pb-6 pt-2">
        <AppMeta />
      </footer>
    </div>
  )
}
