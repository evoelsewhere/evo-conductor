import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"
import { Building2, KeyRound, Network } from "lucide-react"
import { useEffect, useRef, useState } from "react"

import { api, type SsoProvider } from "@/shared/api/client"
import { BrandLogo } from "@/shared/components/logo"
import { cn } from "@/shared/lib/utils"
import { Badge, StatusDot } from "@/shared/ui/badge"
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
  { value: "custom", label: "Custom" },
] as const

const tabs = [
  { id: "general", label: "General", icon: Building2 },
  { id: "network", label: "Network", icon: Network },
  { id: "sso", label: "SSO", icon: KeyRound },
] as const

type TabId = (typeof tabs)[number]["id"]

/** Uploaded logos are stored as data URLs — keep them small. */
const MAX_LOGO_BYTES = 512 * 1024

/** Project + network + SSO form used inside the settings modal. */
export function SettingsForm() {
  const qc = useQueryClient()
  const { data, isLoading, error } = useQuery({
    queryKey: ["settings"],
    queryFn: () => api.settings(),
  })

  const [tab, setTab] = useState<TabId>("general")
  const [projectName, setProjectName] = useState("")
  const [displayName, setDisplayName] = useState("")
  const [logoUrl, setLogoUrl] = useState("")
  const [bindHost, setBindHost] = useState("")
  const [bindPort, setBindPort] = useState("")
  const [publicUrl, setPublicUrl] = useState("")
  const [maxConnections, setMaxConnections] = useState("")
  const [maxPerSecret, setMaxPerSecret] = useState("")
  const [heartbeatSeconds, setHeartbeatSeconds] = useState("")
  const [ssoEnabled, setSsoEnabled] = useState(false)
  const [provider, setProvider] = useState<SsoProvider>("azure_ad")
  const [issuerUrl, setIssuerUrl] = useState("")
  const [clientId, setClientId] = useState("")
  const [clientSecret, setClientSecret] = useState("")
  const [redirectUri, setRedirectUri] = useState("")
  const [message, setMessage] = useState<string | null>(null)
  const [formError, setFormError] = useState<string | null>(null)
  const logoFileRef = useRef<HTMLInputElement>(null)

  useEffect(() => {
    if (!data) return
    setProjectName(data.project_name)
    setDisplayName(data.display_name ?? "")
    setLogoUrl(data.logo_url ?? "")
    setBindHost(data.bind_host)
    setBindPort(String(data.bind_port))
    setPublicUrl(data.public_url ?? "")
    setMaxConnections(String(data.realtime?.max_connections ?? ""))
    setMaxPerSecret(String(data.realtime?.max_connections_per_secret ?? ""))
    setHeartbeatSeconds(String(data.realtime?.heartbeat_seconds ?? ""))
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
        display_name: displayName,
        logo_url: logoUrl,
      }),
    onSuccess: () => {
      setMessage("Project settings saved")
      setFormError(null)
      void qc.invalidateQueries({ queryKey: ["settings"] })
      void qc.invalidateQueries({ queryKey: ["project"] })
      void qc.invalidateQueries({ queryKey: ["dashboard"] })
    },
    onError: (e) => setFormError(e instanceof Error ? e.message : "Save failed"),
  })

  const saveNetwork = useMutation({
    mutationFn: () => {
      const port = Number(bindPort)
      const maxConn = Number(maxConnections)
      const perSecret = Number(maxPerSecret)
      const heartbeat = Number(heartbeatSeconds)
      if (!bindHost.trim()) throw new Error("Bind host cannot be empty")
      if (!Number.isInteger(port) || port < 1 || port > 65535) {
        throw new Error("Bind port must be between 1 and 65535")
      }
      if (!Number.isInteger(maxConn) || maxConn < 1) {
        throw new Error("Max connections must be at least 1")
      }
      if (!Number.isInteger(perSecret) || perSecret < 1) {
        throw new Error("Max connections per secret must be at least 1")
      }
      if (!Number.isInteger(heartbeat) || heartbeat < 5 || heartbeat > 300) {
        throw new Error("Heartbeat must be between 5 and 300 seconds")
      }
      return api.updateNetwork({
        bind_host: bindHost.trim(),
        bind_port: port,
        public_url: publicUrl,
        realtime: {
          max_connections: maxConn,
          max_connections_per_secret: perSecret,
          heartbeat_seconds: heartbeat,
        },
      })
    },
    onSuccess: () => {
      setMessage("Network settings saved")
      setFormError(null)
      void qc.invalidateQueries({ queryKey: ["settings"] })
    },
    onError: (e) =>
      setFormError(e instanceof Error ? e.message : "Network save failed"),
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

  function onLogoFile(event: React.ChangeEvent<HTMLInputElement>) {
    const file = event.target.files?.[0]
    event.target.value = ""
    if (!file) return
    if (file.size > MAX_LOGO_BYTES) {
      setFormError("Logo image must be 512 KB or smaller")
      return
    }
    const reader = new FileReader()
    reader.onload = () => {
      if (typeof reader.result === "string") {
        setLogoUrl(reader.result)
        setFormError(null)
        setMessage(null)
      }
    }
    reader.readAsDataURL(file)
  }

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
    <div className="sm:grid sm:grid-cols-[10.5rem_minmax(0,1fr)] sm:gap-6">
      <nav
        aria-label="Settings sections"
        className="mb-4 flex gap-1 overflow-x-auto sm:mb-0 sm:flex-col sm:overflow-visible"
      >
        {tabs.map(({ id, label, icon: Icon }) => (
          <button
            key={id}
            type="button"
            aria-current={tab === id ? "page" : undefined}
            onClick={() => setTab(id)}
            className={cn(
              "flex items-center gap-2 rounded-md px-2.5 py-2 text-sm whitespace-nowrap text-(--color-text-muted) transition-colors hover:bg-(--bg-key) hover:text-(--color-text)",
              tab === id && "bg-(--bg-key) font-medium text-(--color-text)",
            )}
          >
            <Icon className="size-4 shrink-0 opacity-85" strokeWidth={1.65} />
            {label}
          </button>
        ))}
      </nav>

      <div className="min-w-0 space-y-4 sm:min-h-[26rem]">
        {message && (
          <div className="rounded-lg border border-(--color-success)/30 bg-(--color-success)/10 px-3 py-2 text-sm text-(--color-success)">
            {message}
          </div>
        )}
        {formError && <ErrorState message={formError} />}

        {tab === "general" && (
          <section className="space-y-3">
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
            <Field label="Logo">
              <div className="flex items-center gap-4">
                <span className="grid size-16 shrink-0 place-items-center overflow-hidden rounded-xl bg-(--bg-key) shadow-[0_2px_10px_-3px_rgba(0,0,0,0.35)]">
                  {logoUrl ? (
                    <img
                      src={logoUrl}
                      alt="Project logo preview"
                      className="size-full object-cover"
                    />
                  ) : (
                    <BrandLogo size="lg" />
                  )}
                </span>
                <div className="space-y-2">
                  <div className="flex flex-wrap gap-2">
                    <Button
                      variant="secondary"
                      onClick={() => logoFileRef.current?.click()}
                    >
                      Upload logo
                    </Button>
                    {logoUrl && (
                      <Button variant="ghost" onClick={() => setLogoUrl("")}>
                        Remove
                      </Button>
                    )}
                  </div>
                  <p className="text-xs text-(--color-text-subtle)">
                    PNG, JPG, SVG or WebP up to 512 KB. Shown in the sidebar and
                    on the sign-in page. Remove to restore the default EvoFlux
                    mark.
                  </p>
                </div>
              </div>
              <input
                ref={logoFileRef}
                type="file"
                accept="image/png,image/jpeg,image/svg+xml,image/webp"
                className="hidden"
                onChange={onLogoFile}
              />
            </Field>
            <Button
              variant="gradient"
              disabled={!projectName.trim() || saveProject.isPending}
              onClick={() => saveProject.mutate()}
            >
              Save project
            </Button>
          </section>
        )}

        {tab === "network" && (
          <section className="space-y-3">
            <div className="grid gap-3 sm:grid-cols-2">
              <Field label="Bind host">
                <Input
                  value={bindHost}
                  onChange={(e) => setBindHost(e.target.value)}
                  placeholder="0.0.0.0"
                />
              </Field>
              <Field label="Bind port">
                <Input
                  inputMode="numeric"
                  value={bindPort}
                  onChange={(e) => setBindPort(e.target.value)}
                  placeholder="4700"
                />
              </Field>
            </div>
            <p className="text-xs text-(--color-text-subtle)">
              Bind address applies after restart. The CONDUCTOR_HOST and
              CONDUCTOR_PORT environment variables take precedence when set.
            </p>
            <Field label="Public URL">
              <Input
                value={publicUrl}
                onChange={(e) => setPublicUrl(e.target.value)}
                placeholder="https://conductor.example.com"
              />
            </Field>

            <h3 className="border-t border-(--border-soft) pt-4 text-sm font-semibold tracking-tight">
              Realtime (SSE)
            </h3>
            <div className="grid gap-3 sm:grid-cols-3">
              <Field label="Max connections">
                <Input
                  inputMode="numeric"
                  value={maxConnections}
                  onChange={(e) => setMaxConnections(e.target.value)}
                />
              </Field>
              <Field label="Per secret">
                <Input
                  inputMode="numeric"
                  value={maxPerSecret}
                  onChange={(e) => setMaxPerSecret(e.target.value)}
                />
              </Field>
              <Field label="Heartbeat (s)">
                <Input
                  inputMode="numeric"
                  value={heartbeatSeconds}
                  onChange={(e) => setHeartbeatSeconds(e.target.value)}
                />
              </Field>
            </div>
            <p className="text-xs text-(--color-text-subtle)">
              Heartbeat and per-secret limits apply to new connections right
              away. Raising the global limit is immediate; lowering it takes
              full effect after restart.
            </p>
            <Button
              variant="gradient"
              disabled={saveNetwork.isPending}
              onClick={() => saveNetwork.mutate()}
            >
              Save network
            </Button>
          </section>
        )}

        {tab === "sso" && (
          <section className="space-y-3">
            <div className="flex items-center justify-between gap-3">
              <h3 className="text-sm font-semibold tracking-tight">SSO</h3>
              <Badge tone={data.sso.enabled ? "success" : "neutral"}>
                <StatusDot tone={data.sso.enabled ? "success" : "neutral"} />
                {data.sso.enabled ? "Enabled" : "Disabled"}
              </Badge>
            </div>
            <SwitchField
              id="sso-enabled"
              label="Enable SSO"
              description="OpenID Connect, Microsoft Entra ID, or Google"
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
                placeholder={
                  provider === "azure_ad"
                    ? "https://login.microsoftonline.com/{tenant-id}/v2.0"
                    : "https://id.example.com"
                }
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
                  autoComplete="new-password"
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
            {ssoEnabled && (
              <p className="rounded-lg border border-(--color-border) bg-(--bg-key)/50 px-3 py-2 text-xs text-(--color-text-muted)">
                ID tokens are verified against the provider JWKS, issuer,
                audience, and nonce. The redirect URI must match the provider
                registration exactly.
              </p>
            )}
            <Button
              variant="secondary"
              disabled={saveSso.isPending}
              onClick={() => saveSso.mutate()}
            >
              Save SSO
            </Button>
          </section>
        )}
      </div>
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
