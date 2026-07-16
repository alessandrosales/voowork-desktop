import {
  ArrowLeftIcon,
  CheckIcon,
  ChevronDownIcon,
  ChevronRightIcon,
  FolderIcon,
  SearchIcon,
  XIcon,
} from "lucide-react"
import { useEffect, useMemo, useState } from "react"
import { useTranslation } from "react-i18next"

import type { ProjectOption } from "@/hooks/use-tracking-session"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { cn } from "@/lib/utils"

export const NO_TASK_ID = "__none__"

type WorkspaceViewProps = Readonly<{
  projects: ProjectOption[]
  resolvedProjectId: string
  resolvedTaskId: string
  disabled?: boolean
  onSelect: (projectId: string, taskId: string) => void
  onBack: () => void
}>

export function WorkspaceView({
  projects,
  resolvedProjectId,
  resolvedTaskId,
  disabled = false,
  onSelect,
  onBack,
}: WorkspaceViewProps) {
  const { t } = useTranslation()
  const [search, setSearch] = useState("")
  const [expandedProjects, setExpandedProjects] = useState<Set<string>>(
    () => new Set(resolvedProjectId ? [resolvedProjectId] : [])
  )

  const query = search.toLowerCase().trim()

  const filteredProjects = useMemo(() => {
    if (!query) return projects

    return projects
      .map((project) => {
        const projectMatch = project.name.toLowerCase().includes(query)
        const matchingTasks = projectMatch
          ? project.tasks
          : project.tasks.filter((task) =>
              task.name.toLowerCase().includes(query)
            )
        if (!projectMatch && matchingTasks.length === 0) return null
        return { ...project, tasks: matchingTasks }
      })
      .filter(Boolean) as ProjectOption[]
  }, [projects, query])

  const toggleProject = (projectId: string) => {
    setExpandedProjects((prev) => {
      const next = new Set(prev)
      if (next.has(projectId)) {
        next.delete(projectId)
      } else {
        next.add(projectId)
      }
      return next
    })
  }

  // Auto-expand all matching projects during search
  useEffect(() => {
    if (query) {
      setExpandedProjects(new Set(filteredProjects.map((p) => p.id)))
    }
  }, [query, filteredProjects])

  const handleSelectTask = (projectId: string, taskId: string) => {
    onSelect(projectId, taskId)
  }

  const noProjects = projects.length === 0

  return (
    <div className="flex h-full flex-col">
      {/* Header */}
      <header className="flex items-center gap-3 border-b px-6 py-3">
        <button
          type="button"
          onClick={onBack}
          className="text-muted-foreground hover:text-foreground -ml-1 flex items-center gap-1 rounded-lg px-2 py-1 text-sm transition-colors"
        >
          <ArrowLeftIcon className="size-4" />
          {t("workspace.back")}
        </button>
        <h1 className="text-base font-semibold">{t("workspace.title")}</h1>
      </header>

      {/* Content */}
      <div className="flex-1 overflow-y-auto px-6 py-4">
        {/* Search */}
        <div className="relative mb-5">
          <SearchIcon className="text-muted-foreground pointer-events-none absolute left-3 top-1/2 size-4 -translate-y-1/2" />
          <Input
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder={t("workspace.searchPlaceholder")}
            className="h-10 rounded-xl pl-9 pr-9 text-sm"
            autoFocus
          />
          {search && (
            <button
              type="button"
              onClick={() => setSearch("")}
              className="text-muted-foreground hover:text-foreground absolute right-2.5 top-1/2 -translate-y-1/2"
            >
              <XIcon className="size-4" />
            </button>
          )}
        </div>

        {/* No projects */}
        {noProjects ? (
          <div className="flex flex-1 flex-col items-center justify-center py-16">
            <FolderIcon className="text-muted-foreground/30 mb-3 size-12" />
            <p className="text-muted-foreground text-center text-sm">
              {t("timer.noProjects")}
            </p>
          </div>
        ) : filteredProjects.length === 0 ? (
          <div className="flex flex-1 flex-col items-center justify-center py-16">
            <SearchIcon className="text-muted-foreground/30 mb-3 size-12" />
            <p className="text-muted-foreground text-center text-sm">
              {t("workspace.noResults")}
            </p>
          </div>
        ) : (
          /* Project list */
          <div className="space-y-1">
            {filteredProjects.map((project) => {
              const isExpanded = expandedProjects.has(project.id)
              const isSelectedProject = project.id === resolvedProjectId

              return (
                <div key={project.id}>
                  {/* Project row */}
                  <button
                    type="button"
                    onClick={() => toggleProject(project.id)}
                    className={cn(
                      "flex w-full items-center gap-2.5 rounded-xl px-3.5 py-3 text-left text-sm transition-colors",
                      isSelectedProject
                        ? "bg-accent/60 text-accent-foreground font-medium"
                        : "text-muted-foreground hover:bg-accent/30 hover:text-accent-foreground"
                    )}
                  >
                    {isExpanded ? (
                      <ChevronDownIcon className="size-4 shrink-0" />
                    ) : (
                      <ChevronRightIcon className="size-4 shrink-0" />
                    )}
                    <FolderIcon className="size-4.5 shrink-0" />
                    <span className="flex-1 truncate">{project.name}</span>
                    <span
                      className={cn(
                        "text-[11px] tabular-nums",
                        isSelectedProject
                          ? "text-accent-foreground/60"
                          : "text-muted-foreground/50"
                      )}
                    >
                      {project.tasks.length}{" "}
                      {project.tasks.length === 1
                        ? t("workspace.task")
                        : t("workspace.tasks")}
                    </span>
                  </button>

                  {/* Tasks */}
                  {isExpanded && (
                    <div className="ml-5 pl-4">
                      {project.tasks.length === 0 ? (
                        <p className="text-muted-foreground/50 px-3.5 py-3 text-xs italic">
                          {t("workspace.noTasks")}
                        </p>
                      ) : (
                        <div className="space-y-0.5 py-1">
                          {project.tasks.map((task) => {
                            const isActive =
                              task.id === resolvedTaskId &&
                              project.id === resolvedProjectId
                            return (
                              <button
                                key={task.id}
                                type="button"
                                disabled={disabled}
                                onClick={() =>
                                  handleSelectTask(project.id, task.id)
                                }
                                className={cn(
                                  "flex w-full items-center gap-3 rounded-lg px-3.5 py-2.5 text-left text-sm transition-colors",
                                  isActive
                                    ? "bg-primary/8 text-primary font-medium"
                                    : "text-muted-foreground hover:bg-accent/30 hover:text-accent-foreground",
                                  disabled && "cursor-not-allowed opacity-50"
                                )}
                              >
                                <span className="flex size-4 shrink-0 items-center justify-center">
                                  {isActive ? (
                                    <span className="flex size-4 items-center justify-center rounded-full bg-primary">
                                      <CheckIcon className="size-3 text-primary-foreground" />
                                    </span>
                                  ) : (
                                    <span className="border-muted-foreground/30 size-3.5 rounded-full border" />
                                  )}
                                </span>
                                <span className="flex-1 truncate">
                                  {task.name}
                                </span>
                              </button>
                            )
                          })}
                        </div>
                      )}
                    </div>
                  )}
                </div>
              )
            })}
          </div>
        )}
      </div>

      {/* Footer with current selection and action */}
      {resolvedProjectId && resolvedTaskId !== NO_TASK_ID ? (
        <footer className="flex items-center justify-between border-t px-6 py-4">
          <div className="flex min-w-0 flex-1 items-center gap-2 text-sm">
            <CheckIcon className="text-primary size-4 shrink-0" />
            <span className="truncate font-medium">
              {projects.find((p) => p.id === resolvedProjectId)?.name}
              <span className="text-muted-foreground mx-1">›</span>
              {projects
                .find((p) => p.id === resolvedProjectId)
                ?.tasks.find((t) => t.id === resolvedTaskId)?.name ?? ""}
            </span>
          </div>
          <Button size="sm" onClick={onBack} className="shrink-0">
            {t("workspace.done")}
          </Button>
        </footer>
      ) : null}
    </div>
  )
}
