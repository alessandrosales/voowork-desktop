import type { TFunction } from "i18next"

export function idlePhaseLabel(phase: string, t: TFunction) {
  switch (phase) {
    case "warning":
    case "countdown":
      return t("idle.phase.warning")
    case "paused_inactivity":
    case "manual_paused":
    case "manual_work_check":
      return t("idle.phase.paused")
    case "resume_prompt":
      return t("idle.phase.resume")
    default:
      return t("idle.phase.active")
  }
}

export function idlePhaseClassName(phase: string) {
  switch (phase) {
    case "warning":
    case "countdown":
      return "voowork-live-pill--warning"
    case "paused_inactivity":
    case "manual_paused":
    case "manual_work_check":
      return "voowork-live-pill--paused"
    case "resume_prompt":
      return "voowork-live-pill--resume"
    default:
      return ""
  }
}

export const trackingInactivityPhaseLabel = idlePhaseLabel
export const trackingInactivityPhaseClassName = idlePhaseClassName

export function timerRingClassName(phase: string, active: boolean) {
  if (!active) {
    return ""
  }
  switch (phase) {
    case "warning":
    case "countdown":
      return "voowork-timer-ring--warning"
    case "paused_inactivity":
    case "manual_paused":
    case "manual_work_check":
    case "resume_prompt":
      return "voowork-timer-ring--paused"
    default:
      return "voowork-timer-ring--active"
  }
}
