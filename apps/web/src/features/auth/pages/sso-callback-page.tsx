import { useEffect, useState } from "react"
import { useNavigate } from "@tanstack/react-router"

import { api } from "@/shared/api/client"
import { BrandMark } from "@/shared/components/brand"
import { useAuthStore } from "@/shared/stores/auth"

export function SsoCallbackPage() {
  const navigate = useNavigate()
  const setSession = useAuthStore((s) => s.setSession)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    void (async () => {
      try {
        const params = new URLSearchParams(window.location.search)
        const token = params.get("token")
        if (!token) {
          setError("Missing SSO token")
          return
        }
        localStorage.setItem("conductor.token", token)
        const user = await api.me()
        setSession(token, user)
        navigate({ to: "/app" })
      } catch (e) {
        setError(e instanceof Error ? e.message : "SSO callback failed")
      }
    })()
  }, [navigate, setSession])

  return (
    <div className="flex min-h-screen flex-col items-center justify-center gap-4 px-6">
      <BrandMark />
      <p className="text-sm text-(--color-text-muted)">
        {error ?? "Completing Microsoft / SSO sign-in…"}
      </p>
    </div>
  )
}
