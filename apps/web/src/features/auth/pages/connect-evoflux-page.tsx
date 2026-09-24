import { useMutation, useQuery } from "@tanstack/react-query"
import { useState } from "react"

import { api } from "@/shared/api/client"
import { BrandMark } from "@/shared/components/brand"
import { CONNECTION_SECRET_SCOPES } from "@/shared/constants/secret"
import { useAuthStore } from "@/shared/stores/auth"
import { Button } from "@/shared/ui/button"
import { ErrorState } from "@/shared/ui/empty-state"

export function ConnectEvofluxPage() {
  const user = useAuthStore((s) => s.user)
  const memberId = new URLSearchParams(window.location.search).get("member")
  const project = useQuery({ queryKey: ["project"], queryFn: () => api.project() })
  const [connectUrl, setConnectUrl] = useState<string | null>(null)

  const connect = useMutation({
    mutationFn: () => {
      if (!user) throw new Error("Sign in first")
      return api.createMemberSecret(user.id, {
        name: "EvoFlux desktop",
        scopes: CONNECTION_SECRET_SCOPES,
      })
    },
    onSuccess: (result) => {
      const url = `evoflux://connect?token=${encodeURIComponent(result.token)}`
      setConnectUrl(url)
      window.location.href = url
    },
  })

  const identityMismatch = Boolean(memberId && user && memberId !== user.id)
  const projectLabel = project.data?.display_name ?? project.data?.project_name

  return (
    <div className="flex min-h-dvh items-center justify-center px-4">
      <div className="w-full max-w-md">
        <BrandMark
          className="mb-8"
          title={projectLabel}
          logoUrl={project.data?.logo_url}
        />
        <div className="space-y-4 rounded-xl border border-(--border-card) bg-(--bg-card)/85 p-6 shadow-(--shadow-depth)">
          <div>
            <h1 className="text-xl font-semibold tracking-tight">
              Connect EvoFlux
            </h1>
            <p className="mt-1 text-sm text-(--color-text-muted)">
              {user
                ? `Signed in as ${user.email}. Click below to link your EvoFlux desktop app to ${projectLabel ?? "this project"}.`
                : "Sign in to continue."}
            </p>
          </div>

          {identityMismatch && (
            <div className="rounded-lg border border-(--color-warning)/25 bg-(--color-warning)/8 px-3 py-2 text-xs text-(--color-text-muted)">
              This invite was sent to a different account. You&apos;re
              currently signed in as {user?.email}. Sign out and sign in with
              the invited email if that wasn&apos;t you.
            </div>
          )}

          {connect.error && (
            <ErrorState
              message={
                connect.error instanceof Error
                  ? connect.error.message
                  : "Failed to connect EvoFlux"
              }
            />
          )}

          {connectUrl ? (
            <div className="space-y-3">
              <p className="text-sm text-(--color-text-muted)">
                Opening EvoFlux… If nothing happened, make sure the EvoFlux
                desktop app is installed on this computer, then try again.
              </p>
              <Button
                variant="gradient"
                className="w-full"
                onClick={() => {
                  window.location.href = connectUrl
                }}
              >
                Open EvoFlux again
              </Button>
            </div>
          ) : (
            <Button
              variant="gradient"
              className="w-full"
              disabled={connect.isPending || !user}
              onClick={() => connect.mutate()}
            >
              {connect.isPending ? "Connecting…" : "Connect EvoFlux"}
            </Button>
          )}
        </div>
      </div>
    </div>
  )
}
