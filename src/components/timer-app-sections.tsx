import { PauseIcon, PlayIcon } from "lucide-react"
import type { TFunction } from "i18next"

import type { ProjectOption } from "@/hooks/use-tracking-session"
import { openWebPanel } from "@/lib/navigation"
import { AppMeta } from "@/components/app-meta"
import { Button } from "@/components/ui/button"
import { Label } from "@/components/ui/label"
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

type TimerControlsProps = Readonly<{
  active: boolean
  loading: boolean
  canStart: boolean
  canResume: boolean
  resolvedProjectId: string
  resolvedTaskId: string
  t: TFunction
  onToggle: () => Promise<void>
}>

export function TimerSessionControls({
  active,
  loading,
  canStart,
  canResume,
  t,
  onToggle,
}: TimerControlsProps) {
  const disabled = loading || (!active && !canStart && !canResume)

  return (
    <button
      type="button"
      aria-label={active ? t("timer.pause") : t("timer.start")}
      className={cn(
        "voowork-timer-play-surface absolute inset-0 z-10 m-auto flex size-[82%] max-w-[180px] items-center justify-center rounded-full border transition-transform",
        !disabled && "hover:scale-[1.02] active:scale-[0.98]",
        disabled && "cursor-not-allowed opacity-60"
      )}
      disabled={disabled}
      onClick={() => {
        onToggle().catch(() => undefined)
      }}
    >
      {active ? (
        <PauseIcon className="text-primary relative z-10 size-14 fill-current" />
      ) : (
        <PlayIcon className="text-primary relative z-10 size-14 fill-current" />
      )}
    </button>
  )
}

type ProjectSelectorsProps = Readonly<{
  projects: ProjectOption[]
  selectedProject?: ProjectOption
  selectedTask?: ProjectOption["tasks"][number]
  resolvedProjectId: string
  resolvedTaskId: string
  loading: boolean
  disabled?: boolean
  t: TFunction
  onProjectChange: (projectId: string) => void | Promise<void>
  onTaskChange: (taskId: string) => void | Promise<void>
}>

export function ProjectSelectors({
  projects,
  selectedProject,
  selectedTask,
  resolvedProjectId,
  resolvedTaskId,
  loading,
  disabled = false,
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

  const taskLabel = selectedTask?.name ?? t("timer.selectTask")

  return (
    <div className="space-y-3 border-t py-4">
      <div className="space-y-1.5">
        <Label htmlFor="timer-project" className="text-muted-foreground text-xs">
          {t("timer.project")}
        </Label>
        <Select
          value={resolvedProjectId}
          onValueChange={(value) => {
            void onProjectChange(value ?? "")
          }}
          disabled={loading || disabled}
        >
          <SelectTrigger id="timer-project" className="h-10 w-full rounded-xl text-sm">
            <SelectValue placeholder={t("timer.selectProject")}>
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
      </div>
      {selectedProject ? (
        <div className="space-y-1.5">
          <Label htmlFor="timer-task" className="text-muted-foreground text-xs">
            {t("timer.task")}
          </Label>
          <Select
            value={resolvedTaskId}
            onValueChange={(value) => {
              void onTaskChange(value ?? NO_TASK_ID)
            }}
            disabled={loading || disabled}
          >
            <SelectTrigger id="timer-task" className="h-10 w-full rounded-xl text-sm">
              <SelectValue placeholder={t("timer.selectTask")}>{taskLabel}</SelectValue>
            </SelectTrigger>
            <SelectContent {...selectContentProps}>
              {selectedProject.tasks.map((task) => (
                <SelectItem key={task.id} value={task.id}>
                  {task.name}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
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
          openWebPanel().catch(() => undefined)
        }}
      >
        {t("timer.openWebPanel")}
      </Button>
      <AppMeta className="mt-3" />
    </footer>
  )
}
