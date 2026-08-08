import { useState } from "react"
import { useNavigate } from "@tanstack/react-router"

import { api } from "@/shared/api/client"
import { BrandMark } from "@/shared/components/brand"
import { useAuthStore } from "@/shared/stores/auth"
import { Button } from "@/shared/ui/button"
import { ErrorState } from "@/shared/ui/empty-state"
import { Input } from "@/shared/ui/input"
import { Label } from "@/shared/ui/label"

export function ChangePasswordPage() {
  const navigate = useNavigate()
  const user = useAuthStore((s) => s.user)
  const setSession = useAuthStore((s) => s.setSession)
  const token = useAuthStore((s) => s.token)
  const [current, setCurrent] = useState("")
  const [next, setNext] = useState("")
  const [confirm, setConfirm] = useState("")
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)

  async function onSubmit(e: React.FormEvent) {
    e.preventDefault()
    if (next.length < 8) {
      setError("New password must be at least 8 characters")
      return
    }
    if (next !== confirm) {
      setError("Passwords do not match")
      return
    }
    setBusy(true)
    setError(null)
    try {
      const updated = await api.changePassword({
        current_password: current || undefined,
        new_password: next,
      })
      if (token) setSession(token, updated)
      navigate({ to: "/app" })
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed")
    } finally {
      setBusy(false)
    }
  }

  return (
    <div className="flex min-h-dvh items-center justify-center px-4">
      <div className="w-full max-w-md">
        <BrandMark className="mb-8" />
        <div className="rounded-xl border border-(--border-card) bg-(--bg-card)/85 p-6 shadow-(--shadow-depth)">
          <h1 className="text-xl font-semibold tracking-tight">
            Change password
          </h1>
          <p className="mt-1 text-sm text-(--color-text-muted)">
            {user?.must_change_password
              ? "You signed in with a temporary password. Choose a new one to continue."
              : "Update your account password."}
          </p>
          <form className="mt-5 space-y-3" onSubmit={(e) => void onSubmit(e)}>
            {!user?.must_change_password && (
              <div className="space-y-1.5">
                <Label htmlFor="current">Current password</Label>
                <Input
                  id="current"
                  type="password"
                  value={current}
                  onChange={(e) => setCurrent(e.target.value)}
                />
              </div>
            )}
            <div className="space-y-1.5">
              <Label htmlFor="next">New password</Label>
              <Input
                id="next"
                type="password"
                value={next}
                onChange={(e) => setNext(e.target.value)}
                required
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="confirm">Confirm</Label>
              <Input
                id="confirm"
                type="password"
                value={confirm}
                onChange={(e) => setConfirm(e.target.value)}
                required
              />
            </div>
            {error && <ErrorState message={error} />}
            <Button type="submit" variant="gradient" className="w-full" disabled={busy}>
              {busy ? "Saving…" : "Save password"}
            </Button>
          </form>
        </div>
      </div>
    </div>
  )
}
