import { useEffect } from "react"
import { GripVertical, PauseIcon, PlayIcon } from "lucide-react"
import { useTranslation } from "react-i18next"

import { Button } from "@/components/ui/button"
import { useMiniTimer } from "@/hooks/use-mini-timer"
import { useMiniWidgetDrag } from "@/hooks/use-mini-widget-drag"
import { cn } from "@/lib/utils"

const INACTIVITY_UI_PHASES = new Set([
  "warning",
  "countdown",
  "paused_inactivity",
  "resume_prompt",
  "manual_work_check",
])

function formatElapsed(seconds: number) {
  const h = Math.floor(seconds / 3600)
  const m = Math.floor((seconds % 3600) / 60)
  const s = seconds % 60
  return [h, m, s].map((part) => String(part).padStart(2, "0")).join(":")
}

export function MiniTimerWidget() {
  const { t } = useTranslation()
  const {
    tracking,
    displaySeconds,
    loading,
    pauseTracking,
    resumeTracking,
    startLastTracking,
    openMainWindow,
  } = useMiniTimer()

  const beginDrag = useMiniWidgetDrag(true)
  const beginDragWithThreshold = useMiniWidgetDrag(false)

  useEffect(() => {
    document.documentElement.classList.add("voowork-mini-surface")
    document.body.classList.add("voowork-mini-surface")
    return () => {
      document.documentElement.classList.remove("voowork-mini-surface")
      document.body.classList.remove("voowork-mini-surface")
    }
  }, [])

  const phase = tracking.inactivity.phase
  const manuallyPaused =
    tracking.active &&
    (phase === "manual_paused" || phase === "manual_work_check")
  const needsMainWindow = tracking.active && INACTIVITY_UI_PHASES.has(phase)
  const isRunning = tracking.active && !manuallyPaused && !needsMainWindow

  const showPlay =
    needsMainWindow || !tracking.active || manuallyPaused

  const toggleLabel = (() => {
    if (needsMainWindow) return t("widget.openApp")
    if (!tracking.active) return t("timer.start")
    if (manuallyPaused) return t("timer.resume")
    return t("timer.pause")
  })()

  const handleToggle = () => {
    if (needsMainWindow) {
      openMainWindow().catch(() => undefined)
      return
    }
    if (!tracking.active) {
      startLastTracking().catch(() => undefined)
      return
    }
    if (manuallyPaused) {
      resumeTracking().catch(() => undefined)
      return
    }
    pauseTracking().catch(() => undefined)
  }

  const openMain = () => {
    openMainWindow().catch(() => undefined)
  }

  return (
    <div className="voowork-mini-widget flex h-full w-full items-center justify-center">
      <div
        className={cn(
          "voowork-mini-shell bg-card/95 border-border/80 flex h-10 items-center gap-2 rounded-2xl border py-0 pl-2.5 pr-0 backdrop-blur-md",
          isRunning && "border-emerald-500/50"
        )}
      >
        <img
          src="/app-icon.png"
          alt=""
          aria-hidden
          draggable={false}
          className="size-[18px] shrink-0 rounded-[4px] object-cover"
        />

        <button
          type="button"
          className="text-foreground voowork-mini-draggable flex shrink-0 cursor-grab items-center border-0 bg-transparent p-0 text-sm font-semibold tabular-nums leading-none active:cursor-grabbing"
          onPointerDown={beginDragWithThreshold}
          onDoubleClick={openMain}
        >
          {formatElapsed(displaySeconds)}
        </button>

        <Button
          type="button"
          variant="ghost"
          size="icon-sm"
          data-no-drag
          className="voowork-mini-no-drag size-8 shrink-0 rounded-xl"
          disabled={loading}
          aria-label={toggleLabel}
          onClick={handleToggle}
        >
          {showPlay ? (
            <PlayIcon className="size-4 fill-current" />
          ) : (
            <PauseIcon className="size-4 fill-current" />
          )}
        </Button>

        <button
          type="button"
          data-tauri-drag-region
          aria-label={t("widget.drag")}
          title={t("widget.drag")}
          className="voowork-mini-drag-handle text-muted-foreground/80 hover:text-muted-foreground voowork-mini-draggable flex h-10 w-7 shrink-0 cursor-grab items-center justify-center rounded-r-2xl border-l border-border/50 active:cursor-grabbing"
          onPointerDown={beginDrag}
        >
          <GripVertical className="size-3.5" strokeWidth={2.25} />
        </button>
      </div>
    </div>
  )
}
