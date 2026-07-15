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

export async function trackedInvoke<T>(command: string, args?: InvokeArgs) {
  if (!isTauriReady()) {
    throw new Error(`Tauri IPC indisponível para o comando: ${command}`)
  }

  return invoke<T>(command, args)
}

export { isTauri }
