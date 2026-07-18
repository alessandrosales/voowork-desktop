import { useCallback, useEffect, useMemo, useRef, useState } from "react"
import { listen } from "@tauri-apps/api/event"

import { useDisplayElapsed } from "@/hooks/use-display-elapsed"
import { trackedInvoke, isTauriReady } from "@/lib/tauri"

export type TrackingInactivityStatus = {
  phase: string
  thresholdSecs: number
  countdownSecs: number
  countdownRemainingSecs: number | null
  countdownEndsAt: string | null
  inactivityStartedAt: string | null
  pausedAt: string | null
  awaySeconds: number | null
  pendingPeriodId: string | null
  meetingExempt: boolean
  activeSeconds: number
  inactivityDiscardedSeconds: number
  inactivityReclassifiedSeconds: number
}

export type TrackingStatus = {
  active: boolean
  trackingId: string | null
  projectId: string | null
  taskId: string | null
  startedAt: string | null
  elapsedSeconds: number
  inactivitySeconds: number
  taskAccumulatedSeconds: number
  activityBufferSeconds: number
  activityBufferAlert: boolean
  mouseEvents: number
  keyboardEvents: number
  clockSkewDetected: boolean
  activityConfidence: number
  activityScore: number
  trackerMode: string | null
  currentApp: string | null
  currentWindowTitle: string | null
  screenshotCount: number
  lastScreenshotAt: string | null
  inactivity: TrackingInactivityStatus
}

export type ProjectOption = {
  id: string
  name: string
  tasks: TaskOption[]
}

export type TaskOption = {
  id: string
  name: string
}

const EMPTY_INACTIVITY: TrackingInactivityStatus = {
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
}

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
  inactivity: EMPTY_INACTIVITY,
}

/** Timer ativo: poll rápido para fases de inatividade e pausa manual. */
const ACTIVE_TRACKING_REFRESH_MS = 1_000
const IDLE_REFRESH_MS = 5_000

function backgroundRefreshIntervalMs(tracking: TrackingStatus) {
  if (!tracking.active) {
    return IDLE_REFRESH_MS
  }

  return ACTIVE_TRACKING_REFRESH_MS
}

export function useTrackingSession() {
  const [tracking, setTracking] = useState<TrackingStatus>(EMPTY_TRACKING)
  const [projects, setProjects] = useState<ProjectOption[]>([])
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const [taskElapsedSeconds, setTaskElapsedSeconds] = useState(0)
  const taskElapsedReqId = useRef(0)

  const { displayElapsedSeconds, freezeDisplayElapsed } =
    useDisplayElapsed(tracking)

  const refreshTaskElapsed = useCallback(async (taskId: string | null) => {
    const reqId = ++taskElapsedReqId.current
    if (!isTauriReady() || !taskId || taskId === "__none__") {
      if (reqId === taskElapsedReqId.current) {
        setTaskElapsedSeconds(0)
      }
      return
    }
    try {
      const seconds = await trackedInvoke<number>("get_task_elapsed_seconds", {
        taskId: taskId,
      })
      if (reqId === taskElapsedReqId.current) {
        setTaskElapsedSeconds(seconds)
      }
    } catch {
      if (reqId === taskElapsedReqId.current) {
        setTaskElapsedSeconds(0)
      }
    }
  }, [])

  const refreshTrackingStatus = useCallback(async () => {
    if (!isTauriReady()) {
      return
    }

    try {
      const trackingStatus = await trackedInvoke<TrackingStatus>("get_tracking_status")
      setTracking(trackingStatus)
      setError(null)
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    }
  }, [])

  const loadProjects = useCallback(async () => {
    if (!isTauriReady()) {
      return
    }

    // Tenta sincronizar primeiro, mas não falha se a API estiver offline
    try {
      await trackedInvoke("sync_projects")
    } catch {
      // sync failure é não-fatal — usa cache local
    }

    try {
      const projectList = await trackedInvoke<ProjectOption[]>("list_projects")
      setProjects(projectList)
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    }
  }, [])

  const refresh = useCallback(async () => {
    if (!isTauriReady()) {
      return
    }

    try {
      const trackingStatus = await trackedInvoke<TrackingStatus>("get_tracking_status")
      setTracking(trackingStatus)
      setError(null)
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    }
  }, [])

  const pollBackground = useCallback(async () => {
    if (!isTauriReady()) {
      return
    }

    try {
      const trackingStatus = await trackedInvoke<TrackingStatus>(
        "get_tracking_status"
      )
      setTracking(trackingStatus)
      setError(null)
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    }
  }, [])

  const refreshMs = backgroundRefreshIntervalMs(tracking)

  useEffect(() => {
    const timer = window.setTimeout(() => {
      refresh().catch(() => undefined)
    }, 0)
    const interval = window.setInterval(() => {
      pollBackground().catch(() => undefined)
    }, refreshMs)
    return () => {
      window.clearTimeout(timer)
      window.clearInterval(interval)
    }
  }, [refresh, pollBackground, refreshMs])

  useEffect(() => {
    if (!isTauriReady()) {
      return
    }

    let unlisten: (() => void) | undefined
    listen("tracking-inactivity-changed", () => {
      refreshTrackingStatus().catch(() => undefined)
    })
      .then((dispose) => {
        unlisten = dispose
      })
      .catch(() => undefined)

    return () => {
      unlisten?.()
    }
  }, [refreshTrackingStatus])

  const startTracking = useCallback(
    async (projectId: string, taskId: string) => {
      setLoading(true)
      setError(null)
      try {
        await trackedInvoke("start_tracking", {
          request: { projectId, taskId },
        })
        await refreshTrackingStatus()
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err))
        throw err
      } finally {
        setLoading(false)
      }
    },
    [refreshTrackingStatus]
  )

  const restartTracking = useCallback(
    async (projectId: string, taskId: string) => {
      setLoading(true)
      setError(null)
      try {
        await trackedInvoke("restart_tracking", {
          request: { projectId, taskId },
        })
        await refreshTrackingStatus()
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err))
        throw err
      } finally {
        setLoading(false)
      }
    },
    [refreshTrackingStatus]
  )

  const pauseTracking = useCallback(async () => {
    setError(null)
    freezeDisplayElapsed()
    try {
      await trackedInvoke("pause_tracking")
      await refreshTrackingStatus()
      await refreshTaskElapsed(
        tracking.taskId && tracking.taskId !== "__none__" ? tracking.taskId : null
      )
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
      throw err
    }
  }, [freezeDisplayElapsed, refreshTrackingStatus, refreshTaskElapsed, tracking.taskId])

  const resumeTracking = useCallback(async () => {
    setLoading(true)
    setError(null)
    try {
      await trackedInvoke("resume_tracking")
      await refreshTrackingStatus()
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
      throw err
    } finally {
      setLoading(false)
    }
  }, [refreshTrackingStatus])

  const stopTracking = useCallback(async () => {
    setLoading(true)
    setError(null)
    try {
      await trackedInvoke("stop_tracking")
      await refreshTrackingStatus()
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
      throw err
    } finally {
      setLoading(false)
    }
  }, [refreshTrackingStatus])

  const confirmStillWorking = useCallback(async () => {
    setLoading(true)
    setError(null)
    try {
      await trackedInvoke("confirm_still_working")
      await refreshTrackingStatus()
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
      throw err
    } finally {
      setLoading(false)
    }
  }, [refreshTrackingStatus])

  const classifyTrackingInactivityPeriod = useCallback(
    async (periodId: string, category: string) => {
      setLoading(true)
      setError(null)
      try {
        await trackedInvoke("classify_tracking_inactivity_period", {
          request: { periodId, category },
        })
        await refreshTrackingStatus()
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err))
        throw err
      } finally {
        setLoading(false)
      }
    },
    [refreshTrackingStatus]
  )

  const skipTrackingInactivityClassification = useCallback(async () => {
    setLoading(true)
    setError(null)
    try {
      await trackedInvoke("skip_tracking_inactivity_classification")
      await refreshTrackingStatus()
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
      throw err
    } finally {
      setLoading(false)
    }
  }, [refreshTrackingStatus])

  const confirmManualWork = useCallback(async () => {
    setLoading(true)
    setError(null)
    try {
      await trackedInvoke("confirm_manual_work")
      await refreshTrackingStatus()
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
      throw err
    } finally {
      setLoading(false)
    }
  }, [refresh])

  const dismissManualWorkCheck = useCallback(async () => {
    setLoading(true)
    setError(null)
    try {
      await trackedInvoke("dismiss_manual_work_check")
      await refreshTrackingStatus()
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
      throw err
    } finally {
      setLoading(false)
    }
  }, [refreshTrackingStatus])

  const dismissInactivityPeriod = useCallback(async () => {
    setLoading(true)
    setError(null)
    try {
      await trackedInvoke("dismiss_inactivity_period")
      await refreshTrackingStatus()
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
      throw err
    } finally {
      setLoading(false)
    }
  }, [refreshTrackingStatus])

  const dismissActivityBuffer = useCallback(async () => {
    setLoading(true)
    setError(null)
    try {
      await trackedInvoke("dismiss_activity_buffer")
      await refreshTrackingStatus()
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
      throw err
    } finally {
      setLoading(false)
    }
  }, [refreshTrackingStatus])

  const displayTracking = useMemo(() => tracking, [tracking])

  return {
    tracking: displayTracking,
    displayElapsedSeconds,
    taskElapsedSeconds,
    refreshTaskElapsed,
    projects,
    loading,
    error,
    refresh,
    loadProjects,
    startTracking,
    restartTracking,
    pauseTracking,
    resumeTracking,
    stopTracking,
    confirmStillWorking,
    classifyTrackingInactivityPeriod,
    skipTrackingInactivityClassification,
    confirmManualWork,
    dismissManualWorkCheck,
    dismissInactivityPeriod,
    dismissActivityBuffer,
  }
}
