import { useState, type SyntheticEvent } from "react"
import { ExternalLinkIcon, LoaderIcon, LogInIcon } from "lucide-react"
import { openUrl } from "@tauri-apps/plugin-opener"
import { useTranslation } from "react-i18next"

import { AppMeta } from "@/components/app-meta"
import { VooworkLogo } from "@/components/voowork-logo"
import { LanguageToggle } from "@/components/language-toggle"
import { ThemeToggle } from "@/components/theme-toggle"
import { useAuth } from "@/hooks/use-auth"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"

export function CompactLogin() {
  const { t } = useTranslation()
  const { login, loading, error } = useAuth()
  const [email, setEmail] = useState("admin@admin.com")
  const [password, setPassword] = useState("12345678")

  const handleSubmit = async (event: SyntheticEvent<HTMLFormElement>) => {
    event.preventDefault()
    await login(email, password)
  }

  return (
    <div className="voowork-shell flex h-full min-h-0 flex-col">
      <div className="mx-auto flex w-full max-w-lg flex-1 flex-col px-6 py-6">
        <div className="relative flex flex-col items-center pt-2 text-center">
          <div className="absolute top-0 right-0 flex items-center gap-1">
            <LanguageToggle />
            <ThemeToggle />
          </div>
          <VooworkLogo size="lg" className="mb-4" />
          <p className="text-muted-foreground mt-1 max-w-[280px] text-xs leading-relaxed">
            {t("auth.tagline")}
          </p>
        </div>

        <form
          className="mt-8 flex min-h-0 flex-1 flex-col"
          onSubmit={(event) => {
            handleSubmit(event).catch(() => undefined)
          }}
        >
          <div className="flex flex-1 flex-col justify-center gap-3">
            <div className="grid gap-1.5">
              <Label htmlFor="email" className="text-xs">
                {t("auth.email")}
              </Label>
              <Input
                id="email"
                type="email"
                value={email}
                onChange={(e) => setEmail(e.target.value)}
                placeholder={t("auth.emailPlaceholder")}
                autoComplete="email"
                disabled={loading}
                className="h-10 rounded-xl"
              />
            </div>
            <div className="grid gap-1.5">
              <Label htmlFor="password" className="text-xs">
                {t("auth.password")}
              </Label>
              <Input
                id="password"
                type="password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                autoComplete="current-password"
                disabled={loading}
                className="h-10 rounded-xl"
              />
            </div>

            {error ? <p className="text-destructive text-xs">{error}</p> : null}
          </div>

          <Button
            type="submit"
            className="voowork-start-btn mt-4 h-11 shrink-0 rounded-2xl font-semibold"
            disabled={loading}
          >
          {loading ? (
            <LoaderIcon className="size-4 animate-spin" />
          ) : (
            <LogInIcon className="size-4" />
          )}
          {t("auth.signIn")}
        </Button>
        </form>

        <footer className="mt-auto border-t pt-4 pb-5">
          <Button
            type="button"
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
      </div>
    </div>
  )
}
