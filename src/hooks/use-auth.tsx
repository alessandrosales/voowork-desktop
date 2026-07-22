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

import { trackedInvoke, waitForTauriReady } from "@/lib/tauri"

export type AuthUser = {
  id: string
  name: string
  email: string
  profile: string
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
  initializing: boolean
  loading: boolean
  error: string | null
  login: (email: string, password: string) => Promise<AuthState>
  logout: () => Promise<void>
}

const AuthContext = createContext<AuthContextValue | null>(null)

const AUTH_BOOTSTRAP_SAFETY_TIMEOUT_MS = 45_000

type RawAuthState = AuthState & {
  is_authenticated?: boolean
}

function normalizeAuthState(state: RawAuthState): AuthState {
  return {
    isAuthenticated: Boolean(state.isAuthenticated ?? state.is_authenticated),
    user: state.user ?? null,
    organization: state.organization ?? null,
  }
}

function requestAuthValidation(): Promise<AuthState> {
  return trackedInvoke<RawAuthState>("validate_auth_session").then(
    normalizeAuthState
  )
}

function useInitializingSafetyTimeout(setInitializing: (v: boolean) => void) {
  useEffect(() => {
    const timer = window.setTimeout(
      () => setInitializing(false),
      AUTH_BOOTSTRAP_SAFETY_TIMEOUT_MS,
    )
    return () => window.clearTimeout(timer)
  }, [setInitializing])
}

export function AuthProvider({ children }: Readonly<{ children: ReactNode }>) {
  const [auth, setAuth] = useState<AuthState>(EMPTY_AUTH)
  const [initializing, setInitializing] = useState(true)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  useInitializingSafetyTimeout(setInitializing)

  useEffect(() => {
    let cancelled = false
    let unlistenExpired: (() => void) | undefined
    let unlistenLoggedOut: (() => void) | undefined

    const clearAuth = () => {
      setAuth(EMPTY_AUTH)
      setError(null)
      setLoading(false)
      setInitializing(false)
    }

    const setupListeners = async () => {
      const ready = await waitForTauriReady()
      if (!ready || cancelled) {
        return
      }

      listen("auth-session-expired", clearAuth)
        .then((dispose) => {
          if (cancelled) {
            dispose()
            return
          }
          unlistenExpired = dispose
        })
        .catch(() => undefined)

      listen("auth-logged-out", clearAuth)
        .then((dispose) => {
          if (cancelled) {
            dispose()
            return
          }
          unlistenLoggedOut = dispose
        })
        .catch(() => undefined)
    }

    const bootstrap = async () => {
      const ready = await waitForTauriReady()
      if (!ready || cancelled) {
        if (!cancelled) {
          setInitializing(false)
        }
        return
      }

      const listenerPromise = setupListeners()

      try {
        const localState = normalizeAuthState(
          await trackedInvoke<RawAuthState>("get_auth_state"),
        )

        await listenerPromise

        if (!localState.isAuthenticated) {
          if (!cancelled) {
            setAuth(EMPTY_AUTH)
            setError(null)
          }
          return
        }

        const validated = await requestAuthValidation()
        if (!cancelled) {
          setAuth(validated)
          setError(null)
        }
      } catch (err) {
        if (!cancelled) {
          setAuth(EMPTY_AUTH)
          setError(err instanceof Error ? err.message : String(err))
        }
      } finally {
        if (!cancelled) {
          setInitializing(false)
        }
      }
    }

    void bootstrap()

    return () => {
      cancelled = true
      unlistenExpired?.()
      unlistenLoggedOut?.()
    }
  }, [])

  const login = useCallback(async (email: string, password: string) => {
    setLoading(true)
    setError(null)
    try {
      const state = normalizeAuthState(
        await trackedInvoke<RawAuthState>("login", {
          request: { email, password },
        })
      )
      setAuth(state)
      return state
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
