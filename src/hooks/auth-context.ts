import { createContext, useContext } from "react"

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

export type AuthContextValue = {
  auth: AuthState
  initializing: boolean
  loading: boolean
  error: string | null
  login: (email: string, password: string) => Promise<AuthState>
  logout: () => Promise<void>
}

export const AuthContext = createContext<AuthContextValue | null>(null)

export function useAuth() {
  const context = useContext(AuthContext)
  if (!context) {
    throw new Error("useAuth must be used within AuthProvider")
  }
  return context
}
