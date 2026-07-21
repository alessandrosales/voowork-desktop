import { useCallback, useEffect, useRef, useState } from "react"

import type { TrackingStatus } from "@/hooks/use-tracking-session"

const BILLABLE_PHASES = new Set(["active", "warning", "countdown"])
const FROZEN_PHASES = new Set(["manual_paused", "manual_work_check"])
const IDLE_FROZEN_PHASES = new Set(["paused_inactivity", "resume_prompt"])

function isFrozenPhase(phase: string) {
  return FROZEN_PHASES.has(phase) || IDLE_FROZEN_PHASES.has(phase)
}

function elapsedFromAnchor(seconds: number, anchoredAt: number) {
  return seconds + Math.floor((Date.now() - anchoredAt) / 1000)
}

type ElapsedAnchor = {
  seconds: number
  at: number
}

export function useDisplayElapsed(tracking: TrackingStatus) {
  const [elapsedAnchor, setElapsedAnchor] = useState<ElapsedAnchor>(() => ({
    seconds: 0,
    at: Date.now(),
  }))
  const frozenElapsedRef = useRef<number | null>(null)
  const pauseIntentRef = useRef(false)
  const prevPhaseRef = useRef(tracking.inactivity.phase)
  const [displayElapsedSeconds, setDisplayElapsedSeconds] = useState(0)

  const freezeDisplayElapsed = useCallback(() => {
    if (!tracking.active) {
      return
    }

    pauseIntentRef.current = true

    const phase = tracking.inactivity.phase
    const frozen = BILLABLE_PHASES.has(phase)
      ? elapsedFromAnchor(elapsedAnchor.seconds, elapsedAnchor.at)
      : tracking.elapsedSeconds

    frozenElapsedRef.current = frozen
    setDisplayElapsedSeconds(frozen)
  }, [
    elapsedAnchor.at,
    elapsedAnchor.seconds,
    tracking.active,
    tracking.elapsedSeconds,
    tracking.inactivity.phase,
  ])

  /**
   * Desfaz o congelamento otimista feito por freezeDisplayElapsed() quando o
   * pause falha (A11). Sem isso, o relógio ficaria travado num valor obsoleto
   * enquanto o tracking continua correndo no backend.
   */
  const cancelPauseFreeze = useCallback(() => {
    pauseIntentRef.current = false
    frozenElapsedRef.current = null
    if (tracking.active && BILLABLE_PHASES.has(tracking.inactivity.phase)) {
      setElapsedAnchor({ seconds: tracking.elapsedSeconds, at: Date.now() })
    }
  }, [tracking.active, tracking.elapsedSeconds, tracking.inactivity.phase])

  useEffect(() => {
    if (!tracking.active) {
      pauseIntentRef.current = false
      prevPhaseRef.current = "active"
      frozenElapsedRef.current = null
      queueMicrotask(() => setDisplayElapsedSeconds(tracking.elapsedSeconds))
      return
    }

    const phase = tracking.inactivity.phase
    const prevPhase = prevPhaseRef.current
    prevPhaseRef.current = phase

    if (isFrozenPhase(phase)) {
      pauseIntentRef.current = false
      frozenElapsedRef.current = tracking.elapsedSeconds
      return
    }

    if (pauseIntentRef.current) {
      return
    }

    if (
      (FROZEN_PHASES.has(prevPhase) || IDLE_FROZEN_PHASES.has(prevPhase)) &&
      BILLABLE_PHASES.has(phase)
    ) {
      frozenElapsedRef.current = null
      setElapsedAnchor({ seconds: tracking.elapsedSeconds, at: Date.now() })
      return
    }

    if (BILLABLE_PHASES.has(phase) && frozenElapsedRef.current === null) {
      setElapsedAnchor({ seconds: tracking.elapsedSeconds, at: Date.now() })
    }
  }, [tracking.active, tracking.elapsedSeconds, tracking.inactivity.phase])

  useEffect(() => {
    const tick = () => {
      if (!tracking.active) {
        setDisplayElapsedSeconds(tracking.elapsedSeconds)
        return
      }

      const phase = tracking.inactivity.phase
      const frozen = frozenElapsedRef.current

      if (
        pauseIntentRef.current ||
        frozen !== null ||
        isFrozenPhase(phase)
      ) {
        setDisplayElapsedSeconds(frozen ?? tracking.elapsedSeconds)
        return
      }

      if (BILLABLE_PHASES.has(phase)) {
        setDisplayElapsedSeconds(
          elapsedFromAnchor(elapsedAnchor.seconds, elapsedAnchor.at)
        )
        return
      }

      setDisplayElapsedSeconds(tracking.elapsedSeconds)
    }

    tick()
    const interval = window.setInterval(tick, 1000)
    return () => window.clearInterval(interval)
  }, [
    elapsedAnchor.at,
    elapsedAnchor.seconds,
    tracking.active,
    tracking.elapsedSeconds,
    tracking.inactivity.phase,
  ])

  return { displayElapsedSeconds, freezeDisplayElapsed, cancelPauseFreeze }
}
