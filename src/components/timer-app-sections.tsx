import { PauseIcon, PlayIcon } from "lucide-react"
import type { TFunction } from "i18next"

import { openWebPanel } from "@/lib/navigation"
import { AppMeta } from "@/components/app-meta"
import { Button } from "@/components/ui/button"
import { cn } from "@/lib/utils"

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
        "voowork-timer-play-surface absolute inset-0 z-10 m-auto flex size-[82%] max-w-[180px] items-center justify-center rounded-full transition-transform",
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
