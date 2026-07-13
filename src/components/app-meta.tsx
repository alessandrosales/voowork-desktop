import { cn } from "@/lib/utils"

const appVersion = import.meta.env.VITE_APP_VERSION?.trim() || null

export function AppMeta({ className }: Readonly<{ className?: string }>) {
  const year = new Date().getFullYear()

  return (
    <p
      className={cn(
        "text-muted-foreground text-center text-[10px] leading-relaxed",
        className
      )}
    >
      <span>© {year} Voowork</span>
      {appVersion ? (
        <>
          <span aria-hidden="true" className="mx-1.5">
            ·
          </span>
          <span>v{appVersion}</span>
        </>
      ) : null}
    </p>
  )
}
