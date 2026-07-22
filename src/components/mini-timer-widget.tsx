import { useCallback, useLayoutEffect, useRef } from "react"
import { GripVertical, PauseIcon, PlayIcon } from "lucide-react"
import { useTranslation } from "react-i18next"
import { isTauri } from "@tauri-apps/api/core"
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window"

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

  const frameRef = useRef<HTMLDivElement>(null)

  const phase = tracking.inactivity.phase
  const manuallyPaused =
    tracking.active &&
    (phase === "manual_paused" || phase === "manual_work_check")
  const needsMainWindow = tracking.active && INACTIVITY_UI_PHASES.has(phase)
  const isRunning = tracking.active && !manuallyPaused && !needsMainWindow

  const showPlay =
    needsMainWindow || !tracking.active || manuallyPaused

  const syncWindowToPill = useCallback(() => {
    if (!isTauri()) {
      return
    }

    const frame = frameRef.current
    if (!frame) {
      return
    }

    const rect = frame.getBoundingClientRect()
    const width = Math.ceil(rect.width)
    const height = Math.ceil(rect.height)
    if (width <= 0 || height <= 0) {
      return
    }

    const size = new LogicalSize(width, height)
    const appWindow = getCurrentWindow()

    void (async () => {
      try {
        await appWindow.setMinSize(null)
        await appWindow.setMaxSize(null)
        await appWindow.setSize(size)
        await appWindow.setMinSize(size)
        await appWindow.setMaxSize(size)
      } catch (error) {
        console.error("[mini-timer] failed to resize window to pill", error)
      }
    })()
  }, [])

  useLayoutEffect(() => {
    if (!isTauri()) {
      return
    }

    const frame = frameRef.current
    if (!frame) {
      return
    }

    let rafId = 0
    const scheduleSync = () => {
      cancelAnimationFrame(rafId)
      rafId = requestAnimationFrame(() => {
        syncWindowToPill()
      })
    }

    scheduleSync()

    const observer = new ResizeObserver(scheduleSync)
    observer.observe(frame)

    return () => {
      cancelAnimationFrame(rafId)
      observer.disconnect()
    }
  }, [syncWindowToPill])

  useLayoutEffect(() => {
    syncWindowToPill()
  }, [isRunning, showPlay, loading, displaySeconds, syncWindowToPill])

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
    <div
      ref={frameRef}
      className="voowork-mini-widget inline-flex shrink-0 overflow-hidden p-0.5"
    >
      <div
        className={cn(
          "voowork-mini-shell bg-card/95 border-border/80 flex h-5 w-max items-center gap-2 rounded-full border py-0 pl-2 pr-0",
          isRunning && "border-emerald-500/50"
        )}
      >
        <img
          src="/app-icon.png"
          alt=""
          aria-hidden
          draggable={false}
          className="size-3 shrink-0 rounded-[3px] object-cover"
        />

        <button
          type="button"
          className="text-foreground voowork-mini-draggable flex shrink-0 cursor-grab items-center border-0 bg-transparent p-0 text-xs font-semibold tabular-nums leading-none active:cursor-grabbing"
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
          className="voowork-mini-no-drag size-4 shrink-0 rounded-full cursor-pointer"
          disabled={loading}
          aria-label={toggleLabel}
          onClick={handleToggle}
        >
          {showPlay ? (
            <PlayIcon className="size-3 fill-current" />
          ) : (
            <PauseIcon className="size-3 fill-current" />
          )}
        </Button>

        <button
          type="button"
          aria-label={t("widget.drag")}
          title={t("widget.drag")}
          className="voowork-mini-drag-handle text-muted-foreground hover:bg-muted/60 hover:text-foreground voowork-mini-draggable flex h-5 w-6 shrink-0 cursor-grab items-center justify-center rounded-r-full border-l border-border/60 active:cursor-grabbing"
          onPointerDown={beginDrag}
        >
          <GripVertical className="size-3.5" strokeWidth={2.5} />
        </button>
      </div>
    </div>
  )
}
