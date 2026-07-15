import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react"
import { listen } from "@tauri-apps/api/event"

import { trackedInvoke, isTauriReady } from "@/lib/tauri"

export type AuthUser = {
  id: string
  name: string
  email: string
}

export type AuthOrganization = {
  id: string
  name: string
}

export type AuthState = {
  isAuthenticated: boolean
  user: AuthUser | null
  organization: AuthOrganization | null
}

const EMPTY_AUTH: AuthState = {
  isAuthenticated: false,
  user: null,
  organization: null,
}

type AuthContextValue = {
  auth: AuthState
  /** True only while validating the stored session on app start. */
  initializing: boolean
  /** True while login or logout is in progress. */
  loading: boolean
  error: string | null
  login: (email: string, password: string) => Promise<AuthState>
  logout: () => Promise<void>
}

const AuthContext = createContext<AuthContextValue | null>(null)

const AUTH_BOOTSTRAP_TIMEOUT_MS = 15_000

let authBootstrapPromise: Promise<AuthState> | null = null

function bootstrapAuthSession(): Promise<AuthState> {
  authBootstrapPromise ??= trackedInvoke<AuthState>("validate_auth_session")
  return authBootstrapPromise
}

function bootstrapAuthSessionWithTimeout(): Promise<AuthState> {
  return Promise.race([
    bootstrapAuthSession(),
    new Promise<AuthState>((_, reject) => {
      window.setTimeout(
        () => reject(new Error("Tempo esgotado ao validar sessão.")),
        AUTH_BOOTSTRAP_TIMEOUT_MS
      )
    }),
  ])
}

function applyAuthState(state: AuthState): AuthState {
  return {
    isAuthenticated: state.isAuthenticated,
    user: state.user ?? null,
    organization: state.organization ?? null,
  }
}

export function AuthProvider({ children }: Readonly<{ children: ReactNode }>) {
  const [auth, setAuth] = useState<AuthState>(EMPTY_AUTH)
  const [initializing, setInitializing] = useState(true)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    if (!isTauriReady()) {
      setInitializing(false)
      return
    }

    let active = true

    const bootstrap = async () => {
      try {
        const state = await bootstrapAuthSessionWithTimeout()
        if (!active) return
        setAuth(applyAuthState(state))
        setError(null)
      } catch (err) {
        if (!active) return
        setAuth(EMPTY_AUTH)
        setError(err instanceof Error ? err.message : String(err))
      } finally {
        if (active) {
          setInitializing(false)
        }
      }
    }

    void bootstrap()

    return () => {
      active = false
    }
  }, [])

  useEffect(() => {
    if (!isTauriReady()) {
      return
    }

    let unlistenExpired: (() => void) | undefined
    let unlistenLoggedOut: (() => void) | undefined

    const clearAuth = () => {
      setAuth(EMPTY_AUTH)
      setError(null)
      setLoading(false)
    }

    listen("auth-session-expired", clearAuth)
      .then((dispose) => {
        unlistenExpired = dispose
      })
      .catch(() => undefined)

    listen("auth-logged-out", clearAuth)
      .then((dispose) => {
        unlistenLoggedOut = dispose
      })
      .catch(() => undefined)

    return () => {
      unlistenExpired?.()
      unlistenLoggedOut?.()
    }
  }, [])

  const login = useCallback(async (email: string, password: string) => {
    setLoading(true)
    setError(null)
    try {
      const state = await trackedInvoke<AuthState>("login", {
        request: { email, password },
      })
      const nextAuth = applyAuthState(state)
      setAuth(nextAuth)
      return nextAuth
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err)
      setError(message)
      throw err
    } finally {
      setLoading(false)
    }
  }, [])

  const logout = useCallback(async () => {
    setLoading(true)
    try {
      await trackedInvoke("logout")
      setAuth(EMPTY_AUTH)
      setError(null)
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setLoading(false)
    }
  }, [])

  const value = useMemo(
    () => ({
      auth,
      initializing,
      loading,
      error,
      login,
      logout,
    }),
    [auth, initializing, loading, error, login, logout]
  )

  return <AuthContext.Provider value={value}>{children}</AuthContext.Provider>
}

export function useAuth() {
  const context = useContext(AuthContext)
  if (!context) {
    throw new Error("useAuth must be used within AuthProvider")
  }
  return context
}
