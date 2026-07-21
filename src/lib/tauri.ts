import { invoke, isTauri, type InvokeArgs } from "@tauri-apps/api/core"

export function isTauriReady(): boolean {
  if (!isTauri() || typeof window === "undefined") {
    return false
  }

  const internals = (
    window as Window & {
      __TAURI_INTERNALS__?: { invoke?: unknown }
    }
  ).__TAURI_INTERNALS__

  return internals?.invoke !== undefined
}

export async function waitForTauriReady(maxAttempts = 30): Promise<boolean> {
  for (let attempt = 0; attempt < maxAttempts; attempt += 1) {
    if (isTauriReady()) {
      return true
    }
    await new Promise((resolve) => setTimeout(resolve, 100))
  }
  return false
}

export async function trackedInvoke<T>(command: string, args?: InvokeArgs) {
  if (!isTauriReady()) {
    throw new Error(`IPC not available for command: ${command}`)
  }

  return invoke<T>(command, args)
}

export { isTauri }
