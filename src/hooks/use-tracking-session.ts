import { useCallback, useEffect, useMemo, useState } from "react"
import { listen } from "@tauri-apps/api/event"

import { trackedInvoke, isTauriReady } from "@/lib/tauri"

export type IdleStatus = {
  phase: string
  thresholdSecs: number
  countdownSecs: number
  countdownRemainingSecs: number | null
  countdownEndsAt: string | null
  idleStartedAt: string | null
  pausedAt: string | null
  awaySeconds: number | null
  pendingPeriodId: string | null
  meetingExempt: boolean
  activeSeconds: number
  idleDiscardedSeconds: number
  idleReclassifiedSeconds: number
}

export type SessionStatus = {
  active: boolean
  sessionId: string | null
  projectId: string | null
  taskId: string | null
  startedAt: string | null
  elapsedSeconds: number
  mouseEvents: number
  keyboardEvents: number
  clockSkewDetected: boolean
  activityConfidence: number
  trackerMode: string | null
  currentApp: string | null
  currentWindowTitle: string | null
  screenshotCount: number
  lastScreenshotAt: string | null
  idle: IdleStatus
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

const EMPTY_IDLE: IdleStatus = {
  phase: "active",
  thresholdSecs: 120,
  countdownSecs: 60,
  countdownRemainingSecs: null,
  countdownEndsAt: null,
  idleStartedAt: null,
  pausedAt: null,
  awaySeconds: null,
  pendingPeriodId: null,
  meetingExempt: false,
  activeSeconds: 0,
  idleDiscardedSeconds: 0,
  idleReclassifiedSeconds: 0,
}

const EMPTY_SESSION: SessionStatus = {
  active: false,
  sessionId: null,
  projectId: null,
  taskId: null,
  startedAt: null,
  elapsedSeconds: 0,
  mouseEvents: 0,
  keyboardEvents: 0,
  clockSkewDetected: false,
  activityConfidence: 1,
  trackerMode: null,
  currentApp: null,
  currentWindowTitle: null,
  screenshotCount: 0,
  lastScreenshotAt: null,
  idle: EMPTY_IDLE,
}

function pollIntervalMs(session: SessionStatus) {
  if (!session.active) {
    return 2000
  }
  if (
    session.idle.phase !== "active" &&
    session.idle.phase !== "manual_paused"
  ) {
    return 500
  }
  return 1000
}

export function useTrackingSession() {
  const [session, setSession] = useState<SessionStatus>(EMPTY_SESSION)
  const [projects, setProjects] = useState<ProjectOption[]>([])
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const refresh = useCallback(async () => {
    if (!isTauriReady()) {
      return
    }

    try {
      const [sessionStatus, projectList] = await Promise.all([
        trackedInvoke<SessionStatus>("get_session_status"),
        trackedInvoke<ProjectOption[]>("list_projects"),
      ])
      setSession(sessionStatus)
      setProjects(projectList)
      setError(null)
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    }
  }, [])

  const pollMs = pollIntervalMs(session)

  useEffect(() => {
    const timer = window.setTimeout(() => {
      refresh().catch(() => undefined)
    }, 0)
    const interval = window.setInterval(() => {
      refresh().catch(() => undefined)
    }, pollMs)
    return () => {
      window.clearTimeout(timer)
      window.clearInterval(interval)
    }
  }, [refresh, pollMs])

  useEffect(() => {
    if (!isTauriReady()) {
      return
    }

    let unlisten: (() => void) | undefined
    listen("idle-changed", () => {
      refresh().catch(() => undefined)
    })
      .then((dispose) => {
        unlisten = dispose
      })
      .catch(() => undefined)

    return () => {
      unlisten?.()
    }
  }, [refresh])

  const startSession = useCallback(
    async (projectId: string, taskId?: string) => {
      setLoading(true)
      setError(null)
      try {
        await trackedInvoke("start_session", {
          request: { projectId, taskId: taskId ?? null },
        })
        await refresh()
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err))
        throw err
      } finally {
        setLoading(false)
      }
    },
    [refresh]
  )

  const stopSession = useCallback(async () => {
    setLoading(true)
    setError(null)
    try {
      await trackedInvoke("stop_session")
      await refresh()
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
      throw err
    } finally {
      setLoading(false)
    }
  }, [refresh])

  const confirmStillWorking = useCallback(async () => {
    setLoading(true)
    setError(null)
    try {
      await trackedInvoke("confirm_still_working")
      await refresh()
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
      throw err
    } finally {
      setLoading(false)
    }
  }, [refresh])

  const classifyIdlePeriod = useCallback(
    async (periodId: string, category: string) => {
      setLoading(true)
      setError(null)
      try {
        await trackedInvoke("classify_idle_period", {
          request: { periodId, category },
        })
        await refresh()
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err))
        throw err
      } finally {
        setLoading(false)
      }
    },
    [refresh]
  )

  const skipIdleClassification = useCallback(async () => {
    setLoading(true)
    setError(null)
    try {
      await trackedInvoke("skip_idle_classification")
      await refresh()
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
      throw err
    } finally {
      setLoading(false)
    }
  }, [refresh])

  const pauseSession = useCallback(async () => {
    await trackedInvoke("pause_session")
    await refresh()
  }, [refresh])

  const resumeSession = useCallback(async () => {
    await trackedInvoke("resume_session")
    await refresh()
  }, [refresh])

  const confirmManualWork = useCallback(async () => {
    setLoading(true)
    setError(null)
    try {
      await trackedInvoke("confirm_manual_work")
      await refresh()
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
      await refresh()
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
      throw err
    } finally {
      setLoading(false)
    }
  }, [refresh])

  const displaySession = useMemo(() => session, [session])

  return {
    session: displaySession,
    projects,
    loading,
    error,
    refresh,
    startSession,
    stopSession,
    pauseSession,
    resumeSession,
    confirmStillWorking,
    classifyIdlePeriod,
    skipIdleClassification,
    confirmManualWork,
    dismissManualWorkCheck,
  }
}
