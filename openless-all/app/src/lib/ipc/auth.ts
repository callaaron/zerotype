// Auth IPC bindings for ZeroType.
import { invokeOrMock } from "./shared"

export interface AuthUser {
  id: string
  email: string | null
  phone: string | null
  createdAt: string
  isAdmin: boolean
}

export interface AuthStatus {
  loggedIn: boolean
  user: AuthUser | null
}

export interface RegisterRequest {
  email?: string
  phone?: string
  password: string
}

export interface LoginRequest {
  credential: string
  password: string
}

export function register(req: RegisterRequest): Promise<AuthUser> {
  return invokeOrMock("register", req as unknown as Record<string, unknown>, () => ({
    id: "mock-user-1",
    email: req.email ?? null,
    phone: req.phone ?? null,
    createdAt: new Date().toISOString(),
    isAdmin: false,
  }))
}

export function login(req: LoginRequest): Promise<AuthUser> {
  return invokeOrMock("login", req as unknown as Record<string, unknown>, () => ({
    id: "mock-user-1",
    email: req.credential,
    phone: null,
    createdAt: new Date().toISOString(),
    isAdmin: true,
  }))
}

export function logout(token: string): Promise<void> {
  return invokeOrMock("logout", { token }, () => undefined)
}

export function validateSession(token: string): Promise<AuthUser> {
  return invokeOrMock("validate_session", { token }, () => ({
    id: "mock-user-1",
    email: "mock@zerotype.app",
    phone: null,
    createdAt: new Date().toISOString(),
    isAdmin: false,
  }))
}

export function getAuthStatus(): Promise<AuthStatus> {
  return invokeOrMock("get_auth_status", undefined, () => ({
    loggedIn: true,
    user: {
      id: "mock-user-1",
      email: "demo@zerotype.app",
      phone: null,
      createdAt: new Date().toISOString(),
      isAdmin: true,
    },
  }))
}

export function listUsers(): Promise<AuthUser[]> {
  return invokeOrMock("list_users", undefined, () => [
    {
      id: "mock-user-1",
      email: "demo@zerotype.app",
      phone: null,
      createdAt: new Date().toISOString(),
      isAdmin: true,
    },
  ])
}
