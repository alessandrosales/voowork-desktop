import { LogOutIcon } from "lucide-react"
import { useTranslation } from "react-i18next"

import type { AuthState } from "@/hooks/use-auth"
import { Button } from "@/components/ui/button"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import { cn } from "@/lib/utils"

function userInitials(name?: string | null, email?: string | null) {
  if (name) {
    const parts = name.trim().split(/\s+/).filter(Boolean)
    if (parts.length >= 2) {
      return `${parts[0]?.[0] ?? ""}${parts.at(-1)?.[0] ?? ""}`.toUpperCase()
    }
    if (parts[0]) {
      return parts[0].slice(0, 2).toUpperCase()
    }
  }

  if (email) {
    return email.slice(0, 2).toUpperCase()
  }

  return "??"
}

export function ProfileMenu({
  auth,
  loading,
  onLogout,
  className,
}: Readonly<{
  auth: AuthState
  loading?: boolean
  onLogout: () => Promise<void>
  className?: string
}>) {
  const { t } = useTranslation()
  const initials = userInitials(auth.user?.name, auth.user?.email)

  return (
    <DropdownMenu>
      <DropdownMenuTrigger
        render={
          <Button
            type="button"
            variant="ghost"
            size="icon-sm"
            className={cn("shrink-0 rounded-full p-0", className)}
            aria-label={t("profile.menuLabel")}
            disabled={loading}
          />
        }
      >
        <span className="bg-primary/15 text-primary flex size-8 items-center justify-center rounded-full text-xs font-semibold">
          {initials}
        </span>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="z-[200] min-w-56">
        <DropdownMenuGroup>
          <DropdownMenuLabel className="flex flex-col gap-0.5 py-2">
            <span className="text-foreground text-sm font-medium">
              {auth.user?.name ?? t("common.hello")}
            </span>
            {auth.user?.email ? (
              <span className="text-muted-foreground truncate text-xs font-normal">
                {auth.user.email}
              </span>
            ) : null}
            {auth.organization?.name ? (
              <span className="text-muted-foreground truncate text-xs font-normal">
                {auth.organization.name}
              </span>
            ) : null}
          </DropdownMenuLabel>
        </DropdownMenuGroup>
        <DropdownMenuSeparator />
        <DropdownMenuGroup>
          <DropdownMenuItem
            variant="destructive"
            onClick={() => {
              void onLogout()
            }}
          >
            <LogOutIcon />
            {t("profile.logout")}
          </DropdownMenuItem>
        </DropdownMenuGroup>
      </DropdownMenuContent>
    </DropdownMenu>
  )
}
