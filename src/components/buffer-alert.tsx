import { AlertTriangleIcon } from "lucide-react"
import { useTranslation } from "react-i18next"

import { Button } from "@/components/ui/button"

type BufferAlertProps = Readonly<{
  bufferSeconds: number
  loading?: boolean
  onDismiss: () => void | Promise<void>
  onStart: () => void | Promise<void>
}>

function formatBufferMinutes(seconds: number) {
  const minutes = Math.max(1, Math.round(seconds / 60))
  return minutes
}

export function BufferAlert({
  bufferSeconds,
  loading = false,
  onDismiss,
  onStart,
}: BufferAlertProps) {
  const { t } = useTranslation()

  return (
    <div
      className="fixed inset-0 z-[9998] flex items-center justify-center p-4"
      role="alertdialog"
      aria-modal="true"
      aria-labelledby="buffer-alert-title"
    >
      <div className="absolute inset-0 bg-black/70 backdrop-blur-sm" />
      <div className="relative w-full max-w-md rounded-2xl border border-primary/30 bg-card p-6 shadow-2xl">
        <div className="flex items-start gap-3">
          <div className="flex size-10 shrink-0 items-center justify-center rounded-full bg-primary/15">
            <AlertTriangleIcon className="text-primary size-5" />
          </div>
          <div className="space-y-4">
            <div>
              <p id="buffer-alert-title" className="text-base font-semibold">
                {t("buffer.title")}
              </p>
              <p className="text-muted-foreground mt-1 text-sm leading-relaxed">
                {t("buffer.description", {
                  minutes: formatBufferMinutes(bufferSeconds),
                })}
              </p>
            </div>
            <div className="flex flex-col gap-2">
              <Button
                size="lg"
                className="h-11 w-full rounded-xl"
                disabled={loading}
                onClick={() => {
                  Promise.resolve(onStart()).catch(() => undefined)
                }}
              >
                {t("buffer.startNow")}
              </Button>
              <Button
                size="lg"
                variant="outline"
                className="h-11 w-full rounded-xl"
                disabled={loading}
                onClick={() => {
                  Promise.resolve(onDismiss()).catch(() => undefined)
                }}
              >
                {t("buffer.dismiss")}
              </Button>
            </div>
          </div>
        </div>
      </div>
    </div>
  )
}
