import { useEffect, useRef, useState } from "react"
import { trackedInvoke } from "@/lib/tauri"

const POLL_INTERVAL_MS = 10_000

export interface SyncStatus {
  pending: number
  failed: number
  confirmed: number
}

export function useSyncStatus(enabled: boolean): SyncStatus {
  const [status, setStatus] = useState<SyncStatus>({
    pending: 0,
    failed: 0,
    confirmed: 0,
  })
  const mountedRef = useRef(true)

  useEffect(() => {
    mountedRef.current = true

    const poll = async () => {
      if (!enabled) return
      try {
        const appStatus = await trackedInvoke<{
          syncPending: number
          syncFailed: number
          syncConfirmed: number
        }>("get_app_status")
        if (mountedRef.current) {
          setStatus({
            pending: appStatus.syncPending,
            failed: appStatus.syncFailed,
            confirmed: appStatus.syncConfirmed,
          })
        }
      } catch {
        // Silently ignore — IPC may not be ready or session may be gone
      }
    }

    void poll()
    const interval = window.setInterval(poll, POLL_INTERVAL_MS)

    return () => {
      mountedRef.current = false
      window.clearInterval(interval)
    }
  }, [enabled])

  return status
}
