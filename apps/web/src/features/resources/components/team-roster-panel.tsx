import { useMutation, useQuery } from "@tanstack/react-query"
import { AlertTriangle, Crown, Plus, Trash2, Users, X } from "lucide-react"
import { useEffect, useState } from "react"

import { api, type DraftFileTree } from "@/shared/api/client"
import {
  CAPABILITY_FIELDS,
  agentPath,
  isValidAgentName,
  newMemberAgent,
  readRoster,
  serializeAgent,
  serializeManifest,
  type CapabilityField,
  type TeamAgent,
} from "@/features/resources/lib/team-roster"
import { RESOURCE_KIND, RESOURCE_QUERY_KEY, RESOURCE_STATUS } from "@/shared/constants/resource"
import { Badge } from "@/shared/ui/badge"
import { Button } from "@/shared/ui/button"
import { EmptyState, ErrorState } from "@/shared/ui/empty-state"
import { Input } from "@/shared/ui/input"
import { Label } from "@/shared/ui/label"
import { Textarea } from "@/shared/ui/textarea"

const CAPABILITY_LABEL: Record<CapabilityField, string> = {
  skills: "Skills",
  mcp: "MCP servers",
  tools: "Tools",
  tools_opt_out: "Disabled tools",
}

const CAPABILITY_HINT: Record<CapabilityField, string> = {
  skills: "Assigned skills preload; every other skill stays available on demand.",
  mcp: "Server names declared by the project's published Plugins. Each entry grants this Agent every tool that server exposes.",
  tools: "Built-in EvoFlux tools to add beyond the team defaults.",
  tools_opt_out: "Built-in tools this Agent must not use.",
}

const THINKING_LEVELS = [
  { value: "", label: "Provider default" },
  { value: "minimal", label: "Minimal" },
  { value: "low", label: "Low" },
  { value: "medium", label: "Medium" },
  { value: "high", label: "High" },
  { value: "xhigh", label: "X-High" },
  { value: "max", label: "Max" },
] as const

export function TeamRosterPanel({
  resourceId,
  tree,
  canAuthor,
  teamLabel,
  onTreeChange,
  onDirtyChange,
}: {
  resourceId: string
  tree: DraftFileTree
  canAuthor: boolean
  teamLabel: string
  onTreeChange: (next: DraftFileTree) => void
  /** Lets the page guard a tab switch the same way it guards the source editor. */
  onDirtyChange: (dirty: boolean) => void
}) {
  const [dirtyCards, setDirtyCards] = useState<Record<string, boolean>>({})
  const [newMemberName, setNewMemberName] = useState("")
  const [error, setError] = useState<string | null>(null)
  const roster = readRoster(tree.files)

  /**
   * Turn a failed write into something the author can act on.
   *
   * A revision conflict means someone else advanced this draft, so the panel is
   * holding a stale tree and every later save would fail the same way. Pulling
   * the current draft back in is what actually unblocks them, and the raw
   * "draft revision conflict" string never said that.
   */
  async function reportFailure(cause: unknown, fallback: string) {
    const message = cause instanceof Error ? cause.message : fallback
    if (/revision conflict/i.test(message)) {
      try {
        onTreeChange(await api.resourceDraft(resourceId))
        setError(
          "This draft changed somewhere else, so your edit was not saved. The latest version is now loaded — reapply your change.",
        )
        return
      } catch {
        // fall through to the original message
      }
    }
    setError(message)
  }
  // Offer what this project already governs, so a reference is far more likely
  // to resolve on the installations that receive the team.
  const catalog = useQuery({
    queryKey: [RESOURCE_QUERY_KEY],
    queryFn: () => api.resources(),
  })
  const published = (catalog.data ?? []).filter(
    (item) => item.status === RESOURCE_STATUS.PUBLISHED,
  )
  const suggestions: Record<CapabilityField, string[]> = {
    skills: published
      .filter((item) => item.kind === RESOURCE_KIND.SKILL)
      .map((item) => item.slug),
    // A plugin's slug is not a server name: EvoFlux resolves `mcp:` against the
    // names inside the package, so offer exactly those.
    mcp: published
      .filter((item) => item.kind === RESOURCE_KIND.PLUGIN)
      .flatMap((item) => pluginMcpServerNames(item.payload)),
    tools: [],
    tools_opt_out: [],
  }
  const leadName = roster.lead?.name ?? null
  const memberNames = roster.members.map((member) => member.name)

  const addMember = useMutation({
    mutationFn: async (name: string) => {
      if (!leadName) throw new Error("This team has no lead Agent yet.")
      const agent = newMemberAgent(name, leadName, teamLabel)
      const created = await api.createResourceDraftFile(
        resourceId,
        agent.path,
        serializeAgent(agent),
        tree.revision,
      )
      return api.saveResourceDraftFile(
        resourceId,
        "team.json",
        serializeManifest(leadName, [...memberNames, name]),
        created.revision,
      )
    },
    onSuccess: (next) => {
      setNewMemberName("")
      setError(null)
      onTreeChange(next)
    },
    onError: (cause) => void reportFailure(cause, "Could not add the member."),
  })

  const removeMember = useMutation({
    mutationFn: async (name: string) => {
      if (!leadName) throw new Error("This team has no lead Agent yet.")
      const removed = await api.deleteResourceDraftEntry(
        resourceId,
        agentPath(name),
        tree.revision,
      )
      return api.saveResourceDraftFile(
        resourceId,
        "team.json",
        serializeManifest(
          leadName,
          memberNames.filter((item) => item !== name),
        ),
        removed.revision,
      )
    },
    onSuccess: (next) => {
      setError(null)
      onTreeChange(next)
    },
    onError: (cause) => void reportFailure(cause, "Could not remove the member."),
  })

  const saveAgent = useMutation({
    mutationFn: (agent: TeamAgent) =>
      api.saveResourceDraftFile(
        resourceId,
        agent.path,
        serializeAgent(agent),
        tree.revision,
      ),
    onSuccess: (next) => {
      setError(null)
      onTreeChange(next)
    },
    onError: (cause) => void reportFailure(cause, "Could not save the Agent."),
  })

  useEffect(() => {
    onDirtyChange(Object.values(dirtyCards).some(Boolean))
  }, [dirtyCards, onDirtyChange])
  useEffect(() => () => onDirtyChange(false), [onDirtyChange])

  function trackDirty(path: string, value: boolean) {
    setDirtyCards((current) =>
      current[path] === value ? current : { ...current, [path]: value },
    )
  }

  const busy = addMember.isPending || removeMember.isPending || saveAgent.isPending
  const nameTaken =
    newMemberName === leadName || memberNames.includes(newMemberName)
  const nameInvalid = newMemberName !== "" && !isValidAgentName(newMemberName)
  const manifestDrifted =
    roster.manifestLead !== null &&
    leadName !== null &&
    (roster.manifestLead !== leadName ||
      [...(roster.manifestMembers ?? [])].sort().join(" ") !==
        [...memberNames].sort().join(" "))

  if (!roster.lead) {
    return (
      <ErrorState message="This draft has no Agent with role 'lead' under agents/. Add one in Source & validation before using the roster." />
    )
  }

  return (
    <div className="space-y-4">
      {error && <ErrorState message={error} />}

      {manifestDrifted && (
        <div className="flex items-start gap-2 rounded-lg border border-(--color-warning)/30 bg-(--color-warning)/8 px-3 py-2 text-xs text-(--color-warning)">
          <AlertTriangle className="mt-0.5 size-4 shrink-0" />
          <span>
            team.json disagrees with the Agent files. Adding or removing a member here
            rewrites it to match.
          </span>
        </div>
      )}

      <AgentCard
        agent={roster.lead}
        teamLabel={teamLabel}
        canAuthor={canAuthor}
        busy={busy}
        suggestions={suggestions}
        onDirtyChange={trackDirty}
        onSave={(next) => saveAgent.mutate(next)}
      />

      <section className="rounded-xl border border-(--border-card) bg-(--bg-card) p-4">
        <div className="mb-3 flex items-center gap-2">
          <Users className="size-4 text-(--color-text-muted)" />
          <h2 className="text-sm font-semibold text-(--color-text)">
            Members ({roster.members.length})
          </h2>
        </div>

        {roster.members.length === 0 ? (
          <EmptyState
            title="No members yet"
            description="A team with only a lead still publishes, but the lead has nobody to delegate to."
          />
        ) : (
          <div className="space-y-3">
            {roster.members.map((member) => (
              <AgentCard
                key={member.path}
                agent={member}
                teamLabel={teamLabel}
                canAuthor={canAuthor}
                busy={busy}
                leadName={leadName ?? undefined}
                suggestions={suggestions}
                onDirtyChange={trackDirty}
                onSave={(next) => saveAgent.mutate(next)}
                onRemove={() => {
                  if (
                    window.confirm(`Remove member "${member.name}" from this team?`)
                  ) {
                    removeMember.mutate(member.name)
                  }
                }}
              />
            ))}
          </div>
        )}

        {canAuthor && (
          <div className="mt-4 flex items-end gap-2 border-t border-(--border-soft) pt-4">
            <div className="flex-1">
              <Label htmlFor="new-member">Add member</Label>
              <Input
                id="new-member"
                value={newMemberName}
                placeholder="reviewer"
                onChange={(event) => setNewMemberName(event.target.value.trim())}
              />
              <p className="mt-1 text-[11px] text-(--color-text-muted)">
                Creates agents/&lt;name&gt;.md with lead: {leadName} already set, and
                updates team.json.
              </p>
            </div>
            <Button
              variant="outline"
              disabled={
                busy || !newMemberName || nameInvalid || nameTaken || !canAuthor
              }
              onClick={() => addMember.mutate(newMemberName)}
            >
              <Plus className="size-3.5" />
              Add
            </Button>
          </div>
        )}
        {nameInvalid && (
          <p className="mt-2 text-[11px] text-(--color-danger)">
            Use letters, digits, dot, underscore or hyphen only.
          </p>
        )}
        {nameTaken && newMemberName !== "" && (
          <p className="mt-2 text-[11px] text-(--color-danger)">
            This team already has an Agent named "{newMemberName}".
          </p>
        )}
      </section>
    </div>
  )
}

function AgentCard({
  agent,
  teamLabel,
  canAuthor,
  busy,
  leadName,
  suggestions,
  onDirtyChange,
  onSave,
  onRemove,
}: {
  agent: TeamAgent
  teamLabel: string
  canAuthor: boolean
  busy: boolean
  leadName?: string
  suggestions: Record<CapabilityField, string[]>
  onDirtyChange: (path: string, dirty: boolean) => void
  onSave: (agent: TeamAgent) => void
  onRemove?: () => void
}) {
  const [description, setDescription] = useState(agent.description)
  const [model, setModel] = useState(agent.model)
  const [thinking, setThinking] = useState(agent.thinkingLevel)
  const [body, setBody] = useState(agent.body)
  const [capabilities, setCapabilities] = useState(agent.capabilities)

  const capabilitiesChanged = CAPABILITY_FIELDS.some(
    (field) => capabilities[field].join("\u0000") !== agent.capabilities[field].join("\u0000"),
  )
  const dirty =
    description !== agent.description ||
    model !== agent.model ||
    thinking !== agent.thinkingLevel ||
    body !== agent.body ||
    capabilitiesChanged

  useEffect(() => {
    onDirtyChange(agent.path, dirty)
  }, [agent.path, dirty, onDirtyChange])
  const isLead = agent.role === "lead"
  const leadMismatch = !isLead && leadName !== undefined && agent.lead !== leadName

  return (
    <div className="rounded-xl border border-(--border-card) bg-(--bg-card) p-4">
      <div className="mb-3 flex flex-wrap items-center gap-2">
        {isLead ? (
          <Crown className="size-4 text-(--color-accent)" />
        ) : (
          <Users className="size-4 text-(--color-text-muted)" />
        )}
        <span className="text-sm font-semibold text-(--color-text)">{agent.name}</span>
        <Badge tone={isLead ? "accent" : "neutral"}>{isLead ? "Lead" : "Member"}</Badge>
        {!isLead && agent.lead && (
          <Badge tone={leadMismatch ? "danger" : "neutral"}>
            lead: {agent.lead}
          </Badge>
        )}
        {!isLead && !agent.lead && <Badge tone="danger">lead missing</Badge>}
        <span className="ml-auto font-mono text-[11px] text-(--color-text-muted)">
          {agent.path}
        </span>
      </div>

      {leadMismatch && (
        <p className="mb-3 text-[11px] text-(--color-danger)">
          This member names a lead this team does not define; EvoFlux would attach it
          elsewhere.
        </p>
      )}

      <div className="grid gap-3 sm:grid-cols-2">
        <div>
          <Label htmlFor={`${agent.path}-model`}>Model</Label>
          <Input
            id={`${agent.path}-model`}
            value={model}
            placeholder="provider:model"
            disabled={!canAuthor}
            onChange={(event) => setModel(event.target.value.trim())}
          />
          <p className="mt-1 text-[11px] text-(--color-text-muted)">
            Leave blank to let each installation choose; EvoFlux keeps an Agent with
            no model off its roster until one is set.
          </p>
        </div>
        <div>
          <Label htmlFor={`${agent.path}-thinking`}>Thinking level</Label>
          <select
            id={`${agent.path}-thinking`}
            value={thinking}
            disabled={!canAuthor}
            onChange={(event) => setThinking(event.target.value)}
            className="h-9 w-full rounded-lg border border-(--border-input) bg-(--bg-input) px-2 text-xs text-(--color-text)"
          >
            {THINKING_LEVELS.map((level) => (
              <option key={level.value} value={level.value}>
                {level.label}
              </option>
            ))}
          </select>
        </div>
      </div>

      <div className="mt-3">
        <Label htmlFor={`${agent.path}-description`}>Description</Label>
        <Input
          id={`${agent.path}-description`}
          value={description}
          disabled={!canAuthor}
          onChange={(event) => setDescription(event.target.value)}
        />
      </div>

      <div className="mt-3">
        <Label htmlFor={`${agent.path}-body`}>System prompt</Label>
        <Textarea
          id={`${agent.path}-body`}
          rows={10}
          value={body}
          disabled={!canAuthor}
          onChange={(event) => setBody(event.target.value)}
        />
      </div>

      <section className="mt-4 rounded-lg border border-(--border-soft) p-3">
        <h3 className="mb-2 text-xs font-semibold text-(--color-text)">Capabilities</h3>
        <div className="grid gap-3 sm:grid-cols-2">
          {CAPABILITY_FIELDS.map((field) => (
            <CapabilityEditor
              key={field}
              field={field}
              owner={agent.path}
              values={capabilities[field]}
              suggestions={suggestions[field]}
              disabled={!canAuthor}
              onChange={(next) =>
                setCapabilities((current) => ({ ...current, [field]: next }))
              }
            />
          ))}
        </div>
      </section>

      <div className="mt-3 flex items-center gap-2">
        <Button
          variant="outline"
          disabled={!canAuthor || busy || !dirty}
          onClick={() =>
            onSave({
              ...agent,
              description,
              model,
              thinkingLevel: thinking,
              body,
              capabilities,
            })
          }
        >
          Save {isLead ? "lead" : "member"}
        </Button>
        {onRemove && canAuthor && (
          <Button variant="ghost" disabled={busy} onClick={onRemove}>
            <Trash2 className="size-3.5" />
            Remove
          </Button>
        )}
        {dirty && (
          <span className="text-[11px] text-(--color-text-muted)">
            Unsaved changes for {teamLabel}
          </span>
        )}
      </div>
    </div>
  )
}

function pluginMcpServerNames(payload: unknown): string[] {
  if (!payload || typeof payload !== "object") return []
  const servers = (payload as { mcp_servers?: unknown }).mcp_servers
  return Array.isArray(servers)
    ? servers.filter((item): item is string => typeof item === "string")
    : []
}

function CapabilityEditor({
  field,
  owner,
  values,
  suggestions,
  disabled,
  onChange,
}: {
  field: CapabilityField
  /** Agent path, so every card's inputs get their own id and label target. */
  owner: string
  values: string[]
  suggestions: string[]
  disabled: boolean
  onChange: (next: string[]) => void
}) {
  const [draft, setDraft] = useState("")
  const inputId = `${owner}-${field}-input`
  const unused = suggestions.filter((item) => !values.includes(item))

  function add(name: string) {
    const value = name.trim()
    if (!value || values.includes(value)) return
    onChange([...values, value])
    setDraft("")
  }

  return (
    <div>
      <Label htmlFor={inputId}>{CAPABILITY_LABEL[field]}</Label>
      <div className="mb-1 flex flex-wrap gap-1">
        {values.length === 0 && (
          <span className="text-[11px] text-(--color-text-muted)">None</span>
        )}
        {values.map((value) => (
          <span
            key={value}
            className="inline-flex items-center gap-1 rounded-md border border-(--border-soft) px-1.5 py-0.5 font-mono text-[11px] text-(--color-text)"
          >
            {value}
            {!disabled && (
              <button
                type="button"
                aria-label={`Remove ${value}`}
                onClick={() => onChange(values.filter((item) => item !== value))}
                className="text-(--color-text-muted) hover:text-(--color-danger)"
              >
                <X className="size-3" />
              </button>
            )}
          </span>
        ))}
      </div>
      {!disabled && (
        <>
          <div className="flex gap-1">
            <Input
              id={inputId}
              value={draft}
              placeholder="name"
              disabled={disabled}
              onChange={(event) => setDraft(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === "Enter") {
                  event.preventDefault()
                  add(draft)
                }
              }}
            />
            <Button variant="ghost" disabled={!draft.trim()} onClick={() => add(draft)}>
              <Plus className="size-3.5" />
            </Button>
          </div>
          {unused.length > 0 && (
            <div className="mt-1 flex flex-wrap gap-1">
              {unused.slice(0, 8).map((item) => (
                <button
                  key={item}
                  type="button"
                  onClick={() => add(item)}
                  className="rounded-md border border-dashed border-(--border-soft) px-1.5 py-0.5 font-mono text-[11px] text-(--color-text-muted) hover:text-(--color-text)"
                >
                  + {item}
                </button>
              ))}
            </div>
          )}
        </>
      )}
      <p className="mt-1 text-[11px] text-(--color-text-muted)">
        {CAPABILITY_HINT[field]}
      </p>
    </div>
  )
}
