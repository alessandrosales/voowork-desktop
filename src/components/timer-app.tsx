import { FolderIcon, LayoutGridIcon, SquareIcon } from "lucide-react"
import { useCallback, useEffect, useMemo, useRef, useState } from "react"
import { useTranslation } from "react-i18next"

import { useAuth } from "@/hooks/use-auth"
import { useTrackingSession } from "@/hooks/use-tracking-session"
import { LanguageToggle } from "@/components/language-toggle"
import { ProfileMenu } from "@/components/profile-menu"
import { ThemeToggle } from "@/components/theme-toggle"
import { TrackingInactivityOverlay } from "@/components/tracking-inactivity-overlay"
import { BufferAlert } from "@/components/buffer-alert"
import { SettingsView } from "@/components/settings-view"
import {
  TimerFooter,
  TimerSessionControls,
} from "@/components/timer-app-sections"
import { VooworkLogo } from "@/components/voowork-logo"
import { NO_TASK_ID, WorkspaceView } from "@/components/workspace-view"
import {
  trackingInactivityPhaseClassName,
  trackingInactivityPhaseLabel,
  timerRingClassName,
} from "@/lib/tracking-inactivity-display"
import { trackedInvoke } from "@/lib/tauri"
import { cn, formatElapsed } from "@/lib/utils"

const SETTING_SELECTED_PROJECT_ID = "selected_project_id"
const SETTING_SELECTED_TASK_ID = "selected_task_id"

type View = "timer" | "workspace" | "settings"

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
    projectsLoading,
    confirmStillWorking,
    skipTrackingInactivityClassification,
    classifyTrackingInactivityPeriod,
    classifyPausedInactivityPeriod,
    confirmManualWork,
    dismissManualWorkCheck,
    dismissInactivityPeriod,
    dismissActivityBuffer,
    refresh,
    loadProjects,
    stopTracking,
  } = useTrackingSession()
  const [projectId, setProjectId] = useState("")
  const [taskId, setTaskId] = useState(NO_TASK_ID)
  const [view, setView] = useState<View>("timer")

  const persistSelection = useCallback(
    async (nextProjectId: string, nextTaskId: string) => {
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
    },
    [],
  )

  // Debounce persistSelection: coalesce rapid selection changes
  // (e.g. keyboard arrow mashing) into a single save.
  const pendingSelectionRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const debouncedPersistSelection = useCallback(
    (nextProjectId: string, nextTaskId: string) => {
      if (pendingSelectionRef.current) {
        clearTimeout(pendingSelectionRef.current)
      }
      pendingSelectionRef.current = setTimeout(() => {
        pendingSelectionRef.current = null
        void persistSelection(nextProjectId, nextTaskId)
      }, 300)
    },
    [persistSelection],
  )

  // Cleanup pending save on unmount
  useEffect(() => {
    return () => {
      if (pendingSelectionRef.current) {
        clearTimeout(pendingSelectionRef.current)
      }
    }
  }, [])

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

  const resolvedProjectId = useMemo(() => {
    const saved = projectId
    if (saved && projects.some((p) => p.id === saved)) return saved
    return ""
  }, [projectId, projects])
  const selectedProject = useMemo(
    () => projects.find((p) => p.id === resolvedProjectId),
    [projects, resolvedProjectId]
  )
  const resolvedTaskId = useMemo(() => {
    if (taskId === NO_TASK_ID) return NO_TASK_ID
    if (
      selectedProject?.tasks &&
      !selectedProject.tasks.some((t) => t.id === taskId)
    ) {
      return NO_TASK_ID
    }
    return taskId
  }, [taskId, selectedProject])
  // Clear stale persisted task selection when task no longer exists in project
  useEffect(() => {
    if (
      taskId !== NO_TASK_ID &&
      selectedProject?.tasks &&
      !selectedProject.tasks.some((t) => t.id === taskId)
    ) {
      trackedInvoke("set_setting", {
        key: SETTING_SELECTED_TASK_ID,
        value: "",
      }).catch(() => undefined)
    }
  }, [taskId, selectedProject])
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

  // Pré-carrega o tempo acumulado da task selecionada sempre que o selector
  // muda, mesmo enquanto ativamente trackeando. Quando o usuário pausar ou
  // trocar de task, o taskElapsedSeconds já estará correto.
  useEffect(() => {
    if (resolvedTaskId === NO_TASK_ID) {
      return
    }
    refreshTaskElapsed(resolvedTaskId).catch(() => undefined)
  }, [resolvedTaskId, refreshTaskElapsed])

  const displaySeconds = showLiveSessionTimer
    ? displayElapsedSeconds
    : selectionMatchesSession && manuallyPaused
      ? tracking.elapsedSeconds
      : taskElapsedSeconds
  const time = formatElapsed(displaySeconds)
  const hasSelection =
    Boolean(resolvedProjectId) && resolvedTaskId !== NO_TASK_ID
  const canStart = !active && hasSelection
  const handleLogout = async () => {
    if (active) {
      const confirmed = window.confirm(t("profile.confirmLogoutWhileTracking"))
      if (!confirmed) return
    }
    await logout()
  }

  const handleStop = async () => {
    if (!active) return
    const confirmed = window.confirm(t("timer.confirmStop"))
    if (!confirmed) return
    await stopTracking()
  }

  const handleSelectTask = (nextProjectId: string, nextTaskId: string) => {
    setProjectId(nextProjectId)
    setTaskId(nextTaskId)
    debouncedPersistSelection(nextProjectId, nextTaskId)
  }

  const handleOpenWorkspace = () => {
    setView("workspace")
  }

  const handleOpenSettings = () => {
    setView("settings")
  }

  const handleBackToTimer = () => {
    setView("timer")
  }

  // Carrega projetos quando abre workspace + stale-while-revalidate a cada 5 min.
  useEffect(() => {
    if (view !== "workspace") {
      return
    }
    loadProjects().catch(() => undefined)
    const interval = window.setInterval(() => {
      loadProjects().catch(() => undefined)
    }, 5 * 60 * 1000)
    return () => window.clearInterval(interval)
  }, [view, loadProjects])

  // ─── Overlays (shared across views) ──────────────────────
  const inactivityOverlay =
    active &&
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
        onClassifyInactivity={async () => {
          if (tracking.inactivity.pendingPeriodId) {
            await classifyTrackingInactivityPeriod(
              tracking.inactivity.pendingPeriodId,
              "offline_work"
            )
          }
        }}
        onClassifyPausedInactivity={classifyPausedInactivityPeriod}
        onConfirmManualWork={confirmManualWork}
        onDismissManualWork={dismissManualWorkCheck}
        onPauseTracking={pauseTracking}
        onReturnToWork={dismissInactivityPeriod}
      />
    ) : null

  const bufferAlertBlock =
    auth.isAuthenticated &&
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
    ) : null

  // ─── Settings view ────────────────────────────────────────
  if (view === "settings") {
    return (
      <>
        <SettingsView onBack={handleBackToTimer} />
        {inactivityOverlay}
        {bufferAlertBlock}
      </>
    )
  }

  // ─── Workspace view ───────────────────────────────────────
  if (view === "workspace") {
    return (
      <>
        <WorkspaceView
          projects={projects}
          projectsLoading={projectsLoading}
          resolvedProjectId={resolvedProjectId}
          resolvedTaskId={resolvedTaskId}
          disabled={active && !manuallyPaused}
          onSelect={handleSelectTask}
          onBack={handleBackToTimer}
        />
        {inactivityOverlay}
        {bufferAlertBlock}
      </>
    )
  }

  // ─── Timer view ───────────────────────────────────────────
  return (
    <div className="voowork-shell flex h-full min-h-0 flex-col">
      <div className="mx-auto flex w-full max-w-lg flex-1 flex-col px-6">
        <header className="flex items-center justify-between gap-3 pt-5">
          <div className="min-w-0 flex-1">
            <VooworkLogo size="compact" />
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
              onOpenSettings={handleOpenSettings}
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

          {/* Stop button — visible only when tracking is active */}
          {active && !manuallyPaused ? (
            <button
              type="button"
              className="voowork-stop-btn mt-4 flex items-center gap-2 rounded-lg border border-destructive/30 px-4 py-2 text-sm font-medium text-destructive transition-all hover:bg-destructive/10 hover:border-destructive/50 disabled:opacity-40"
              onClick={handleStop}
              disabled={loading}
            >
              <SquareIcon className="size-4 fill-destructive/70" />
              {t("timer.stop")}
            </button>
          ) : null}

          {/* Workspace card */}
          <div className="mt-6 w-full max-w-xs space-y-2">
            <p className="text-muted-foreground/60 text-center text-[11px] font-medium uppercase tracking-widest">
              {hasSelection
                ? t("workspace.workingOn")
                : t("workspace.selectWork")}
            </p>

            <button
              type="button"
              onClick={handleOpenWorkspace}
              className={cn(
                "flex w-full items-center gap-3 rounded-xl border px-4 py-3 text-left transition-all",
                hasSelection
                  ? "border-border bg-card hover:border-muted-foreground/40 hover:shadow-sm"
                  : "border-dashed border-muted-foreground/30 text-muted-foreground hover:border-muted-foreground/50 hover:text-accent-foreground"
              )}
            >
              <FolderIcon
                className={cn(
                  "size-5 shrink-0",
                  hasSelection
                    ? "text-primary"
                    : "text-muted-foreground"
                )}
              />
              <div className="flex-1 truncate">
                {hasSelection ? (
                  <>
                    <p className="truncate text-sm font-medium">
                      {selectedProject?.name}
                    </p>
                    <p className="text-muted-foreground truncate text-xs">
                      {selectedTask?.name}
                    </p>
                  </>
                ) : (
                  <p className="text-sm">{t("timer.clickToStart")}</p>
                )}
              </div>
              <LayoutGridIcon className="text-muted-foreground size-4 shrink-0" />
            </button>
          </div>
        </div>

        <TimerFooter t={t} />
      </div>

      {error ? (
        <p className="text-destructive mx-auto mt-1 w-full max-w-xs text-center text-xs">
          {error}
        </p>
      ) : null}

      {inactivityOverlay}

      {bufferAlertBlock}
    </div>
  )
}
