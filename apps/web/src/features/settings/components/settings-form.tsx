import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"
import { useEffect, useState } from "react"

import { api, type SsoProvider } from "@/shared/api/client"
import { Button } from "@/shared/ui/button"
import { ErrorState } from "@/shared/ui/empty-state"
import { Input } from "@/shared/ui/input"
import { Label } from "@/shared/ui/label"
import { Select } from "@/shared/ui/select"
import { Spinner } from "@/shared/ui/spinner"
import { SwitchField } from "@/shared/ui/switch"

const providers = [
  { value: "azure_ad", label: "Microsoft Entra ID (Azure AD)" },
  { value: "oidc", label: "Generic OIDC" },
  { value: "google", label: "Google" },
  { value: "github", label: "GitHub" },
  { value: "custom", label: "Custom" },
] as const

/** Project + SSO form used inside the settings modal. */
export function SettingsForm() {
  const qc = useQueryClient()
  const { data, isLoading, error } = useQuery({
    queryKey: ["settings"],
    queryFn: () => api.settings(),
  })

  const [projectName, setProjectName] = useState("")
  const [displayName, setDisplayName] = useState("")
  const [publicUrl, setPublicUrl] = useState("")
  const [ssoEnabled, setSsoEnabled] = useState(false)
  const [provider, setProvider] = useState<SsoProvider>("azure_ad")
  const [issuerUrl, setIssuerUrl] = useState("")
  const [clientId, setClientId] = useState("")
  const [clientSecret, setClientSecret] = useState("")
  const [redirectUri, setRedirectUri] = useState("")
  const [message, setMessage] = useState<string | null>(null)
  const [formError, setFormError] = useState<string | null>(null)

  useEffect(() => {
    if (!data) return
    setProjectName(data.project_name)
    setDisplayName(data.display_name ?? "")
    setPublicUrl(data.public_url ?? "")
    setSsoEnabled(data.sso.enabled)
    setProvider(data.sso.provider)
    setIssuerUrl(data.sso.issuer_url ?? "")
    setClientId(data.sso.client_id ?? "")
    setRedirectUri(data.sso.redirect_uri ?? "")
  }, [data])

  const saveProject = useMutation({
    mutationFn: () =>
      api.updateSettings({
        project_name: projectName,
        display_name: displayName || undefined,
        public_url: publicUrl,
      }),
    onSuccess: () => {
      setMessage("Project settings saved")
      setFormError(null)
      void qc.invalidateQueries({ queryKey: ["settings"] })
      void qc.invalidateQueries({ queryKey: ["dashboard"] })
    },
    onError: (e) => setFormError(e instanceof Error ? e.message : "Save failed"),
  })

  const saveSso = useMutation({
    mutationFn: () =>
      api.updateSso({
        enabled: ssoEnabled,
        provider,
        issuer_url: issuerUrl || undefined,
        client_id: clientId || undefined,
        client_secret: clientSecret || undefined,
        redirect_uri: redirectUri || undefined,
      }),
    onSuccess: () => {
      setMessage("SSO settings saved")
      setClientSecret("")
      setFormError(null)
      void qc.invalidateQueries({ queryKey: ["settings"] })
    },
    onError: (e) =>
      setFormError(e instanceof Error ? e.message : "SSO save failed"),
  })

  if (isLoading) {
    return (
      <div className="flex justify-center py-8">
        <Spinner />
      </div>
    )
  }

  if (error || !data) {
    return (
      <ErrorState
        message={
          error instanceof Error ? error.message : "Failed to load settings"
        }
      />
    )
  }

  return (
    <div className="space-y-6">
      {message && (
        <div className="rounded-lg border border-(--color-success)/30 bg-(--color-success)/10 px-3 py-2 text-sm text-(--color-success)">
          {message}
        </div>
      )}
      {formError && <ErrorState message={formError} />}

      <section className="space-y-3">
        <h3 className="text-sm font-semibold tracking-tight">Project</h3>
        <Field label="Project name">
          <Input
            value={projectName}
            onChange={(e) => setProjectName(e.target.value)}
          />
        </Field>
        <Field label="Display name">
          <Input
            value={displayName}
            onChange={(e) => setDisplayName(e.target.value)}
          />
        </Field>
        <Field label="Public URL">
          <Input
            value={publicUrl}
            onChange={(e) => setPublicUrl(e.target.value)}
            placeholder="https://conductor.example.com"
          />
        </Field>
        <p className="text-xs text-(--color-text-subtle)">
          Bind address is {data.bind_host}:{data.bind_port} (restart required to
          change).
        </p>
        <Button
          variant="gradient"
          disabled={!projectName.trim() || saveProject.isPending}
          onClick={() => saveProject.mutate()}
        >
          Save project
        </Button>
      </section>

      <section className="space-y-3 border-t border-(--border-soft) pt-5">
        <h3 className="text-sm font-semibold tracking-tight">SSO</h3>
        <SwitchField
          id="sso-enabled"
          label="Enable SSO"
          description="Microsoft Entra ID / OIDC / Google / GitHub"
          checked={ssoEnabled}
          onCheckedChange={setSsoEnabled}
        />
        <Field label="Provider">
          <Select
            value={provider}
            onValueChange={(v) => setProvider(v as SsoProvider)}
            options={[...providers]}
            disabled={!ssoEnabled}
          />
        </Field>
        <Field label="Issuer URL">
          <Input
            disabled={!ssoEnabled}
            value={issuerUrl}
            onChange={(e) => setIssuerUrl(e.target.value)}
          />
        </Field>
        <div className="grid gap-3 sm:grid-cols-2">
          <Field label="Client ID">
            <Input
              disabled={!ssoEnabled}
              value={clientId}
              onChange={(e) => setClientId(e.target.value)}
            />
          </Field>
          <Field
            label={
              data.sso.client_secret_set
                ? "Client secret (leave blank to keep)"
                : "Client secret"
            }
          >
            <Input
              type="password"
              disabled={!ssoEnabled}
              value={clientSecret}
              onChange={(e) => setClientSecret(e.target.value)}
            />
          </Field>
        </div>
        <Field label="Redirect URI">
          <Input
            disabled={!ssoEnabled}
            value={redirectUri}
            onChange={(e) => setRedirectUri(e.target.value)}
          />
        </Field>
        <Button
          variant="secondary"
          disabled={saveSso.isPending}
          onClick={() => saveSso.mutate()}
        >
          Save SSO
        </Button>
      </section>
    </div>
  )
}

function Field({
  label,
  children,
}: {
  label: string
  children: React.ReactNode
}) {
  return (
    <div className="space-y-1.5">
      <Label>{label}</Label>
      {children}
    </div>
  )
}
