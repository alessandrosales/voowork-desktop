import { LogOutIcon, User2Icon } from "lucide-react"
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
        <span className="flex size-6 items-center justify-center cursor-pointer rounded-full dark:bg-gray-950 bg-gray-200 text-primary">
        <User2Icon className="size-4 text-foreground" />
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
