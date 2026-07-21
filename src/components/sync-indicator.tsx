import { CloudIcon } from "lucide-react"
import { useTranslation } from "react-i18next"
import { useSyncStatus } from "@/hooks/use-sync-status"
import { cn } from "@/lib/utils"

interface SyncIndicatorProps {
  enabled: boolean
}

export function SyncIndicator({ enabled }: SyncIndicatorProps) {
  const { t } = useTranslation()
  const { pending, failed, confirmed } = useSyncStatus(enabled)

  const hasFailed = failed > 0
  const hasPending = pending > 0
  const hasData = confirmed > 0 || pending > 0 || failed > 0

  if (!hasData) return null

  const colorClass = hasFailed
    ? "text-destructive"
    : hasPending
      ? "text-amber-500"
      : "text-emerald-500"

  const label = hasFailed
    ? t("sync.failed", { count: failed })
    : hasPending
      ? t("sync.pending", { count: pending })
      : t("sync.ok")

  return (
    <span
      className={cn("inline-flex items-center gap-1 text-xs", colorClass)}
      title={label}
    >
      <CloudIcon className="size-3.5" />
      {hasFailed || hasPending ? (
        <span className="font-medium tabular-nums">
          {hasFailed ? failed : pending}
        </span>
      ) : null}
    </span>
  )
}
