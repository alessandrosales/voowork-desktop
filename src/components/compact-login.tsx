import { useState, type SyntheticEvent } from "react"
import { ExternalLinkIcon, LoaderIcon } from "lucide-react"
import { openWebPanel } from "@/lib/navigation"
import { useTranslation } from "react-i18next"

import { AppMeta } from "@/components/app-meta"
import { VooworkLogo } from "@/components/voowork-logo"
import { LanguageToggle } from "@/components/language-toggle"
import { ThemeToggle } from "@/components/theme-toggle"
import { useAuth } from "@/hooks/use-auth"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"

export function CompactLogin() {
  const { t } = useTranslation()
  const { login, loading, error } = useAuth()
  const [email, setEmail] = useState("")
  const [password, setPassword] = useState("")

  const handleSubmit = async (event: SyntheticEvent<HTMLFormElement>) => {
    event.preventDefault()
    await login(email, password)
  }

  return (
    <div className="voowork-shell flex h-full min-h-0 flex-col">
      <div className="mx-auto flex h-full w-full md:min-w-md flex-col px-12">
        <div className="flex min-h-0 flex-1 flex-col items-center justify-center pb-4">
          <div className="flex w-full flex-col items-center">
            <VooworkLogo size="lg" className="mb-6" />
            <h1 className="text-center text-lg font-semibold tracking-tight">
              {t("auth.tagline")}
            </h1>

            <form
              className="mx-auto mt-10 flex w-full max-w-sm flex-col gap-4"
              onSubmit={(event) => {
                handleSubmit(event).catch(() => undefined)
              }}
            >
              <Input
                id="email"
                type="email"
                value={email}
                onChange={(e) => setEmail(e.target.value)}
                placeholder={t("auth.emailPlaceholder")}
                autoComplete="email"
                aria-label={t("auth.email")}
                disabled={loading}
                className="h-11 rounded-xl bg-background shadow-sm"
              />
              <Input
                id="password"
                type="password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                placeholder={t("auth.password")}
                autoComplete="current-password"
                aria-label={t("auth.password")}
                disabled={loading}
                className="h-11 rounded-xl bg-background shadow-sm"
              />

              {error ? (
                <p className="text-destructive text-center text-xs">{error}</p>
              ) : null}

              <Button
                type="submit"
                className="voowork-start-btn mt-2 h-11 w-full rounded-xl font-semibold shadow-sm"
                disabled={loading}
              >
                {loading ? (
                  <LoaderIcon className="size-4 animate-spin" />
                ) : (
                  t("auth.signIn")
                )}
              </Button>
            </form>
          </div>
        </div>

        <footer className="shrink-0 space-y-4 pb-6 pt-2">
          <div className="flex items-center justify-center gap-1.5">
            <LanguageToggle />
            <ThemeToggle />
          </div>

          <Button
            type="button"
            variant="ghost"
            size="sm"
            className="text-muted-foreground h-9 w-full justify-center gap-2 text-xs"
            disabled={loading}
            onClick={() => {
              openWebPanel().catch(() => undefined)
            }}
          >
            <ExternalLinkIcon className="size-3.5" />
            {t("timer.openWebPanel")}
          </Button>

          <AppMeta />
        </footer>
      </div>
    </div>
  )
}
