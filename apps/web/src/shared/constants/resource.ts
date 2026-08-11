export const RESOURCE_KIND = {
  AGENT: "agent",
  SKILL: "skill",
  PLUGIN: "plugin",
  WORKFLOW: "workflow",
  COMMAND: "command",
} as const

export type ResourceKind = (typeof RESOURCE_KIND)[keyof typeof RESOURCE_KIND]

export const RESOURCE_STATUS = {
  DRAFT: "draft",
  BETA: "beta",
  PUBLISHED: "published",
  ARCHIVED: "archived",
} as const

export type ResourceStatus = (typeof RESOURCE_STATUS)[keyof typeof RESOURCE_STATUS]

export const RELEASE_CHANNEL = {
  BETA: "beta",
  PUBLISHED: "published",
} as const

export type ReleaseChannel = (typeof RELEASE_CHANNEL)[keyof typeof RELEASE_CHANNEL]

export const VERSION_MODE = {
  AUTO: "auto",
  MANUAL: "manual",
} as const

export type VersionMode = (typeof VERSION_MODE)[keyof typeof VERSION_MODE]

export const RESOURCE_KIND_LABEL: Record<ResourceKind, string> = {
  [RESOURCE_KIND.AGENT]: "Agent",
  [RESOURCE_KIND.SKILL]: "Skill",
  [RESOURCE_KIND.PLUGIN]: "Plugin",
  [RESOURCE_KIND.WORKFLOW]: "Workflow",
  [RESOURCE_KIND.COMMAND]: "Command",
}

export const RESOURCE_KIND_OPTIONS = [
  { value: RESOURCE_KIND.AGENT, label: "Agents" },
  { value: RESOURCE_KIND.SKILL, label: "Skills" },
  { value: RESOURCE_KIND.PLUGIN, label: "Plugins" },
  { value: RESOURCE_KIND.WORKFLOW, label: "Workflows" },
  { value: RESOURCE_KIND.COMMAND, label: "Commands" },
] as const

export const RESOURCE_QUERY_KEY = "resources" as const
export const RESOURCE_IMPORT_ACCEPT = ".zip,application/zip,application/x-zip-compressed" as const
