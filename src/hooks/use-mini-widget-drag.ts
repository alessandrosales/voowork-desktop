import { useCallback, useRef } from "react"
import { invoke } from "@tauri-apps/api/core"

const DRAG_THRESHOLD_PX = 4

async function beginMiniWidgetDrag() {
  await invoke("begin_mini_widget_drag")
}

export function useMiniWidgetDrag(immediate = false) {
  const dragStateRef = useRef<{
    pointerId: number
    startX: number
    startY: number
    target: HTMLElement
    onPointerMove: (event: PointerEvent) => void
    onPointerUp: () => void
  } | null>(null)

  const cleanupDragListeners = useCallback(() => {
    const dragState = dragStateRef.current
    if (!dragState) {
      return
    }

    const { target, onPointerMove, onPointerUp } = dragState
    target.removeEventListener("pointermove", onPointerMove)
    target.removeEventListener("pointerup", onPointerUp)
    target.removeEventListener("pointercancel", onPointerUp)
    dragStateRef.current = null
  }, [])

  return useCallback(
    (event: React.PointerEvent<HTMLElement>) => {
      if (event.button !== 0) {
        return
      }

      event.preventDefault()

      if (immediate) {
        beginMiniWidgetDrag().catch(() => undefined)
        return
      }

      cleanupDragListeners()
      const target = event.currentTarget
      const startX = event.clientX
      const startY = event.clientY
      const pointerId = event.pointerId

      const onPointerMove = (moveEvent: PointerEvent) => {
        if (moveEvent.pointerId !== pointerId) {
          return
        }

        const dx = moveEvent.clientX - startX
        const dy = moveEvent.clientY - startY
        if (Math.hypot(dx, dy) > DRAG_THRESHOLD_PX) {
          cleanupDragListeners()
          beginMiniWidgetDrag().catch(() => undefined)
        }
      }

      const onPointerUp = () => {
        cleanupDragListeners()
      }

      dragStateRef.current = {
        pointerId,
        startX,
        startY,
        target,
        onPointerMove,
        onPointerUp,
      }
      target.addEventListener("pointermove", onPointerMove)
      target.addEventListener("pointerup", onPointerUp)
      target.addEventListener("pointercancel", onPointerUp)
    },
    [cleanupDragListeners, immediate]
  )
}
