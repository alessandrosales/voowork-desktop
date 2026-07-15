import { useCallback, useEffect, useState } from "react"
import { listen } from "@tauri-apps/api/event"

import { useDisplayElapsed } from "@/hooks/use-display-elapsed"
import type { TrackingStatus } from "@/hooks/use-tracking-session"
import { trackedInvoke, isTauriReady } from "@/lib/tauri"

const EMPTY_TRACKING: TrackingStatus = {
  active: false,
  trackingId: null,
  projectId: null,
  taskId: null,
  startedAt: null,
  elapsedSeconds: 0,
  inactivitySeconds: 0,
  taskAccumulatedSeconds: 0,
  activityBufferSeconds: 0,
  activityBufferAlert: false,
  mouseEvents: 0,
  keyboardEvents: 0,
  clockSkewDetected: false,
  activityConfidence: 1,
  activityScore: 0,
  trackerMode: null,
  currentApp: null,
  currentWindowTitle: null,
  screenshotCount: 0,
  lastScreenshotAt: null,
  inactivity: {
    phase: "active",
    thresholdSecs: 120,
    countdownSecs: 60,
    countdownRemainingSecs: null,
    countdownEndsAt: null,
    inactivityStartedAt: null,
    pausedAt: null,
    awaySeconds: null,
    pendingPeriodId: null,
    meetingExempt: false,
    activeSeconds: 0,
    inactivityDiscardedSeconds: 0,
    inactivityReclassifiedSeconds: 0,
  },
}

export function useMiniTimer() {
  const [tracking, setTracking] = useState<TrackingStatus>(EMPTY_TRACKING)
  const [taskElapsedSeconds, setTaskElapsedSeconds] = useState(0)
  const [loading, setLoading] = useState(false)

  const { displayElapsedSeconds, freezeDisplayElapsed } =
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
        task_id: taskId,
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
          task_id: lastTaskId,
        })
        setTaskElapsedSeconds(seconds)
      } else {
        setTaskElapsedSeconds(0)
      }
    }
  }, [])

  useEffect(() => {
    refresh().catch(() => undefined)
    const interval = window.setInterval(() => {
      refresh().catch(() => undefined)
    }, 1000)

    let unlisten: (() => void) | undefined
    listen("tracking-inactivity-changed", () => {
      refresh().catch(() => undefined)
    })
      .then((dispose) => {
        unlisten = dispose
      })
      .catch(() => undefined)

    return () => {
      window.clearInterval(interval)
      unlisten?.()
    }
  }, [refresh])

  const pauseTracking = useCallback(async () => {
    setLoading(true)
    try {
      freezeDisplayElapsed()
      await trackedInvoke("pause_tracking")
      await refresh()
    } finally {
      setLoading(false)
    }
  }, [freezeDisplayElapsed, refresh])

  const resumeTracking = useCallback(async () => {
    setLoading(true)
    try {
      await trackedInvoke("resume_tracking")
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
    startLastTracking,
    openMainWindow,
    refresh,
  }
}
