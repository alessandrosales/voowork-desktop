import { ExternalLinkIcon, PauseIcon, PlayIcon, SquareIcon } from "lucide-react"
import { openUrl } from "@tauri-apps/plugin-opener"
import type { TFunction } from "i18next"

import type { ProjectOption } from "@/hooks/use-tracking-session"
import { AppMeta } from "@/components/app-meta"
import { Button } from "@/components/ui/button"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { cn } from "@/lib/utils"

export const NO_TASK_ID = "__none__"

const selectContentProps = {
  side: "bottom" as const,
  alignItemWithTrigger: false,
  sideOffset: 8,
  className: "z-[200]",
}

type TimerSessionControlsProps = Readonly<{
  active: boolean
  loading: boolean
  canPause: boolean
  manuallyPaused: boolean
  resolvedProjectId: string
  t: TFunction
  onTogglePause: () => Promise<void>
  onStop: () => Promise<void>
  onStart: () => Promise<void>
}>

export function TimerSessionControls({
  active,
  loading,
  canPause,
  manuallyPaused,
  resolvedProjectId,
  t,
  onTogglePause,
  onStop,
  onStart,
}: TimerSessionControlsProps) {
  if (!active) {
    return (
      <>
        <p className="text-muted-foreground mt-5 text-center text-sm">
          {t("timer.clickToStart")}
        </p>
        <Button
          size="lg"
          className="voowork-start-btn mt-5 h-12 w-full max-w-xs rounded-2xl text-base font-semibold text-primary-foreground shadow-lg transition-all"
          onClick={() => {
            onStart().catch(() => undefined)
          }}
          disabled={loading || !resolvedProjectId}
        >
          <PlayIcon className="size-5" />
          {t("timer.startSession")}
        </Button>
      </>
    )
  }

  return (
    <div className="mt-5 flex w-full max-w-xs flex-col gap-2">
      {(canPause || manuallyPaused) && (
        <Button
          size="lg"
          className={cn(
            "h-12 w-full rounded-2xl text-base font-semibold shadow-lg transition-all",
            manuallyPaused
              ? "voowork-start-btn text-primary-foreground"
              : "voowork-stop-btn text-foreground"
          )}
          onClick={() => {
            onTogglePause().catch(() => undefined)
          }}
          disabled={loading}
        >
          {manuallyPaused ? (
            <>
              <PlayIcon className="size-5" />
              {t("timer.resume")}
            </>
          ) : (
            <>
              <PauseIcon className="size-5" />
              {t("timer.pause")}
            </>
          )}
        </Button>
      )}
      <Button
        size="lg"
        variant="outline"
        className="h-11 w-full rounded-2xl text-sm font-medium"
        onClick={() => {
          onStop().catch(() => undefined)
        }}
        disabled={loading}
      >
        <SquareIcon className="size-4" />
        {t("timer.stopSession")}
      </Button>
    </div>
  )
}

type ProjectSelectorsProps = Readonly<{
  projects: ProjectOption[]
  selectedProject?: ProjectOption
  selectedTask?: ProjectOption["tasks"][number]
  resolvedProjectId: string
  resolvedTaskId: string
  loading: boolean
  t: TFunction
  onProjectChange: (projectId: string) => void
  onTaskChange: (taskId: string) => void
}>

export function ProjectSelectors({
  projects,
  selectedProject,
  selectedTask,
  resolvedProjectId,
  resolvedTaskId,
  loading,
  t,
  onProjectChange,
  onTaskChange,
}: ProjectSelectorsProps) {
  if (projects.length === 0) {
    return (
      <p className="text-muted-foreground border-t py-4 text-center text-xs leading-relaxed">
        {t("timer.noProjects")}
      </p>
    )
  }

  const taskLabel =
    resolvedTaskId === NO_TASK_ID
      ? t("timer.noTask")
      : selectedTask?.name

  return (
    <div className="space-y-2.5 border-t py-4">
      <Select
        value={resolvedProjectId}
        onValueChange={(value) => onProjectChange(value ?? "")}
        disabled={loading}
      >
        <SelectTrigger className="h-10 w-full rounded-xl text-sm">
          <SelectValue placeholder={t("timer.project")}>
            {selectedProject?.name}
          </SelectValue>
        </SelectTrigger>
        <SelectContent {...selectContentProps}>
          {projects.map((project) => (
            <SelectItem key={project.id} value={project.id}>
              {project.name}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
      {selectedProject ? (
        <Select
          value={resolvedTaskId}
          onValueChange={(value) => onTaskChange(value ?? NO_TASK_ID)}
          disabled={loading}
        >
          <SelectTrigger className="h-10 w-full rounded-xl text-sm">
            <SelectValue placeholder={t("timer.task")}>{taskLabel}</SelectValue>
          </SelectTrigger>
          <SelectContent {...selectContentProps}>
            <SelectItem value={NO_TASK_ID}>{t("timer.noTask")}</SelectItem>
            {selectedProject.tasks.map((task) => (
              <SelectItem key={task.id} value={task.id}>
                {task.name}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      ) : null}
    </div>
  )
}

type TimerFooterProps = Readonly<{
  t: TFunction
}>

export function TimerFooter({ t }: TimerFooterProps) {
  return (
    <footer className="border-t py-4 pb-5">
      <Button
        variant="ghost"
        size="sm"
        className="text-muted-foreground h-9 w-full justify-center gap-2 text-xs"
        onClick={() => {
          openUrl("https://app.voowork.com").catch(() => undefined)
        }}
      >
        <ExternalLinkIcon className="size-3.5" />
        {t("timer.openWebPanel")}
      </Button>
      <AppMeta className="mt-3" />
    </footer>
  )
}
