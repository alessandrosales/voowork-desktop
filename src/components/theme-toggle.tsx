import { useTranslation } from "react-i18next"

import { useTheme } from "@/components/theme-provider"
import { Button } from "@/components/ui/button"
import { cn } from "@/lib/utils"
import { MoonIcon, SunIcon } from "lucide-react"

export function ThemeToggle({
  className,
}: Readonly<{ className?: string }>) {
  const { t } = useTranslation()
  const { theme, setTheme } = useTheme()

  return (
    <Button
      type="button"
      variant="ghost"
      size="icon-sm"
      className={cn("shrink-0 rounded-full", className)}
      onClick={() => setTheme(theme === "dark" ? "light" : "dark")}
      aria-label={
        theme === "dark"
          ? t("theme.switchToLight")
          : t("theme.switchToDark")
      }
    >
      {theme === "dark" ? (
        <SunIcon className="size-4" />
      ) : (
        <MoonIcon className="size-4" />
      )}
    </Button>
  )
}
