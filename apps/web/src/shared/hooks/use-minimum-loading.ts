import { useEffect, useRef, useState } from "react"

export const DEFAULT_MINIMUM_LOADING_MS = 1_000

export function minimumLoadingRemaining(
  startedAt: number,
  endedAt: number,
  minimumMs = DEFAULT_MINIMUM_LOADING_MS,
) {
  return Math.max(0, minimumMs - Math.max(0, endedAt - startedAt))
}

/**
 * Keeps an initial loading frame visible long enough to avoid a brief flash.
 * Pass only no-data loading states; background refresh and mutation progress
 * should remain immediate.
 */
export function useMinimumLoading(
  loading: boolean,
  minimumMs = DEFAULT_MINIMUM_LOADING_MS,
) {
  const startedAt = useRef<number | null>(null)
  const wasLoading = useRef(false)
  const timeout = useRef<number | null>(null)
  const [held, setHeld] = useState(loading)

  useEffect(() => {
    const now = performance.now()

    if (loading) {
      if (!wasLoading.current || startedAt.current === null) {
        if (timeout.current !== null) {
          window.clearTimeout(timeout.current)
          timeout.current = null
        }
        startedAt.current = now
      }
      wasLoading.current = true
      setHeld(true)
      return
    }

    wasLoading.current = false
    const cycleStartedAt = startedAt.current
    if (cycleStartedAt === null) {
      setHeld(false)
      return
    }

    const remaining = minimumLoadingRemaining(cycleStartedAt, now, minimumMs)
    if (timeout.current !== null) window.clearTimeout(timeout.current)
    if (remaining === 0) {
      timeout.current = null
      startedAt.current = null
      setHeld(false)
      return
    }

    timeout.current = window.setTimeout(() => {
      timeout.current = null
      startedAt.current = null
      setHeld(false)
    }, remaining)
  }, [loading, minimumMs])

  useEffect(
    () => () => {
      if (timeout.current !== null) window.clearTimeout(timeout.current)
      timeout.current = null
      startedAt.current = null
      wasLoading.current = false
    },
    [],
  )

  return loading || held
}
