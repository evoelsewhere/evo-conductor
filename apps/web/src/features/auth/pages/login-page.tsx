import { useState } from "react"
import { useNavigate } from "@tanstack/react-router"
import { motion } from "framer-motion"

import { api } from "@/shared/api/client"
import { BrandMark } from "@/shared/components/brand"
import { ThemeToggle } from "@/shared/components/theme-toggle"
import { useAuthStore } from "@/shared/stores/auth"
import { Button } from "@/shared/ui/button"
import { ErrorState } from "@/shared/ui/empty-state"
import { Input } from "@/shared/ui/input"
import { Label } from "@/shared/ui/label"
import { Spinner } from "@/shared/ui/spinner"
import { TooltipProvider } from "@/shared/ui/tooltip"

export function LoginPage({
  projectName,
  ssoEnabled,
}: {
  projectName?: string | null
  ssoEnabled?: boolean
}) {
  const navigate = useNavigate()
  const setSession = useAuthStore((s) => s.setSession)
  const [email, setEmail] = useState("")
  const [password, setPassword] = useState("")
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  const [ssoBusy, setSsoBusy] = useState(false)

  async function onSubmit(e: React.FormEvent) {
    e.preventDefault()
    setBusy(true)
    setError(null)
    try {
      const session = await api.login(email.trim(), password)
      setSession(session.token, session.user)
      if (session.user.must_change_password) {
        navigate({ to: "/change-password" })
      } else {
        navigate({ to: "/app" })
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "Login failed")
    } finally {
      setBusy(false)
    }
  }

  async function onSso() {
    setSsoBusy(true)
    setError(null)
    try {
      const { authorization_url } = await api.ssoStart()
      window.location.href = authorization_url
    } catch (err) {
      setError(err instanceof Error ? err.message : "SSO start failed")
      setSsoBusy(false)
    }
  }

  return (
    <TooltipProvider>
      <div className="relative flex min-h-dvh items-center justify-center px-4 py-10 sm:px-6">
        <div className="absolute top-4 right-4">
          <ThemeToggle />
        </div>

        <motion.div
          initial={{ opacity: 0, y: 10 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.28, ease: [0.16, 1, 0.3, 1] }}
          className="w-full max-w-md"
        >
          <BrandMark size="lg" className="mb-8" />
          <div className="rounded-xl border border-(--border-card) bg-(--bg-card)/85 p-5 shadow-(--shadow-depth) backdrop-blur-sm sm:p-6">
            <h1 className="text-xl font-semibold tracking-tight">Sign in</h1>
            <p className="mt-1 text-sm text-(--color-text-muted)">
              {projectName
                ? `Access ${projectName} Conductor`
                : "Access your project Conductor"}
            </p>

            <form className="mt-5 space-y-3" onSubmit={(e) => void onSubmit(e)}>
              <div className="space-y-1.5">
                <Label htmlFor="email">Email</Label>
                <Input
                  id="email"
                  type="email"
                  autoComplete="username"
                  value={email}
                  onChange={(e) => setEmail(e.target.value)}
                  required
                />
              </div>
              <div className="space-y-1.5">
                <Label htmlFor="password">Password</Label>
                <Input
                  id="password"
                  type="password"
                  autoComplete="current-password"
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  required
                />
              </div>

              {error && <ErrorState message={error} />}

              <Button
                type="submit"
                variant="gradient"
                className="w-full"
                disabled={busy || ssoBusy}
              >
                {busy ? (
                  <>
                    <Spinner className="text-current" />
                    Signing in…
                  </>
                ) : (
                  "Continue"
                )}
              </Button>
            </form>

            {ssoEnabled && (
              <div className="mt-4 border-t border-(--border-soft) pt-4">
                <Button
                  variant="outline"
                  className="w-full"
                  disabled={busy || ssoBusy}
                  onClick={() => void onSso()}
                >
                  {ssoBusy ? "Redirecting…" : "Continue with Microsoft / SSO"}
                </Button>
                <p className="mt-2 text-center text-[0.7rem] text-(--color-text-subtle)">
                  Microsoft Entra ID (Azure AD) and generic OIDC
                </p>
              </div>
            )}
          </div>
        </motion.div>
      </div>
    </TooltipProvider>
  )
}
