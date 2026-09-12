import type { DraftFile } from "@/shared/api/client"

export const TEAM_MANIFEST_PATH = "team.json"
export const TEAM_AGENT_DIR = "agents/"
/** EvoFlux's placeholder token; accepted by Conductor but not resolved in a governed file. */
export const PROVIDER_MODEL_TOKEN = "__PROVIDER_MODEL__"

/** Frontmatter keys the roster form owns; anything else is preserved verbatim. */
const MANAGED_KEYS = [
  "name",
  "role",
  "lead",
  "model",
  "thinking_level",
  "description",
  "skills",
  "mcp",
  "tools",
  "tools_opt_out",
] as const

/** Capability lists the roster form edits, in the order they are written back. */
export const CAPABILITY_FIELDS = ["skills", "mcp", "tools", "tools_opt_out"] as const
export type CapabilityField = (typeof CAPABILITY_FIELDS)[number]

export type TeamAgentRole = "lead" | "member"

export type TeamAgent = {
  path: string
  name: string
  role: TeamAgentRole
  lead: string | null
  model: string
  thinkingLevel: string
  description: string
  /** Capability references, by EvoFlux frontmatter key. */
  capabilities: Record<CapabilityField, string[]>
  /** Frontmatter keys the form does not expose, kept so a save cannot drop them. */
  extraFields: Array<[string, string]>
  body: string
}

export type TeamRoster = {
  lead: TeamAgent | null
  members: TeamAgent[]
  /** Names declared by team.json, used to surface a manifest that has drifted. */
  manifestMembers: string[] | null
  manifestLead: string | null
}

type Frontmatter = { fields: Array<[string, string]>; body: string }

/**
 * Mirror of Conductor's server-side frontmatter reader so the form edits the
 * same document the validator will judge.
 */
function parseFrontmatter(content: string): Frontmatter | null {
  const normalized = content.replace(/\r\n/g, "\n")
  const rest = normalized.trimStart()
  if (!rest.startsWith("---\n")) return null
  const afterOpen = rest.slice(4)
  const closeIndex = afterOpen.indexOf("\n---\n")
  let frontmatter: string
  let body: string
  if (closeIndex >= 0) {
    frontmatter = afterOpen.slice(0, closeIndex)
    body = afterOpen.slice(closeIndex + 5)
  } else if (afterOpen.endsWith("\n---")) {
    frontmatter = afterOpen.slice(0, afterOpen.length - 4)
    body = ""
  } else {
    return null
  }
  const fields: Array<[string, string]> = []
  for (const line of frontmatter.split("\n")) {
    const trimmed = line.trim()
    if (!trimmed || trimmed.startsWith("#")) continue
    const separator = line.indexOf(":")
    if (separator < 0) continue
    fields.push([line.slice(0, separator).trim(), line.slice(separator + 1).trim()])
  }
  return { fields, body }
}

/**
 * Read a YAML list written either inline (`[a, b]`) or as an indented block.
 *
 * Conductor's own Markdown reader is line-based, so the roster writes the
 * inline form; the block form still has to be read because an author may have
 * typed it by hand in the source editor.
 */
function parseList(raw: string): string[] {
  const value = raw.trim()
  if (!value) return []
  const inner = value.startsWith("[") && value.endsWith("]") ? value.slice(1, -1) : value
  return inner
    .split(/[,\n]/)
    .map((item) => item.trim().replace(/^-\s*/, "").replace(/^["']|["']$/g, ""))
    .filter(Boolean)
}

function serializeList(values: string[]): string {
  return `[${values.join(", ")}]`
}

function agentNameFromPath(path: string): string | null {
  if (!path.startsWith(TEAM_AGENT_DIR) || !path.endsWith(".md")) return null
  const stem = path.slice(TEAM_AGENT_DIR.length, -".md".length)
  return stem && !stem.includes("/") ? stem : null
}

export function isValidAgentName(value: string): boolean {
  return /^[A-Za-z0-9._-]{1,120}$/.test(value)
}

export function readRoster(files: DraftFile[]): TeamRoster {
  const agents: TeamAgent[] = []
  for (const file of files) {
    const name = agentNameFromPath(file.path)
    if (!name) continue
    const parsed = parseFrontmatter(file.content)
    if (!parsed) continue
    const lookup = new Map(parsed.fields)
    const role = lookup.get("role") === "lead" ? "lead" : "member"
    agents.push({
      path: file.path,
      name,
      role,
      lead: lookup.get("lead") ?? null,
      model: lookup.get("model") ?? "",
      thinkingLevel: lookup.get("thinking_level") ?? "",
      description: lookup.get("description") ?? "",
      capabilities: Object.fromEntries(
        CAPABILITY_FIELDS.map((field) => [field, parseList(lookup.get(field) ?? "")]),
      ) as Record<CapabilityField, string[]>,
      extraFields: parsed.fields.filter(
        ([key]) => !MANAGED_KEYS.includes(key as (typeof MANAGED_KEYS)[number]),
      ),
      body: parsed.body,
    })
  }
  agents.sort((left, right) => left.name.localeCompare(right.name))

  let manifestLead: string | null = null
  let manifestMembers: string[] | null = null
  const manifest = files.find((file) => file.path === TEAM_MANIFEST_PATH)
  if (manifest) {
    try {
      const parsed = JSON.parse(manifest.content) as {
        lead?: unknown
        members?: unknown
      }
      if (typeof parsed.lead === "string") manifestLead = parsed.lead
      if (Array.isArray(parsed.members)) {
        manifestMembers = parsed.members.filter(
          (item): item is string => typeof item === "string",
        )
      }
    } catch {
      manifestLead = null
      manifestMembers = null
    }
  }

  return {
    lead: agents.find((agent) => agent.role === "lead") ?? null,
    members: agents.filter((agent) => agent.role !== "lead"),
    manifestLead,
    manifestMembers,
  }
}

export function serializeAgent(agent: TeamAgent): string {
  const fields: Array<[string, string]> = [
    ["name", agent.name],
    ["role", agent.role],
  ]
  if (agent.role === "member" && agent.lead) fields.push(["lead", agent.lead])
  if (agent.model) fields.push(["model", agent.model])
  if (agent.thinkingLevel) fields.push(["thinking_level", agent.thinkingLevel])
  if (agent.description) fields.push(["description", agent.description])
  for (const field of CAPABILITY_FIELDS) {
    const values = agent.capabilities[field]
    if (values.length > 0) fields.push([field, serializeList(values)])
  }
  fields.push(...agent.extraFields)
  const frontmatter = fields.map(([key, value]) => `${key}: ${value}`).join("\n")
  const body = agent.body.replace(/^\n+/, "")
  return `---\n${frontmatter}\n---\n\n${body}`
}

export function agentPath(name: string): string {
  return `${TEAM_AGENT_DIR}${name}.md`
}

export function newMemberAgent(name: string, leadName: string, teamLabel: string): TeamAgent {
  return {
    path: agentPath(name),
    name,
    role: "member",
    // Written for the author: a member without this joins the installation's
    // default lead instead of this team, which EvoFlux cannot report.
    lead: leadName,
    // Deliberately blank. EvoFlux treats __PROVIDER_MODEL__ as unconfigured and
    // its placeholder backfill rewrites the file in place, which would break the
    // managed copy's integrity check on the next sync.
    model: "",
    thinkingLevel: "",
    description: `Focused specialist for ${teamLabel}`,
    capabilities: { skills: [], mcp: [], tools: [], tools_opt_out: [] },
    extraFields: [],
    body: `You are a focused specialist on "${teamLabel}".\n\n## Responsibilities\n\n- Define the work this member owns.\n- State its boundaries and hand-off conditions.\n`,
  }
}

/**
 * Rebuild team.json from the Agent files rather than editing it in place, so
 * the manifest cannot drift out of agreement with the roster it describes.
 */
export function serializeManifest(leadName: string, memberNames: string[]): string {
  return `${JSON.stringify({ lead: leadName, members: [...memberNames].sort() }, null, 2)}\n`
}
