import { BrandMark } from "@/shared/components/brand"
import { Button } from "@/shared/ui/button"

export function PendingPage() {
  return (
    <div className="flex min-h-dvh flex-col items-center justify-center gap-5 px-6 text-center">
      <BrandMark size="lg" />
      <div className="max-w-md">
        <h1 className="text-xl font-semibold tracking-tight">Awaiting approval</h1>
        <p className="mt-2 text-sm text-(--color-text-muted)">
          Your account is pending admin approval. Ask a project administrator to
          activate it from Members, then sign in again.
        </p>
      </div>
      <Button variant="outline" onClick={() => (window.location.href = "/login")}>
        Back to sign in
      </Button>
    </div>
  )
}
