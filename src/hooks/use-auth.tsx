import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
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
  loading: boolean
  error: string | null
  login: (email: string, password: string) => Promise<AuthState>
  logout: () => Promise<void>
}

const AuthContext = createContext<AuthContextValue | null>(null)

function applyAuthState(state: AuthState): AuthState {
  return {
    isAuthenticated: state.isAuthenticated,
    user: state.user ?? null,
    organization: state.organization ?? null,
  }
}

export function AuthProvider({ children }: Readonly<{ children: ReactNode }>) {
  const [auth, setAuth] = useState<AuthState>(EMPTY_AUTH)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const bootstrappedRef = useRef(false)

  useEffect(() => {
    if (!isTauriReady()) {
      setLoading(false)
      return
    }

    if (bootstrappedRef.current) {
      return
    }
    bootstrappedRef.current = true

    const bootstrap = async () => {
      try {
        const state = await trackedInvoke<AuthState>("validate_auth_session")
        setAuth(applyAuthState(state))
        setError(null)
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err))
      } finally {
        setLoading(false)
      }
    }

    void bootstrap()
  }, [])

  useEffect(() => {
    if (!isTauriReady()) {
      return
    }

    let unlisten: (() => void) | undefined
    listen("auth-session-expired", () => {
      setAuth(EMPTY_AUTH)
      setError(null)
      setLoading(false)
    })
      .then((dispose) => {
        unlisten = dispose
      })
      .catch(() => undefined)

    return () => {
      unlisten?.()
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
      loading,
      error,
      login,
      logout,
    }),
    [auth, loading, error, login, logout]
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
