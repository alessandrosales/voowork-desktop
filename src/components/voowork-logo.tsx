import { useTheme } from "@/hooks/use-theme"
import { cn } from "@/lib/utils"

const LOGO_HEIGHTS = {
  xs: "h-6",
  compact: "h-7",
  sm: "h-8",
  md: "h-10",
  lg: "h-14",
} as const

export function VooworkLogo({
  className,
  size = "md",
}: Readonly<{
  className?: string
  size?: keyof typeof LOGO_HEIGHTS
}>) {
  const { theme } = useTheme()
  const src = theme === "dark" ? "/logo-dark.svg" : "/logo.svg"
  const height = LOGO_HEIGHTS[size]

  return (
    <img
      src={src}
      alt="Voowork"
      className={cn("w-auto shrink-0", height, className)}
      draggable={false}
    />
  )
}
