import { useCallback, useEffect, useState } from "react"
import { listen } from "@tauri-apps/api/event"

import { useDisplayElapsed } from "@/hooks/use-display-elapsed"
import { EMPTY_TRACKING, type TrackingStatus } from "@/hooks/use-tracking-session"
import { trackedInvoke, isTauriReady } from "@/lib/tauri"

const ACTIVE_POLL_MS = 1_000
const IDLE_POLL_MS = 5_000

export function useMiniTimer() {
  const [tracking, setTracking] = useState<TrackingStatus>(EMPTY_TRACKING)
  const [taskElapsedSeconds, setTaskElapsedSeconds] = useState(0)
  const [loading, setLoading] = useState(false)

  const { displayElapsedSeconds, freezeDisplayElapsed, cancelPauseFreeze } =
    useDisplayElapsed(tracking)

  const refresh = useCallback(async () => {
    if (!isTauriReady()) {
      return
    }

    const status = await trackedInvoke<TrackingStatus>("get_tracking_status")
    setTracking(status)

    const taskId = status.taskId
    if (!status.active && taskId) {
      const seconds = await trackedInvoke<number>("get_task_elapsed_seconds", {
        taskId,
      })
      setTaskElapsedSeconds(seconds)
      return
    }

    if (!status.active) {
      const lastTaskId = await trackedInvoke<string | null>("get_setting", {
        key: "last_task_id",
      })
      if (lastTaskId) {
        const seconds = await trackedInvoke<number>("get_task_elapsed_seconds", {
          taskId: lastTaskId,
        })
        setTaskElapsedSeconds(seconds)
      } else {
        setTaskElapsedSeconds(0)
      }
    }
  }, [])

  // Adaptive polling: 1s when tracking active, 5s when idle
  const refreshMs = tracking.active ? ACTIVE_POLL_MS : IDLE_POLL_MS

  useEffect(() => {
    const logRefreshError = (err: unknown) => {
      console.error("mini-timer refresh failed", err)
    }

    const interval = window.setInterval(() => {
      refresh().catch(logRefreshError)
    }, refreshMs)

    // Initial fetch: deferred to avoid set-state-during-render
    queueMicrotask(() => refresh().catch(logRefreshError))

    let cancelled = false
    let unlisten: (() => void) | undefined
    listen("tracking-inactivity-changed", () => {
      if (cancelled) {
        return
      }
      refresh().catch(logRefreshError)
    })
      .then((dispose) => {
        unlisten = dispose
      })
      .catch(() => undefined)

    return () => {
      cancelled = true
      window.clearInterval(interval)
      unlisten?.()
    }
  }, [refresh, refreshMs])

  const pauseTracking = useCallback(async () => {
    setLoading(true)
    freezeDisplayElapsed()
    try {
      await trackedInvoke("pause_tracking")
      await refresh()
    } catch (err) {
      // Pause falhou: desfaz o freeze otimista para o relógio não travar (A11).
      cancelPauseFreeze()
      console.error("mini-timer pause failed", err)
    } finally {
      setLoading(false)
    }
  }, [freezeDisplayElapsed, cancelPauseFreeze, refresh])

  const resumeTracking = useCallback(async () => {
    setLoading(true)
    try {
      await trackedInvoke("resume_tracking")
      await refresh()
    } finally {
      setLoading(false)
    }
  }, [refresh])

  const stopTracking = useCallback(async () => {
    setLoading(true)
    try {
      await trackedInvoke("stop_tracking")
      await refresh()
    } finally {
      setLoading(false)
    }
  }, [refresh])

  const startLastTracking = useCallback(async () => {
    setLoading(true)
    try {
      const [projectId, taskId] = await Promise.all([
        trackedInvoke<string | null>("get_setting", { key: "last_project_id" }),
        trackedInvoke<string | null>("get_setting", { key: "last_task_id" }),
      ])
      if (!projectId || !taskId) {
        await trackedInvoke("open_main_window")
        return
      }
      await trackedInvoke("start_tracking", {
        request: { projectId, taskId },
      })
      await refresh()
    } finally {
      setLoading(false)
    }
  }, [refresh])

  const openMainWindow = useCallback(async () => {
    await trackedInvoke("open_main_window")
  }, [])

  const displaySeconds = tracking.active
    ? displayElapsedSeconds
    : taskElapsedSeconds

  return {
    tracking,
    displaySeconds,
    loading,
    pauseTracking,
    resumeTracking,
    stopTracking,
    startLastTracking,
    openMainWindow,
    refresh,
  }
}
