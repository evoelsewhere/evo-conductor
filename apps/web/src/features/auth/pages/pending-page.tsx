import { BrandMark } from "@/shared/components/brand"
import { Button } from "@/shared/ui/button"

export function PendingPage() {
  const email =
    typeof window !== "undefined"
      ? new URLSearchParams(window.location.search).get("email")
      : null

  return (
    <div className="flex min-h-dvh flex-col items-center justify-center gap-5 px-6 text-center">
      <BrandMark size="lg" />
      <div className="max-w-md">
        <h1 className="text-xl font-semibold tracking-tight">Awaiting approval</h1>
        <p className="mt-2 text-sm text-(--color-text-muted)">
          {email ? (
            <>
              <span className="font-medium text-(--color-text)">{email}</span> signed
              in via SSO and is waiting for an admin to approve access.
            </>
          ) : (
            <>
              Your account is pending admin approval. Ask a project admin to
              activate you from Members.
            </>
          )}
        </p>
      </div>
      <Button variant="outline" onClick={() => (window.location.href = "/login")}>
        Back to sign in
      </Button>
    </div>
  )
}
