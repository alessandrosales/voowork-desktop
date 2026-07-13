import { LanguagesIcon } from "lucide-react"
import { useTranslation } from "react-i18next"

import {
  type AppLocale,
  SUPPORTED_LOCALES,
  changeAppLocale,
} from "@/i18n"
import { Button } from "@/components/ui/button"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import { cn } from "@/lib/utils"

export function LanguageToggle({
  className,
}: Readonly<{ className?: string }>) {
  const { i18n, t } = useTranslation()
  const currentLocale = i18n.language as AppLocale

  return (
    <DropdownMenu>
      <DropdownMenuTrigger
        render={
          <Button
            type="button"
            variant="ghost"
            size="icon-sm"
            className={cn("shrink-0 rounded-full", className)}
            aria-label={t("language.label")}
          />
        }
      >
        <LanguagesIcon className="size-4" />
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="z-[200] min-w-36">
        <DropdownMenuRadioGroup
          value={currentLocale}
          onValueChange={(value) => {
            const locale = value as AppLocale
            if (SUPPORTED_LOCALES.includes(locale)) {
              changeAppLocale(locale).catch(() => undefined)
            }
          }}
        >
          {SUPPORTED_LOCALES.map((locale) => (
            <DropdownMenuRadioItem key={locale} value={locale}>
              {t(`language.${locale}`)}
            </DropdownMenuRadioItem>
          ))}
        </DropdownMenuRadioGroup>
      </DropdownMenuContent>
    </DropdownMenu>
  )
}
