import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"
import {
  Activity,
  Building2,
  Database,
  HardDrive,
  KeyRound,
  Network,
  ShieldCheck,
} from "lucide-react"
import { useEffect, useRef, useState } from "react"

import {
  api,
  type CollectionLevel,
  type GitAuthMode,
  type SsoProvider,
  type StorageBackend,
} from "@/shared/api/client"
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
import { Textarea } from "@/shared/ui/textarea"

const providers = [
  { value: "azure_ad", label: "Microsoft Entra ID (Azure AD)" },
  { value: "oidc", label: "Generic OIDC" },
  { value: "google", label: "Google" },
  { value: "custom", label: "Custom" },
] as const

const tabs = [
  {
    id: "general",
    label: "General",
    description: "Identity and branding",
    icon: Building2,
  },
  {
    id: "network",
    label: "Connectivity",
    description: "Public URL and realtime",
    icon: Network,
  },
  {
    id: "storage",
    label: "Object storage",
    description: "Files and migration",
    icon: HardDrive,
  },
  {
    id: "data-policy",
    label: "Data & privacy",
    description: "Client collection policy",
    icon: ShieldCheck,
  },
  {
    id: "sso",
    label: "Authentication",
    description: "SSO and onboarding",
    icon: KeyRound,
  },
] as const

type TabId = (typeof tabs)[number]["id"]

/** Logos are uploaded to the selected object store. */
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
  const [projectDescription, setProjectDescription] = useState("")
  const [logoUrl, setLogoUrl] = useState("")
  const [bindHost, setBindHost] = useState("")
  const [bindPort, setBindPort] = useState("")
  const [publicUrl, setPublicUrl] = useState("")
  const [maxConnections, setMaxConnections] = useState("")
  const [maxPerSecret, setMaxPerSecret] = useState("")
  const [heartbeatSeconds, setHeartbeatSeconds] = useState("")
  const [storageBackend, setStorageBackend] = useState<StorageBackend>("local")
  const [localRoot, setLocalRoot] = useState("")
  const [s3Bucket, setS3Bucket] = useState("")
  const [s3Region, setS3Region] = useState("")
  const [s3Endpoint, setS3Endpoint] = useState("")
  const [s3Prefix, setS3Prefix] = useState("")
  const [s3PathStyle, setS3PathStyle] = useState(false)
  const [azureAccount, setAzureAccount] = useState("")
  const [azureContainer, setAzureContainer] = useState("")
  const [azureEndpoint, setAzureEndpoint] = useState("")
  const [azurePrefix, setAzurePrefix] = useState("")
  const [gitRepositoryUrl, setGitRepositoryUrl] = useState("")
  const [gitBranch, setGitBranch] = useState("main")
  const [gitPrefix, setGitPrefix] = useState("")
  const [gitAuthMode, setGitAuthMode] =
    useState<GitAuthMode>("environment")
  const [gitUsername, setGitUsername] = useState("")
  const [gitCredential, setGitCredential] = useState("")
  const [clearGitCredential, setClearGitCredential] = useState(false)
  const [collectionLevel, setCollectionLevel] =
    useState<CollectionLevel>("L1")
  const [ssoEnabled, setSsoEnabled] = useState(false)
  const [provider, setProvider] = useState<SsoProvider>("azure_ad")
  const [issuerUrl, setIssuerUrl] = useState("")
  const [clientId, setClientId] = useState("")
  const [clientSecret, setClientSecret] = useState("")
  const [redirectUri, setRedirectUri] = useState("")
  const [scopes, setScopes] = useState("")
  const [message, setMessage] = useState<string | null>(null)
  const [formError, setFormError] = useState<string | null>(null)
  const logoFileRef = useRef<HTMLInputElement>(null)

  useEffect(() => {
    if (!data) return
    setProjectName(data.project_name)
    setDisplayName(data.display_name ?? "")
    setProjectDescription(data.description ?? "")
    setLogoUrl(data.logo_url ?? "")
    setBindHost(data.bind_host)
    setBindPort(String(data.bind_port))
    setPublicUrl(data.public_url ?? "")
    setMaxConnections(String(data.realtime?.max_connections ?? ""))
    setMaxPerSecret(String(data.realtime?.max_connections_per_secret ?? ""))
    setHeartbeatSeconds(String(data.realtime?.heartbeat_seconds ?? ""))
    setStorageBackend(data.storage.backend)
    setLocalRoot(data.storage.local.root ?? "")
    setS3Bucket(data.storage.s3.bucket)
    setS3Region(data.storage.s3.region)
    setS3Endpoint(data.storage.s3.endpoint ?? "")
    setS3Prefix(data.storage.s3.prefix)
    setS3PathStyle(data.storage.s3.path_style)
    setAzureAccount(data.storage.azure_blob.account)
    setAzureContainer(data.storage.azure_blob.container)
    setAzureEndpoint(data.storage.azure_blob.endpoint ?? "")
    setAzurePrefix(data.storage.azure_blob.prefix)
    setGitRepositoryUrl(data.storage.git.repository_url)
    setGitBranch(data.storage.git.branch)
    setGitPrefix(data.storage.git.prefix)
    setGitAuthMode(data.storage.git.auth_mode)
    setGitUsername(data.storage.git.username ?? "")
    setGitCredential("")
    setClearGitCredential(false)
    setCollectionLevel(data.data_policy.collection_level)
    setSsoEnabled(data.sso.enabled)
    setProvider(data.sso.provider)
    setIssuerUrl(data.sso.issuer_url ?? "")
    setClientId(data.sso.client_id ?? "")
    setRedirectUri(data.sso.redirect_uri ?? "")
    setScopes(data.sso.scopes.join(" "))
  }, [data])

  const saveProject = useMutation({
    mutationFn: () =>
      api.updateSettings({
        project_name: projectName,
        display_name: displayName,
        description: projectDescription,
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
        scopes: scopes
          .split(/[\s,]+/)
          .map((scope) => scope.trim())
          .filter(Boolean),
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

  const saveDataPolicy = useMutation({
    mutationFn: () => api.updateDataPolicy(collectionLevel),
    onSuccess: () => {
      setMessage("Client data policy saved")
      setFormError(null)
      void qc.invalidateQueries({ queryKey: ["settings"] })
    },
    onError: (e) =>
      setFormError(e instanceof Error ? e.message : "Data policy save failed"),
  })

  const saveStorage = useMutation({
    mutationFn: () =>
      api.updateStorage(
        {
          backend: storageBackend,
          local: { root: localRoot.trim() || null },
          s3: {
            bucket: s3Bucket.trim(),
            region: s3Region.trim(),
            endpoint: s3Endpoint.trim() || null,
            prefix: s3Prefix.trim(),
            path_style: s3PathStyle,
          },
          azure_blob: {
            account: azureAccount.trim(),
            container: azureContainer.trim(),
            endpoint: azureEndpoint.trim() || null,
            prefix: azurePrefix.trim(),
          },
          git: {
            repository_url: gitRepositoryUrl.trim(),
            branch: gitBranch.trim(),
            prefix: gitPrefix.trim(),
            auth_mode: gitAuthMode,
            username: gitUsername.trim() || null,
            credential:
              gitAuthMode === "https_token"
                ? gitCredential.trim() || null
                : null,
            clear_credential: clearGitCredential,
            credential_set: data?.storage.git.credential_set ?? false,
          },
        },
        true,
      ),
    onSuccess: (result) => {
      setGitCredential("")
      setClearGitCredential(false)
      setMessage(
        `Storage switched to ${result.storage.backend}; ${result.objects_copied.toLocaleString()} objects verified`,
      )
      setFormError(null)
      void qc.invalidateQueries({ queryKey: ["settings"] })
    },
    onError: (e) =>
      setFormError(e instanceof Error ? e.message : "Storage migration failed"),
  })

  const uploadLogo = useMutation({
    mutationFn: (file: File) => api.uploadProjectLogo(file),
    onSuccess: (settings) => {
      setLogoUrl(settings.logo_url ?? "")
      setMessage("Project logo uploaded to object storage")
      setFormError(null)
      void qc.invalidateQueries({ queryKey: ["settings"] })
      void qc.invalidateQueries({ queryKey: ["project"] })
    },
    onError: (e) =>
      setFormError(e instanceof Error ? e.message : "Logo upload failed"),
  })

  const deleteLogo = useMutation({
    mutationFn: () => api.deleteProjectLogo(),
    onSuccess: () => {
      setLogoUrl("")
      setMessage("Project logo removed")
      setFormError(null)
      void qc.invalidateQueries({ queryKey: ["settings"] })
      void qc.invalidateQueries({ queryKey: ["project"] })
    },
    onError: (e) =>
      setFormError(e instanceof Error ? e.message : "Logo removal failed"),
  })

  function onLogoFile(event: React.ChangeEvent<HTMLInputElement>) {
    const file = event.target.files?.[0]
    event.target.value = ""
    if (!file) return
    if (file.size > MAX_LOGO_BYTES) {
      setFormError("Logo image must be 512 KB or smaller")
      return
    }
    if (!["image/png", "image/jpeg", "image/webp"].includes(file.type)) {
      setFormError("Logo must be PNG, JPEG or WebP")
      return
    }
    setMessage(null)
    uploadLogo.mutate(file)
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

  const gitCredentialMatchesRepository =
    data.storage.git.credential_set &&
    gitRepositoryUrl.trim() === data.storage.git.repository_url
  const gitStorageIsIncomplete =
    storageBackend === "git" &&
    (!gitRepositoryUrl.trim() ||
      !gitBranch.trim() ||
      (gitAuthMode === "https_token" &&
        !gitCredential.trim() &&
        !gitCredentialMatchesRepository))

  return (
    <div className="flex min-h-0 flex-1 flex-col lg:grid lg:grid-cols-[15rem_minmax(0,1fr)]">
      <aside className="shrink-0 border-b border-(--border-soft) bg-(--bg-key)/35 p-4 lg:border-r lg:border-b-0 lg:p-5">
        <div className="mb-4 hidden items-center gap-3 rounded-xl border border-(--border-soft) bg-(--bg-card) p-3 lg:flex">
          <span className="grid size-10 shrink-0 place-items-center overflow-hidden rounded-lg bg-(--bg-key)">
            {logoUrl ? (
              <img
                src={logoUrl}
                alt=""
                className="size-full object-cover"
              />
            ) : (
              <BrandLogo size="sm" />
            )}
          </span>
          <div className="min-w-0">
            <p className="truncate text-sm font-semibold">
              {displayName || projectName}
            </p>
            <p className="truncate text-xs text-(--color-text-subtle)">
              {projectName}
            </p>
          </div>
        </div>

        <nav
          aria-label="Settings sections"
          className="flex gap-1 overflow-x-auto lg:flex-col lg:overflow-visible"
        >
          {tabs.map(({ id, label, description, icon: Icon }) => (
            <button
              key={id}
              type="button"
              aria-current={tab === id ? "page" : undefined}
              onClick={() => {
                setTab(id)
                setMessage(null)
                setFormError(null)
              }}
              className={cn(
                "group flex shrink-0 items-center gap-3 rounded-lg px-3 py-2.5 text-left text-sm text-(--color-text-muted) transition-colors hover:bg-(--bg-key) hover:text-(--color-text)",
                tab === id &&
                  "bg-(--bg-key) font-medium text-(--color-text) ring-1 ring-(--border-soft)",
              )}
            >
              <span
                className={cn(
                  "grid size-8 shrink-0 place-items-center rounded-md bg-(--bg-card) text-(--color-text-subtle)",
                  tab === id && "text-(--color-accent)",
                )}
              >
                <Icon className="size-4" strokeWidth={1.7} />
              </span>
              <span>
                <span className="block whitespace-nowrap">{label}</span>
                <span className="mt-0.5 hidden text-[11px] font-normal text-(--color-text-subtle) lg:block">
                  {description}
                </span>
              </span>
            </button>
          ))}
        </nav>

        <div className="mt-5 hidden space-y-2 border-t border-(--border-soft) pt-4 lg:block">
          <SettingStatus
            icon={HardDrive}
            label="Storage"
            value={data.storage.backend.replace("_", " ")}
          />
          <SettingStatus
            icon={Activity}
            label="Telemetry"
            value={data.data_policy.collection_level}
          />
          <SettingStatus
            icon={KeyRound}
            label="SSO"
            value={data.sso.enabled ? "enabled" : "disabled"}
          />
        </div>
      </aside>

      <div className="min-h-0 min-w-0 flex-1 overflow-y-auto px-5 py-5 sm:px-7 sm:py-6 lg:px-9 lg:py-7">
        <div className="mx-auto max-w-3xl space-y-5">
        {message && (
          <div className="rounded-lg border border-(--color-success)/30 bg-(--color-success)/10 px-3 py-2 text-sm text-(--color-success)">
            {message}
          </div>
        )}
        {formError && <ErrorState message={formError} />}

        {tab === "general" && (
          <section className="space-y-5">
            <SectionHeader
              eyebrow="Project profile"
              title="Identity & branding"
              description="Control how this Conductor project appears to members and connected EvoFlux installations."
            />
            <SettingsCard>
              <div className="grid gap-4 sm:grid-cols-2">
                <Field
                  label="Project key"
                  hint="Stable short name used in project identity and client registration."
                >
                  <Input
                    value={projectName}
                    onChange={(e) => setProjectName(e.target.value)}
                  />
                </Field>
                <Field
                  label="Display name"
                  hint="Human-friendly name shown in the app shell and sign-in screen."
                >
                  <Input
                    value={displayName}
                    onChange={(e) => setDisplayName(e.target.value)}
                  />
                </Field>
              </div>
              <Field
                label="Project description"
                hint="Shown to members and sent to registered EvoFlux clients. Maximum 500 characters."
              >
                <Textarea
                  value={projectDescription}
                  onChange={(event) =>
                    setProjectDescription(event.target.value)
                  }
                  maxLength={500}
                  rows={3}
                  placeholder="Describe this project's purpose, audience, and governed resources."
                  className="resize-y"
                />
                <div className="text-right text-[11px] tabular-nums text-(--color-text-subtle)">
                  {projectDescription.length}/500
                </div>
              </Field>
            </SettingsCard>
            <SettingsCard>
              <Field
                label="Project logo"
                hint="PNG, JPG or WebP up to 512 KB. The binary stays in object storage; SQL stores only its key and digest."
              >
              <div className="flex items-center gap-5 pt-1">
                <span className="grid size-20 shrink-0 place-items-center overflow-hidden rounded-2xl bg-(--bg-key) shadow-[0_3px_14px_-4px_rgba(0,0,0,0.45)]">
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
                      disabled={uploadLogo.isPending}
                      onClick={() => logoFileRef.current?.click()}
                    >
                      Upload logo
                    </Button>
                    {logoUrl && (
                      <Button
                        variant="ghost"
                        disabled={deleteLogo.isPending}
                        onClick={() => deleteLogo.mutate()}
                      >
                        Remove
                      </Button>
                    )}
                  </div>
                </div>
              </div>
              <input
                ref={logoFileRef}
                type="file"
                accept="image/png,image/jpeg,image/webp"
                className="hidden"
                onChange={onLogoFile}
              />
              </Field>
            </SettingsCard>
            <ActionRow>
              <Button
                variant="gradient"
                disabled={!projectName.trim() || saveProject.isPending}
                onClick={() => saveProject.mutate()}
              >
                Save project profile
              </Button>
            </ActionRow>
          </section>
        )}

        {tab === "network" && (
          <section className="space-y-5">
            <SectionHeader
              eyebrow="Connectivity"
              title="Network & realtime delivery"
              description="Configure the public endpoint and connection limits used by browser sessions and EvoFlux subscribers."
            />
            <SettingsCard>
            <div className="grid gap-4 sm:grid-cols-2">
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
            </SettingsCard>

            <SettingsCard title="Realtime (SSE)" description="Limits are enforced per Conductor instance and connection secret.">
            <div className="grid gap-4 sm:grid-cols-3">
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
            </SettingsCard>
            <ActionRow>
              <Button
                variant="gradient"
                disabled={saveNetwork.isPending}
                onClick={() => saveNetwork.mutate()}
              >
                Save connectivity
              </Button>
            </ActionRow>
          </section>
        )}

        {tab === "storage" && (
          <section className="space-y-5">
            <SectionHeader
              eyebrow="File data plane"
              title="Resource object storage"
              description="Drafts and immutable releases are content-addressed ZIP objects. SQL contains only keys, hashes and manifests."
              trailing={<Badge tone="neutral">{data.storage.backend.replace("_", " ")}</Badge>}
            />

            <SettingsCard>
            <Field label="Backend">
              <Select
                value={storageBackend}
                onValueChange={(value) => setStorageBackend(value as StorageBackend)}
                options={[
                  { value: "local", label: "Local filesystem" },
                  { value: "s3", label: "Amazon S3 / compatible" },
                  { value: "azure_blob", label: "Azure Blob Storage" },
                  { value: "git", label: "Git repository" },
                ]}
              />
            </Field>

            {storageBackend === "local" && (
              <Field label="Object root">
                <Input
                  value={localRoot}
                  onChange={(event) => setLocalRoot(event.target.value)}
                  placeholder="objects (relative to CONDUCTOR_DATA_DIR)"
                />
                <p className="mt-1 text-xs text-(--color-text-subtle)">
                  Leave blank for CONDUCTOR_DATA_DIR/objects. Existing objects are copied and SHA-256 verified before switching.
                </p>
              </Field>
            )}

            {storageBackend === "s3" && (
              <div className="space-y-3">
                <div className="grid gap-3 sm:grid-cols-2">
                  <Field label="Bucket">
                    <Input value={s3Bucket} onChange={(event) => setS3Bucket(event.target.value)} />
                  </Field>
                  <Field label="Region">
                    <Input value={s3Region} onChange={(event) => setS3Region(event.target.value)} placeholder="ap-southeast-1" />
                  </Field>
                  <Field label="Endpoint (optional)">
                    <Input value={s3Endpoint} onChange={(event) => setS3Endpoint(event.target.value)} placeholder="https://s3.example.com" />
                  </Field>
                  <Field label="Object prefix">
                    <Input value={s3Prefix} onChange={(event) => setS3Prefix(event.target.value)} placeholder="conductor/project" />
                  </Field>
                </div>
                <SwitchField
                  id="s3-path-style"
                  label="Path-style requests"
                  description="Enable for MinIO and S3-compatible endpoints that do not support virtual-hosted buckets."
                  checked={s3PathStyle}
                  onCheckedChange={setS3PathStyle}
                />
                <CredentialNotice>
                  Credentials come from the AWS credential chain (IAM role, workload identity, AWS_ACCESS_KEY_ID/AWS_SECRET_ACCESS_KEY). Secrets are never saved in Conductor SQL.
                </CredentialNotice>
              </div>
            )}

            {storageBackend === "azure_blob" && (
              <div className="space-y-3">
                <div className="grid gap-3 sm:grid-cols-2">
                  <Field label="Storage account">
                    <Input value={azureAccount} onChange={(event) => setAzureAccount(event.target.value)} />
                  </Field>
                  <Field label="Container">
                    <Input value={azureContainer} onChange={(event) => setAzureContainer(event.target.value)} />
                  </Field>
                  <Field label="Endpoint (optional)">
                    <Input value={azureEndpoint} onChange={(event) => setAzureEndpoint(event.target.value)} placeholder="https://account.blob.core.windows.net" />
                  </Field>
                  <Field label="Object prefix">
                    <Input value={azurePrefix} onChange={(event) => setAzurePrefix(event.target.value)} placeholder="conductor/project" />
                  </Field>
                </div>
                <CredentialNotice>
                  Credentials come from the Azure credential chain (managed identity, workload identity, AZURE_STORAGE_ACCOUNT_KEY or SAS). Secret values are not part of project settings.
                </CredentialNotice>
              </div>
            )}

            {storageBackend === "git" && (
              <div className="space-y-4">
                <Field
                  label="Repository URL"
                  hint="HTTPS, SSH, SCP syntax or a mounted absolute repository path. Never embed a token in this URL."
                >
                  <Input
                    value={gitRepositoryUrl}
                    onChange={(event) =>
                      setGitRepositoryUrl(event.target.value)
                    }
                    placeholder="https://git.example.com/team/resources.git"
                  />
                </Field>
                <div className="grid gap-3 sm:grid-cols-2">
                  <Field label="Branch">
                    <Input
                      value={gitBranch}
                      onChange={(event) => setGitBranch(event.target.value)}
                      placeholder="main"
                    />
                  </Field>
                  <Field
                    label="Object prefix"
                    hint="Directory inside the repository."
                  >
                    <Input
                      value={gitPrefix}
                      onChange={(event) => setGitPrefix(event.target.value)}
                      placeholder="evo-conductor/objects"
                    />
                  </Field>
                </div>
                <Field label="Authentication">
                  <Select
                    value={gitAuthMode}
                    onValueChange={(value) =>
                      setGitAuthMode(value as GitAuthMode)
                    }
                    options={[
                      {
                        value: "environment",
                        label: "SSH agent / credential helper",
                      },
                      {
                        value: "https_token",
                        label: "HTTPS access token",
                      },
                    ]}
                  />
                </Field>
                {gitAuthMode === "https_token" && (
                  <div className="grid gap-3 sm:grid-cols-2">
                    <Field
                      label="Username"
                      hint="Provider-specific; commonly git, oauth2 or x-access-token."
                    >
                      <Input
                        value={gitUsername}
                        onChange={(event) =>
                          setGitUsername(event.target.value)
                        }
                        placeholder="oauth2"
                        autoComplete="off"
                      />
                    </Field>
                    <Field
                      label={
                        gitCredentialMatchesRepository
                          ? "Access token (leave blank to keep)"
                          : "Access token"
                      }
                      hint="Write-only. The API never returns this value."
                    >
                      <Input
                        type="password"
                        value={gitCredential}
                        onChange={(event) => {
                          setGitCredential(event.target.value)
                          setClearGitCredential(false)
                        }}
                        placeholder={
                          gitCredentialMatchesRepository
                            ? "Saved credential"
                            : "Paste access token"
                        }
                        autoComplete="new-password"
                      />
                    </Field>
                  </div>
                )}
                {gitCredentialMatchesRepository && (
                  <div className="flex items-center justify-between gap-3 rounded-lg border border-(--border-soft) bg-(--bg-key)/45 px-3 py-2">
                    <span className="text-xs text-(--color-text-muted)">
                      {clearGitCredential
                        ? "The saved credential will be removed when you save."
                        : "A credential is saved for this repository."}
                    </span>
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={() => {
                        setClearGitCredential((current) => {
                          if (!current) setGitAuthMode("environment")
                          return !current
                        })
                        setGitCredential("")
                      }}
                    >
                      {clearGitCredential ? "Keep credential" : "Remove"}
                    </Button>
                  </div>
                )}
                <CredentialNotice>
                  Git uses a serialized local mirror and pushes
                  content-addressed objects to this branch. HTTPS tokens are
                  stored in a permission-restricted file outside SQL. For
                  multi-replica deployments, prefer a shared SSH agent or
                  workload credential helper.
                </CredentialNotice>
              </div>
            )}
            </SettingsCard>

            <div className="rounded-lg border border-(--color-warning)/25 bg-(--color-warning)/8 px-3 py-2 text-xs text-(--color-text-muted)">
              Saving pauses resource writes, verifies the candidate backend,
              copies every referenced object, checks its digest, persists the
              sanitized setting, then switches atomically.
            </div>
            <ActionRow>
              <Button
                variant="gradient"
                disabled={saveStorage.isPending || gitStorageIsIncomplete}
                onClick={() => saveStorage.mutate()}
              >
                {saveStorage.isPending ? "Migrating objects…" : "Save and migrate storage"}
              </Button>
            </ActionRow>
          </section>
        )}

        {tab === "data-policy" && (
          <section className="space-y-5">
            <SectionHeader
              eyebrow="Client policy"
              title="Data collection & privacy"
              description="Choose the project-wide telemetry contract advertised to every EvoFlux installation. Changes apply to new registrations and the ingestion gate immediately."
              trailing={<Badge tone={collectionLevel === "L0" ? "neutral" : "success"}>{collectionLevel}</Badge>}
            />
            <div className="grid gap-3">
              <PolicyOption
                level="L0"
                title="Collection off"
                description="Disable client usage telemetry. Resource delivery, heartbeat and inventory remain available."
                selected={collectionLevel === "L0"}
                onSelect={() => setCollectionLevel("L0")}
              />
              <PolicyOption
                level="L1"
                title="Operational metadata"
                description="Collect request outcome, latency, tokens and resource attribution without prompts, responses or tool arguments."
                selected={collectionLevel === "L1"}
                recommended
                onSelect={() => setCollectionLevel("L1")}
              />
              <PolicyOption
                level="L2"
                title="Extended diagnostics"
                description="Reserve the richer privacy-safe contract for detailed model, tool and failure analysis. Sensitive content remains excluded."
                selected={collectionLevel === "L2"}
                onSelect={() => setCollectionLevel("L2")}
              />
            </div>
            <CredentialNotice>
              Conductor never requests prompt text, model responses, tool arguments, credentials or local file contents through this policy.
            </CredentialNotice>
            <ActionRow>
              <Button
                variant="gradient"
                disabled={saveDataPolicy.isPending}
                onClick={() => saveDataPolicy.mutate()}
              >
                Save data policy
              </Button>
            </ActionRow>
          </section>
        )}

        {tab === "sso" && (
          <section className="space-y-5">
            <SectionHeader
              eyebrow="Authentication"
              title="Single sign-on"
              description="Connect an OpenID Connect identity provider. New SSO identities enter the pending approval queue unless they match an invited member."
              trailing={<Badge tone={data.sso.enabled ? "success" : "neutral"}><StatusDot tone={data.sso.enabled ? "success" : "neutral"} />{data.sso.enabled ? "Enabled" : "Disabled"}</Badge>}
            />
            <SettingsCard>
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
            <Field
              label="OIDC scopes"
              hint="Space- or comma-separated. Keep openid, profile and email unless your provider requires additional claims."
            >
              <Input
                disabled={!ssoEnabled}
                value={scopes}
                placeholder="openid profile email"
                onChange={(e) => setScopes(e.target.value)}
              />
            </Field>
            {ssoEnabled && (
              <p className="rounded-lg border border-(--color-border) bg-(--bg-key)/50 px-3 py-2 text-xs text-(--color-text-muted)">
                ID tokens are verified against the provider JWKS, issuer,
                audience, and nonce. The redirect URI must match the provider
                registration exactly.
              </p>
            )}
            </SettingsCard>
            <ActionRow>
              <Button
                variant="gradient"
                disabled={saveSso.isPending}
                onClick={() => saveSso.mutate()}
              >
                Save authentication
              </Button>
            </ActionRow>
          </section>
        )}
        </div>
      </div>
    </div>
  )
}

function Field({
  label,
  hint,
  children,
}: {
  label: string
  hint?: string
  children: React.ReactNode
}) {
  return (
    <div className="space-y-1.5">
      <Label>{label}</Label>
      {children}
      {hint && (
        <p className="text-xs leading-relaxed text-(--color-text-subtle)">
          {hint}
        </p>
      )}
    </div>
  )
}

function SectionHeader({
  eyebrow,
  title,
  description,
  trailing,
}: {
  eyebrow: string
  title: string
  description: string
  trailing?: React.ReactNode
}) {
  return (
    <header className="flex items-start justify-between gap-4 border-b border-(--border-soft) pb-5">
      <div className="min-w-0">
        <p className="text-[11px] font-semibold tracking-[0.14em] text-(--color-accent) uppercase">
          {eyebrow}
        </p>
        <h3 className="mt-1 text-xl font-semibold tracking-tight">{title}</h3>
        <p className="mt-1.5 max-w-2xl text-sm leading-relaxed text-(--color-text-muted)">
          {description}
        </p>
      </div>
      {trailing && <div className="shrink-0 pt-1">{trailing}</div>}
    </header>
  )
}

function SettingsCard({
  title,
  description,
  children,
}: {
  title?: string
  description?: string
  children: React.ReactNode
}) {
  return (
    <div className="space-y-4 rounded-xl border border-(--border-soft) bg-(--bg-card)/65 p-4 sm:p-5">
      {(title || description) && (
        <div>
          {title && <h4 className="text-sm font-semibold">{title}</h4>}
          {description && (
            <p className="mt-1 text-xs leading-relaxed text-(--color-text-subtle)">
              {description}
            </p>
          )}
        </div>
      )}
      {children}
    </div>
  )
}

function ActionRow({ children }: { children: React.ReactNode }) {
  return (
    <div className="flex justify-end border-t border-(--border-soft) pt-4">
      {children}
    </div>
  )
}

function PolicyOption({
  level,
  title,
  description,
  selected,
  recommended = false,
  onSelect,
}: {
  level: CollectionLevel
  title: string
  description: string
  selected: boolean
  recommended?: boolean
  onSelect: () => void
}) {
  return (
    <button
      type="button"
      aria-pressed={selected}
      onClick={onSelect}
      className={cn(
        "flex w-full items-start gap-4 rounded-xl border p-4 text-left transition-colors",
        selected
          ? "border-(--color-accent)/55 bg-(--color-accent)/8 ring-1 ring-(--color-accent)/20"
          : "border-(--border-soft) bg-(--bg-card)/65 hover:border-(--color-text-subtle)/40 hover:bg-(--bg-key)/60",
      )}
    >
      <span
        className={cn(
          "mt-0.5 grid size-9 shrink-0 place-items-center rounded-lg font-mono text-sm font-semibold",
          selected
            ? "bg-(--color-accent) text-white"
            : "bg-(--bg-key) text-(--color-text-muted)",
        )}
      >
        {level}
      </span>
      <span className="min-w-0 flex-1">
        <span className="flex flex-wrap items-center gap-2 text-sm font-semibold">
          {title}
          {recommended && <Badge tone="info">Recommended</Badge>}
        </span>
        <span className="mt-1 block text-xs leading-relaxed text-(--color-text-muted)">
          {description}
        </span>
      </span>
      <span
        aria-hidden="true"
        className={cn(
          "mt-2 size-3.5 rounded-full border",
          selected
            ? "border-(--color-accent) bg-(--color-accent) shadow-[inset_0_0_0_3px_var(--bg-card)]"
            : "border-(--color-text-subtle)",
        )}
      />
    </button>
  )
}

function SettingStatus({
  icon: Icon,
  label,
  value,
}: {
  icon: typeof Database
  label: string
  value: string
}) {
  return (
    <div className="flex items-center gap-2.5 px-1 text-xs">
      <Icon className="size-3.5 text-(--color-text-subtle)" strokeWidth={1.7} />
      <span className="text-(--color-text-subtle)">{label}</span>
      <span className="ml-auto font-medium text-(--color-text-muted) capitalize">
        {value}
      </span>
    </div>
  )
}

function CredentialNotice({ children }: { children: React.ReactNode }) {
  return (
    <p className="rounded-lg border border-(--color-border) bg-(--bg-key)/50 px-3 py-2 text-xs text-(--color-text-muted)">
      {children}
    </p>
  )
}
