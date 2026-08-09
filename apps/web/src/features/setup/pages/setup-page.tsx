import { useMemo, useState } from "react"
import { useNavigate } from "@tanstack/react-router"
import { AnimatePresence, motion } from "framer-motion"
import {
  ArrowLeft,
  ArrowRight,
  Check,
  KeyRound,
  Server,
  Shield,
  Sparkles,
} from "lucide-react"

import { api, type SsoProvider } from "@/shared/api/client"
import { BrandMark } from "@/shared/components/brand"
import { ThemeToggle } from "@/shared/components/theme-toggle"
import { cn } from "@/shared/lib/utils"
import { Button } from "@/shared/ui/button"
import { ErrorState } from "@/shared/ui/empty-state"
import { Input } from "@/shared/ui/input"
import { Label } from "@/shared/ui/label"
import { Select } from "@/shared/ui/select"
import { Spinner } from "@/shared/ui/spinner"
import { SwitchField } from "@/shared/ui/switch"
import { TooltipProvider } from "@/shared/ui/tooltip"

const steps = [
  { id: "project", title: "Project", icon: Sparkles },
  { id: "host", title: "Host", icon: Server },
  { id: "admin", title: "Admin", icon: Shield },
  { id: "sso", title: "SSO", icon: KeyRound },
] as const

const ssoProviders = [
  { value: "azure_ad", label: "Microsoft Entra ID (Azure AD)" },
  { value: "oidc", label: "Generic OIDC" },
  { value: "google", label: "Google" },
  { value: "custom", label: "Custom" },
] as const satisfies ReadonlyArray<{ value: SsoProvider; label: string }>

export function SetupPage() {
  const navigate = useNavigate()
  const [step, setStep] = useState(0)
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)

  const [projectName, setProjectName] = useState("")
  const [displayName, setDisplayName] = useState("")
  const [bindHost, setBindHost] = useState("0.0.0.0")
  const [bindPort, setBindPort] = useState("4700")
  const [publicUrl, setPublicUrl] = useState("http://127.0.0.1:5174")
  const [adminEmail, setAdminEmail] = useState("")
  const [adminName, setAdminName] = useState("")
  const [adminPassword, setAdminPassword] = useState("")
  const [ssoEnabled, setSsoEnabled] = useState(false)
  const [ssoProvider, setSsoProvider] = useState<SsoProvider>("azure_ad")
  const [issuerUrl, setIssuerUrl] = useState("")
  const [clientId, setClientId] = useState("")
  const [clientSecret, setClientSecret] = useState("")
  const [redirectUri, setRedirectUri] = useState(
    "http://127.0.0.1:4700/api/auth/sso/callback",
  )

  const canNext = useMemo(() => {
    if (step === 0) return projectName.trim().length > 1
    if (step === 1) return Boolean(bindHost) && Number(bindPort) > 0
    if (step === 2)
      return (
        adminEmail.includes("@") &&
        adminName.trim().length > 1 &&
        adminPassword.length >= 12
      )
    if (step === 3) {
      if (!ssoEnabled) return true
      return Boolean(
        clientId.trim() &&
          clientSecret.trim() &&
          issuerUrl.trim() &&
          redirectUri.trim(),
      )
    }
    return false
  }, [
    step,
    projectName,
    bindHost,
    bindPort,
    adminEmail,
    adminName,
    adminPassword,
    ssoEnabled,
    clientId,
    clientSecret,
    issuerUrl,
    redirectUri,
  ])

  async function finish() {
    setBusy(true)
    setError(null)
    try {
      await api.completeSetup({
        project_name: projectName.trim(),
        display_name: displayName.trim() || undefined,
        bind_host: bindHost.trim(),
        bind_port: Number(bindPort),
        public_url: publicUrl.trim() || undefined,
        admin_email: adminEmail.trim(),
        admin_display_name: adminName.trim(),
        admin_password: adminPassword,
        sso: {
          enabled: ssoEnabled,
          provider: ssoProvider,
          issuer_url: issuerUrl || undefined,
          client_id: clientId || undefined,
          client_secret: clientSecret || undefined,
          redirect_uri: redirectUri || undefined,
        },
      })
      navigate({ to: "/login" })
    } catch (e) {
      setError(e instanceof Error ? e.message : "Setup failed")
    } finally {
      setBusy(false)
    }
  }

  return (
    <TooltipProvider>
      <div className="relative mx-auto flex min-h-dvh max-w-3xl flex-col px-4 py-8 sm:px-6 sm:py-10">
        <div className="mb-8 flex items-start justify-between gap-4">
          <BrandMark size="lg" />
          <ThemeToggle />
        </div>

        <div className="mb-8">
          <h1 className="text-2xl font-semibold tracking-tight text-(--color-text)">
            Set up your Conductor
          </h1>
          <p className="mt-1.5 max-w-xl text-sm text-(--color-text-muted)">
            First-run wizard for the project master server. After publish,
            members can sign in and create secrets to connect EvoFlux.
          </p>
        </div>

        <div className="mb-6 grid grid-cols-2 gap-2 sm:grid-cols-4">
          {steps.map((s, i) => {
            const Icon = s.icon
            const active = i === step
            const done = i < step
            return (
              <div
                key={s.id}
                className={cn(
                  "flex items-center gap-2 rounded-md border px-3 py-2 text-xs transition-colors",
                  active
                    ? "border-(--color-accent)/40 bg-(--color-accent-soft) text-(--color-text)"
                    : done
                      ? "border-(--color-border) bg-(--bg-card) text-(--color-text-2)"
                      : "border-(--border-soft) text-(--color-text-subtle)",
                )}
              >
                <span
                  className={cn(
                    "flex size-5 shrink-0 items-center justify-center rounded-sm",
                    done || active ? "bg-(--bg-key)" : "bg-transparent",
                  )}
                >
                  {done ? (
                    <Check className="size-3" />
                  ) : (
                    <Icon className="size-3" />
                  )}
                </span>
                <span className="truncate">{s.title}</span>
              </div>
            )
          })}
        </div>

        <div className="rounded-xl border border-(--border-card) bg-(--bg-card)/80 p-4 shadow-(--shadow-depth) backdrop-blur-sm sm:p-5">
          <AnimatePresence mode="wait">
            <motion.div
              key={step}
              initial={{ opacity: 0, y: 8 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -6 }}
              transition={{ duration: 0.2, ease: [0.16, 1, 0.3, 1] }}
              className="space-y-4"
            >
              {step === 0 && (
                <>
                  <Field label="Project name" htmlFor="project">
                    <Input
                      id="project"
                      value={projectName}
                      onChange={(e) => setProjectName(e.target.value)}
                      placeholder="acme-platform"
                      autoFocus
                    />
                  </Field>
                  <Field label="Display name" htmlFor="display">
                    <Input
                      id="display"
                      value={displayName}
                      onChange={(e) => setDisplayName(e.target.value)}
                      placeholder="Acme Platform"
                    />
                  </Field>
                </>
              )}

              {step === 1 && (
                <>
                  <div className="grid gap-3 sm:grid-cols-2">
                    <Field label="Bind host" htmlFor="host">
                      <Input
                        id="host"
                        value={bindHost}
                        onChange={(e) => setBindHost(e.target.value)}
                        placeholder="0.0.0.0"
                      />
                    </Field>
                    <Field label="Bind port" htmlFor="port">
                      <Input
                        id="port"
                        value={bindPort}
                        onChange={(e) => setBindPort(e.target.value)}
                        placeholder="4700"
                      />
                    </Field>
                  </div>
                  <Field label="Public URL" htmlFor="public">
                    <Input
                      id="public"
                      value={publicUrl}
                      onChange={(e) => setPublicUrl(e.target.value)}
                      placeholder="https://conductor.example.com"
                    />
                  </Field>
                  <p className="text-xs text-(--color-text-subtle)">
                    EvoFlux clients and SSO redirects will use this published
                    URL.
                  </p>
                </>
              )}

              {step === 2 && (
                <>
                  <Field label="Admin display name" htmlFor="admin-name">
                    <Input
                      id="admin-name"
                      value={adminName}
                      onChange={(e) => setAdminName(e.target.value)}
                      placeholder="Hung"
                    />
                  </Field>
                  <Field label="Admin email" htmlFor="admin-email">
                    <Input
                      id="admin-email"
                      type="email"
                      value={adminEmail}
                      onChange={(e) => setAdminEmail(e.target.value)}
                      placeholder="admin@company.com"
                    />
                  </Field>
                  <Field label="Admin password" htmlFor="admin-pass">
                    <Input
                      id="admin-pass"
                      type="password"
                      value={adminPassword}
                      onChange={(e) => setAdminPassword(e.target.value)}
                      placeholder="At least 12 characters"
                      autoComplete="new-password"
                    />
                  </Field>
                </>
              )}

              {step === 3 && (
                <>
                  <SwitchField
                    id="sso-enabled"
                    label="Enable SSO"
                    description="OpenID Connect, Microsoft Entra ID, or Google"
                    checked={ssoEnabled}
                    onCheckedChange={setSsoEnabled}
                  />
                  <div
                    className={cn(
                      "space-y-3 transition-opacity",
                      !ssoEnabled && "pointer-events-none opacity-45",
                    )}
                  >
                    <Field label="Provider" htmlFor="provider">
                      <Select
                        id="provider"
                        disabled={!ssoEnabled}
                        value={ssoProvider}
                        onValueChange={setSsoProvider}
                        options={ssoProviders}
                      />
                    </Field>
                    <Field label="Issuer URL" htmlFor="issuer">
                      <Input
                        id="issuer"
                        disabled={!ssoEnabled}
                        value={issuerUrl}
                        onChange={(e) => setIssuerUrl(e.target.value)}
                        placeholder={
                          ssoProvider === "azure_ad"
                            ? "https://login.microsoftonline.com/{tenant-id}/v2.0"
                            : "https://issuer.example.com"
                        }
                      />
                    </Field>
                    <div className="grid gap-3 sm:grid-cols-2">
                      <Field label="Client ID" htmlFor="client-id">
                        <Input
                          id="client-id"
                          disabled={!ssoEnabled}
                          value={clientId}
                          onChange={(e) => setClientId(e.target.value)}
                        />
                      </Field>
                      <Field label="Client secret" htmlFor="client-secret">
                        <Input
                          id="client-secret"
                          type="password"
                          disabled={!ssoEnabled}
                          value={clientSecret}
                          onChange={(e) => setClientSecret(e.target.value)}
                        />
                      </Field>
                    </div>
                    <Field label="Redirect URI" htmlFor="redirect">
                      <Input
                        id="redirect"
                        disabled={!ssoEnabled}
                        value={redirectUri}
                        onChange={(e) => setRedirectUri(e.target.value)}
                      />
                    </Field>
                    <p className="text-xs text-(--color-text-subtle)">
                      {ssoProvider === "azure_ad"
                        ? "Register a Web app in Entra ID. Redirect URI must match exactly. Public URL is used to return to the console after login."
                        : "OIDC authorization-code + PKCE. Redirect URI must point at /api/auth/sso/callback on this Conductor host."}
                    </p>
                  </div>
                </>
              )}
            </motion.div>
          </AnimatePresence>

          {error && <ErrorState className="mt-4" message={error} />}

          <div className="mt-6 flex items-center justify-between gap-3">
            <Button
              variant="ghost"
              disabled={step === 0 || busy}
              onClick={() => setStep((s) => Math.max(0, s - 1))}
            >
              <ArrowLeft className="size-3.5" />
              Back
            </Button>
            {step < steps.length - 1 ? (
              <Button
                variant="gradient"
                disabled={!canNext || busy}
                onClick={() => setStep((s) => s + 1)}
              >
                Continue
                <ArrowRight className="size-3.5" />
              </Button>
            ) : (
              <Button
                variant="gradient"
                disabled={!canNext || busy}
                onClick={() => void finish()}
              >
                {busy ? (
                  <>
                    <Spinner className="text-current" />
                    Publishing…
                  </>
                ) : (
                  "Finish & publish"
                )}
              </Button>
            )}
          </div>
        </div>
      </div>
    </TooltipProvider>
  )
}

function Field({
  label,
  htmlFor,
  children,
}: {
  label: string
  htmlFor: string
  children: React.ReactNode
}) {
  return (
    <div className="space-y-1.5">
      <Label htmlFor={htmlFor}>{label}</Label>
      {children}
    </div>
  )
}
