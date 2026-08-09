import { useEffect, useState } from "react"
import { useNavigate } from "@tanstack/react-router"

import { api } from "@/shared/api/client"
import { BrandMark } from "@/shared/components/brand"
import { useAuthStore } from "@/shared/stores/auth"
import { authSession } from "@/shared/lib/auth-session"
import { ErrorState } from "@/shared/ui/empty-state"
import { Spinner } from "@/shared/ui/spinner"

export function SsoCallbackPage() {
  const navigate = useNavigate()
  const setSession = useAuthStore((s) => s.setSession)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    void (async () => {
      try {
        const params = new URLSearchParams(window.location.hash.slice(1))
        const token = params.get("token")
        window.history.replaceState(null, "", window.location.pathname)
        if (!token) {
          setError("Missing SSO token")
          return
        }
        authSession.setToken(token)
        const user = await api.me()
        setSession(token, user)
        navigate({ to: "/app" })
      } catch (e) {
        setError(e instanceof Error ? e.message : "SSO callback failed")
      }
    })()
  }, [navigate, setSession])

  return (
    <div className="flex min-h-dvh flex-col items-center justify-center gap-4 px-6">
      <BrandMark size="lg" />
      {error ? (
        <ErrorState message={error} className="max-w-md" />
      ) : (
        <p className="inline-flex items-center gap-2 text-sm text-(--color-text-muted)">
          <Spinner />
          Completing Microsoft / SSO sign-in…
        </p>
      )}
    </div>
  )
}
