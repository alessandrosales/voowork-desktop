import { useEffect, useState } from "react"
import { AlertTriangleIcon, PauseCircleIcon } from "lucide-react"
import { useTranslation } from "react-i18next"
import type { TFunction } from "i18next"

import type { TrackingInactivityStatus } from "@/hooks/use-tracking-session"
import { Button } from "@/components/ui/button"
import { cn } from "@/lib/utils"

type TrackingInactivityOverlayProps = Readonly<{
  inactivity: TrackingInactivityStatus
  onConfirmStillWorking: () => void | Promise<void>
  onAcknowledgeReturn: () => void | Promise<void>
  onConfirmManualWork: () => void | Promise<void>
  onDismissManualWork: () => void | Promise<void>
  onPauseTracking?: () => void | Promise<void>
  onReturnToWork?: () => void | Promise<void>
  loading?: boolean
}>

function formatAwayMinutes(seconds: number | null, t: TFunction) {
  if (!seconds || seconds < 60) {
    return t("idle.lessThanMinute")
  }
  const minutes = Math.round(seconds / 60)
  return t("idle.minute", { count: minutes })
}

function TrackingInactivityCountdownPanel({
  countdownSecs,
  initial,
}: Readonly<{
  countdownSecs: number
  initial: number
}>) {
  const { t } = useTranslation()
  const [remaining, setRemaining] = useState(initial)

  useEffect(() => {
    const interval = window.setInterval(() => {
      setRemaining((current) => Math.max(0, current - 1))
    }, 1000)

    return () => window.clearInterval(interval)
  }, [])

  const progress =
    countdownSecs > 0 ? ((countdownSecs - remaining) / countdownSecs) * 100 : 0

  return (
    <div className="space-y-2 rounded-xl border border-amber-500/25 bg-amber-500/10 p-4">
      <div className="flex items-center justify-between text-sm">
        <span className="text-muted-foreground">{t("idle.autoPauseIn")}</span>
        <span className="font-mono text-2xl font-bold tabular-nums text-amber-500">
          {remaining}s
        </span>
      </div>
      <div className="bg-muted h-2 overflow-hidden rounded-full">
        <div
          className="h-full rounded-full bg-amber-500 transition-all duration-1000 ease-linear"
          style={{ width: `${Math.min(100, progress)}%` }}
        />
      </div>
    </div>
  )
}

export function TrackingInactivityOverlay({
  inactivity,
  onConfirmStillWorking,
  onAcknowledgeReturn,
  onConfirmManualWork,
  onDismissManualWork,
  onPauseTracking,
  onReturnToWork,
  loading = false,
}: TrackingInactivityOverlayProps) {
  const { t } = useTranslation()
  const showWarning =
    inactivity.phase === "countdown" || inactivity.phase === "warning"
  const showPaused = inactivity.phase === "paused_inactivity"
  const showResume = inactivity.phase === "resume_prompt"
  const showManualWork = inactivity.phase === "manual_work_check"
  const countdownSeed =
    inactivity.countdownRemainingSecs ?? inactivity.countdownSecs

  if (!showWarning && !showPaused && !showResume && !showManualWork) {
    return null
  }

  return (
    <div
      className="fixed inset-0 z-[9999] flex items-center justify-center p-4"
      role="alertdialog"
      aria-modal="true"
      aria-labelledby="idle-alert-title"
    >
      <div className="absolute inset-0 bg-black/70 backdrop-blur-sm" />

      <div
        className={cn(
          "relative w-full max-w-md rounded-2xl border bg-card p-6 shadow-2xl",
          showWarning && "border-amber-500/40",
          showPaused && "border-slate-500/40",
          showResume && "border-primary/30",
          showManualWork && "border-primary/40"
        )}
      >
        {showWarning ? (
          <div className="space-y-5">
            <div className="flex items-start gap-3">
              <div className="flex size-10 shrink-0 items-center justify-center rounded-full bg-amber-500/15">
                <AlertTriangleIcon className="size-5 text-amber-500" />
              </div>
              <div>
                <p id="idle-alert-title" className="text-base font-semibold">
                  {t("idle.noActivityTitle")}
                </p>
                <p className="text-muted-foreground mt-1 text-sm leading-relaxed">
                  {t("idle.noActivityDescription")}
                </p>
              </div>
            </div>

            {inactivity.phase === "countdown" ? (
              <TrackingInactivityCountdownPanel
                key={countdownSeed}
                countdownSecs={inactivity.countdownSecs}
                initial={countdownSeed}
              />
            ) : null}

            <Button
              size="lg"
              className="h-12 w-full rounded-xl text-base font-semibold"
              onClick={() => {
                Promise.resolve(onConfirmStillWorking()).catch(() => undefined)
              }}
              disabled={loading}
            >
              {t("idle.stillWorking")}
            </Button>

            {onPauseTracking ? (
              <Button
                size="lg"
                variant="outline"
                className="h-12 w-full rounded-xl text-base"
                onClick={() => {
                  Promise.resolve(onPauseTracking()).catch(() => undefined)
                }}
                disabled={loading}
              >
                {t("idle.pause")}
              </Button>
            ) : null}
          </div>
        ) : null}

        {showPaused ? (
          <div className="space-y-5 text-center">
            <div className="mx-auto flex size-12 items-center justify-center rounded-full bg-slate-500/15">
              <PauseCircleIcon className="size-6 text-slate-400" />
            </div>
            <div>
              <p id="idle-alert-title" className="text-base font-semibold">
                {t("idle.pausedTitle")}
              </p>
              <p className="text-muted-foreground mt-2 text-sm leading-relaxed">
                {t("idle.pausedDescription")}
              </p>
            </div>

            {onReturnToWork ? (
              <Button
                size="lg"
                className="h-12 w-full rounded-xl text-base font-semibold"
                onClick={() => {
                  Promise.resolve(onReturnToWork()).catch(() => undefined)
                }}
                disabled={loading}
              >
                {t("idle.returnToWork")}
              </Button>
            ) : null}
          </div>
        ) : null}

        {showResume ? (
          <div className="space-y-5">
            <div>
              <p id="idle-alert-title" className="text-base font-semibold">
                {t("idle.returnedTitle")}
              </p>
              <p className="text-muted-foreground mt-2 text-sm leading-relaxed">
                {t("idle.returnedAway", {
                  duration: formatAwayMinutes(inactivity.awaySeconds, t),
                })}{" "}
                {t("idle.returnedPeriod")}{" "}
                <span className="text-foreground font-medium">
                  {t("idle.notCounted")}
                </span>{" "}
                {t("idle.returnedSuffix")}
              </p>
            </div>

            <Button
              size="lg"
              className="h-12 w-full rounded-xl text-base font-semibold"
              onClick={() => {
                Promise.resolve(onAcknowledgeReturn()).catch(() => undefined)
              }}
              disabled={loading}
            >
              {t("idle.continueWorking")}
            </Button>
          </div>
        ) : null}

        {showManualWork ? (
          <div className="space-y-5">
            <div className="flex items-start gap-3">
              <div className="flex size-10 shrink-0 items-center justify-center rounded-full bg-primary/15">
                <AlertTriangleIcon className="text-primary size-5" />
              </div>
              <div>
                <p id="idle-alert-title" className="text-base font-semibold">
                  {t("idle.manualWorkTitle")}
                </p>
                <p className="text-muted-foreground mt-1 text-sm leading-relaxed">
                  {t("idle.manualWorkDescription")}
                </p>
              </div>
            </div>

            <Button
              size="lg"
              className="h-12 w-full rounded-xl text-base font-semibold"
              onClick={() => {
                Promise.resolve(onConfirmManualWork()).catch(() => undefined)
              }}
              disabled={loading}
            >
              {t("idle.manualWorkConfirm")}
            </Button>
            <Button
              size="lg"
              variant="outline"
              className="h-12 w-full rounded-xl text-base"
              onClick={() => {
                Promise.resolve(onDismissManualWork()).catch(() => undefined)
              }}
              disabled={loading}
            >
              {t("idle.manualWorkDismiss")}
            </Button>
          </div>
        ) : null}
      </div>
    </div>
  )
}
