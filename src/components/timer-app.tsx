import { useEffect, useMemo, useState } from "react"
import { useTranslation } from "react-i18next"

import { useAuth } from "@/hooks/use-auth"
import { useTrackingSession } from "@/hooks/use-tracking-session"
import { LanguageToggle } from "@/components/language-toggle"
import { ProfileMenu } from "@/components/profile-menu"
import { ThemeToggle } from "@/components/theme-toggle"
import { IdleOverlay } from "@/components/idle-overlay"
import {
  NO_TASK_ID,
  ProjectSelectors,
  TimerFooter,
  TimerSessionControls,
} from "@/components/timer-app-sections"
import { VooworkLogo } from "@/components/voowork-logo"
import {
  idlePhaseClassName,
  idlePhaseLabel,
  timerRingClassName,
} from "@/lib/idle-display"
import { cn } from "@/lib/utils"

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
    session,
    loading,
    error,
    startSession,
    stopSession,
    pauseSession,
    resumeSession,
    projects,
    confirmStillWorking,
    skipIdleClassification,
    confirmManualWork,
    dismissManualWorkCheck,
    refresh,
  } = useTrackingSession()
  const [projectId, setProjectId] = useState("")
  const [taskId, setTaskId] = useState(NO_TASK_ID)

  useEffect(() => {
    if (!auth.isAuthenticated) {
      return
    }
    refresh().catch(() => undefined)
  }, [auth.isAuthenticated, refresh])

  const resolvedProjectId = projectId || projects[0]?.id || ""
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

  const time = formatElapsed(session.elapsedSeconds)
  const active = session.active
  const idlePhase = session.idle.phase
  const manuallyPaused = idlePhase === "manual_paused"
  const manualWorkCheck = idlePhase === "manual_work_check"
  const autoIdlePaused =
    idlePhase === "paused_idle" || idlePhase === "resume_prompt"
  const canPause =
    active && !manuallyPaused && !manualWorkCheck && !autoIdlePaused
  const handleLogout = async () => {
    if (session.active) {
      await stopSession()
    }
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
                  idlePhaseClassName(idlePhase)
                )}
              >
                <span className="voowork-live-dot" />
                {idlePhaseLabel(idlePhase, t)}
                {session.idle.meetingExempt ? ` · ${t("common.call")}` : null}
              </span>
            ) : null}
            <ProfileMenu
              auth={auth}
              loading={loading || authLoading}
              onLogout={handleLogout}
            />
            <LanguageToggle />
            <ThemeToggle />
          </div>
        </header>

        <div className="flex flex-1 flex-col items-center justify-center py-4">
          <div className="relative flex aspect-square w-full max-w-[220px] items-center justify-center">
            <div
              className={cn(
                "voowork-timer-ring absolute inset-0 rounded-full",
                timerRingClassName(idlePhase, active)
              )}
            />
            <div className="bg-card/80 relative z-10 flex aspect-square w-[82%] flex-col items-center justify-center rounded-full border shadow-inner backdrop-blur-sm">
              <div className="font-mono text-4xl font-semibold tabular-nums tracking-tight sm:text-5xl">
                <span>{time.hours}</span>
                <span className="text-muted-foreground mx-0.5">:</span>
                <span>{time.minutes}</span>
              </div>
              <div className="text-muted-foreground mt-1 font-mono text-xl tabular-nums">
                {time.seconds}
              </div>
            </div>
          </div>

          <TimerSessionControls
            active={active}
            loading={loading}
            canPause={canPause}
            manuallyPaused={manuallyPaused || manualWorkCheck}
            resolvedProjectId={resolvedProjectId}
            t={t}
            onTogglePause={async () => {
              if (manuallyPaused || manualWorkCheck) {
                await resumeSession()
                return
              }
              if (canPause) {
                await pauseSession()
              }
            }}
            onStop={stopSession}
            onStart={async () => {
              if (!resolvedProjectId) return
              const selectedTaskId =
                resolvedTaskId === NO_TASK_ID ? undefined : resolvedTaskId
              await startSession(resolvedProjectId, selectedTaskId)
            }}
          />
          {error ? (
            <p className="text-destructive mt-3 w-full max-w-xs text-center text-xs">
              {error}
            </p>
          ) : null}
        </div>

        {!active ? (
          <ProjectSelectors
            projects={projects}
            selectedProject={selectedProject}
            selectedTask={selectedTask}
            resolvedProjectId={resolvedProjectId}
            resolvedTaskId={resolvedTaskId}
            loading={loading}
            t={t}
            onProjectChange={(value) => {
              setProjectId(value)
              setTaskId(NO_TASK_ID)
            }}
            onTaskChange={setTaskId}
          />
        ) : null}

        {active && selectedProject ? (
          <div className="border-t py-3 text-center">
            <p className="text-muted-foreground text-xs">
              {selectedProject.name}
              {selectedTask ? ` · ${selectedTask.name}` : null}
            </p>
          </div>
        ) : null}

        <TimerFooter t={t} />
      </div>

      {active && idlePhase !== "manual_paused" ? (
        <IdleOverlay
          idle={session.idle}
          loading={loading}
          onConfirmStillWorking={confirmStillWorking}
          onAcknowledgeReturn={skipIdleClassification}
          onConfirmManualWork={confirmManualWork}
          onDismissManualWork={dismissManualWorkCheck}
        />
      ) : null}
    </div>
  )
}
