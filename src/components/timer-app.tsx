import { useEffect, useMemo, useState } from "react"
import { useTranslation } from "react-i18next"

import { useAuth } from "@/hooks/use-auth"
import { useTrackingSession } from "@/hooks/use-tracking-session"
import { LanguageToggle } from "@/components/language-toggle"
import { ProfileMenu } from "@/components/profile-menu"
import { ThemeToggle } from "@/components/theme-toggle"
import { TrackingInactivityOverlay } from "@/components/tracking-inactivity-overlay"
import { BufferAlert } from "@/components/buffer-alert"
import {
  NO_TASK_ID,
  ProjectSelectors,
  TimerFooter,
  TimerSessionControls,
} from "@/components/timer-app-sections"
import { VooworkLogo } from "@/components/voowork-logo"
import {
  trackingInactivityPhaseClassName,
  trackingInactivityPhaseLabel,
  timerRingClassName,
} from "@/lib/tracking-inactivity-display"
import { trackedInvoke } from "@/lib/tauri"
import { cn } from "@/lib/utils"

const SETTING_SELECTED_PROJECT_ID = "selected_project_id"
const SETTING_SELECTED_TASK_ID = "selected_task_id"

function formatElapsed(seconds: number) {
  const h = Math.floor(seconds / 3600)
  const m = Math.floor((seconds % 3600) / 60)
  const s = seconds % 60
  return {
    hours: String(h).padStart(2, "0"),
    minutes: String(m).padStart(2, "0"),
    seconds: String(s).padStart(2, "0"),
  }
}

export function TimerApp() {
  const { t } = useTranslation()
  const { auth, logout, loading: authLoading } = useAuth()
  const {
    tracking,
    displayElapsedSeconds,
    taskElapsedSeconds,
    refreshTaskElapsed,
    loading,
    error,
    startTracking,
    restartTracking,
    pauseTracking,
    resumeTracking,
    projects,
    confirmStillWorking,
    skipTrackingInactivityClassification,
    confirmManualWork,
    dismissManualWorkCheck,
    dismissActivityBuffer,
    refresh,
  } = useTrackingSession()
  const [projectId, setProjectId] = useState("")
  const [taskId, setTaskId] = useState(NO_TASK_ID)

  const persistSelection = async (nextProjectId: string, nextTaskId: string) => {
    await Promise.all([
      trackedInvoke("set_setting", {
        key: SETTING_SELECTED_PROJECT_ID,
        value: nextProjectId,
      }),
      trackedInvoke("set_setting", {
        key: SETTING_SELECTED_TASK_ID,
        value: nextTaskId === NO_TASK_ID ? "" : nextTaskId,
      }),
    ])
  }

  useEffect(() => {
    if (!auth.isAuthenticated) {
      return
    }

    Promise.all([
      trackedInvoke<string | null>("get_setting", {
        key: SETTING_SELECTED_PROJECT_ID,
      }),
      trackedInvoke<string | null>("get_setting", {
        key: SETTING_SELECTED_TASK_ID,
      }),
    ])
      .then(([savedProjectId, savedTaskId]) => {
        if (savedProjectId) {
          setProjectId(savedProjectId)
        }
        if (savedTaskId) {
          setTaskId(savedTaskId)
        }
      })
      .catch(() => undefined)
  }, [auth.isAuthenticated])

  useEffect(() => {
    if (!auth.isAuthenticated) {
      return
    }
    refresh().catch(() => undefined)
  }, [auth.isAuthenticated, refresh])

  // If the saved projectId doesn't match any known project (e.g. after a
  // db:seed that regenerated UUIDs), fall back to the first project instead
  // of showing a stale UUID in the Select trigger.
  const resolvedProjectId = useMemo(() => {
    const saved = projectId
    if (saved && projects.some((p) => p.id === saved)) return saved
    return projects[0]?.id || ""
  }, [projectId, projects])
  const selectedProject = useMemo(
    () => projects.find((p) => p.id === resolvedProjectId),
    [projects, resolvedProjectId]
  )
  const resolvedTaskId = taskId
  const selectedTask = useMemo(
    () =>
      resolvedTaskId === NO_TASK_ID
        ? undefined
        : selectedProject?.tasks.find((task) => task.id === resolvedTaskId),
    [selectedProject, resolvedTaskId]
  )

  const active = tracking.active
  const idlePhase = tracking.inactivity.phase
  const manuallyPaused =
    active &&
    (idlePhase === "manual_paused" || idlePhase === "manual_work_check")

  const selectionMatchesSession =
    active &&
    resolvedTaskId !== NO_TASK_ID &&
    resolvedTaskId === (tracking.taskId ?? "") &&
    resolvedProjectId === (tracking.projectId ?? "")

  const showLiveSessionTimer = selectionMatchesSession && !manuallyPaused

  useEffect(() => {
    if (showLiveSessionTimer) {
      return
    }
    refreshTaskElapsed(
      resolvedTaskId === NO_TASK_ID ? null : resolvedTaskId
    ).catch(() => undefined)
  }, [
    showLiveSessionTimer,
    resolvedTaskId,
    refreshTaskElapsed,
    tracking.trackingId,
    tracking.active,
    idlePhase,
  ])

  const displaySeconds = showLiveSessionTimer
    ? displayElapsedSeconds
    : selectionMatchesSession && manuallyPaused
      ? tracking.elapsedSeconds
      : taskElapsedSeconds
  const time = formatElapsed(displaySeconds)
  const canStart =
    !active &&
    Boolean(resolvedProjectId) &&
    resolvedTaskId !== NO_TASK_ID
  const handleLogout = async () => {
    await logout()
  }

  return (
    <div className="voowork-shell flex h-full min-h-0 flex-col">
      <div className="mx-auto flex w-full max-w-lg flex-1 flex-col px-6">
        <header className="flex items-start justify-between gap-3 pt-5">
          <div className="min-w-0 flex-1">
            <VooworkLogo size="md" />
          </div>
          <div className="flex shrink-0 items-center gap-1.5">
            {active ? (
              <span
                className={cn(
                  "voowork-live-pill text-[11px] font-medium",
                  trackingInactivityPhaseClassName(idlePhase)
                )}
              >
                <span className="voowork-live-dot" />
                {trackingInactivityPhaseLabel(idlePhase, t)}
                {tracking.inactivity.meetingExempt ? ` · ${t("common.call")}` : null}
              </span>
            ) : null}
            <LanguageToggle />
            <ThemeToggle />
            <ProfileMenu
              auth={auth}
              loading={loading || authLoading}
              onLogout={handleLogout}
            />
          </div>
        </header>

        <div className="flex flex-1 flex-col items-center justify-center py-4">
          <div className="relative flex aspect-square w-full max-w-[220px] flex-col items-center justify-center">
            <div
              className={cn(
                "voowork-timer-ring absolute inset-0 rounded-full",
                timerRingClassName(idlePhase, active)
              )}
            />
            <TimerSessionControls
              active={active && !manuallyPaused}
              loading={loading}
              canStart={canStart}
              canResume={manuallyPaused}
              resolvedProjectId={resolvedProjectId}
              resolvedTaskId={resolvedTaskId}
              t={t}
              onToggle={async () => {
                if (active) {
                  if (
                    idlePhase === "manual_paused" ||
                    idlePhase === "manual_work_check"
                  ) {
                    const selectionChanged =
                      resolvedProjectId !== (tracking.projectId ?? "") ||
                      (resolvedTaskId !== NO_TASK_ID &&
                        resolvedTaskId !== (tracking.taskId ?? ""))
                    if (selectionChanged && resolvedTaskId !== NO_TASK_ID) {
                      await restartTracking(resolvedProjectId, resolvedTaskId)
                      await refreshTaskElapsed(resolvedTaskId)
                      return
                    }
                    await resumeTracking()
                    return
                  }
                  await pauseTracking()
                  return
                }
                if (!canStart) return
                await startTracking(resolvedProjectId, resolvedTaskId)
                await refreshTaskElapsed(resolvedTaskId)
              }}
            />
          </div>

          <div className="mt-6 text-center">
            <div className="font-mono text-5xl font-semibold tabular-nums tracking-tight sm:text-6xl">
              <span>{time.hours}</span>
              <span className="text-muted-foreground mx-1">:</span>
              <span>{time.minutes}</span>
              <span className="text-muted-foreground mx-1">:</span>
              <span className="text-muted-foreground">{time.seconds}</span>
            </div>
          </div>
          {error ? (
            <p className="text-destructive mt-3 w-full max-w-xs text-center text-xs">
              {error}
            </p>
          ) : null}
        </div>

        <ProjectSelectors
          projects={projects}
          selectedProject={selectedProject}
          selectedTask={selectedTask}
          resolvedProjectId={resolvedProjectId}
          resolvedTaskId={resolvedTaskId}
          loading={loading}
          disabled={active && !manuallyPaused}
          t={t}
          onProjectChange={(value) => {
            setProjectId(value)
            setTaskId(NO_TASK_ID)
            void persistSelection(value, NO_TASK_ID)
          }}
          onTaskChange={(newTaskId) => {
            setTaskId(newTaskId)
            void persistSelection(resolvedProjectId, newTaskId)
          }}
        />

        <TimerFooter t={t} />
      </div>

      {active &&
      (idlePhase === "manual_work_check" ||
        idlePhase === "paused_inactivity" ||
        idlePhase === "resume_prompt" ||
        idlePhase === "warning" ||
        idlePhase === "countdown") ? (
        <TrackingInactivityOverlay
          inactivity={tracking.inactivity}
          loading={loading}
          onConfirmStillWorking={confirmStillWorking}
          onAcknowledgeReturn={skipTrackingInactivityClassification}
          onConfirmManualWork={confirmManualWork}
          onDismissManualWork={dismissManualWorkCheck}
        />
      ) : null}

      {auth.isAuthenticated &&
      !active &&
      !loading &&
      tracking.activityBufferAlert &&
      resolvedTaskId !== NO_TASK_ID ? (
        <BufferAlert
          bufferSeconds={tracking.activityBufferSeconds}
          loading={loading}
          onDismiss={dismissActivityBuffer}
          onStart={async () => {
            if (!canStart) return
            await startTracking(resolvedProjectId, resolvedTaskId)
          }}
        />
      ) : null}
    </div>
  )
}
