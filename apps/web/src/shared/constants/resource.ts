export const RESOURCE_KIND = {
  AGENT: "agent",
  SKILL: "skill",
  PLUGIN: "plugin",
  WORKFLOW: "workflow",
  COMMAND: "command",
} as const

export type ResourceKind = (typeof RESOURCE_KIND)[keyof typeof RESOURCE_KIND]

export const RESOURCE_TARGET_MODE = {
  WORK: "work",
  CODING: "coding",
} as const

export type ResourceTargetMode =
  (typeof RESOURCE_TARGET_MODE)[keyof typeof RESOURCE_TARGET_MODE]

export const RESOURCE_TARGET_MODES = [
  RESOURCE_TARGET_MODE.WORK,
  RESOURCE_TARGET_MODE.CODING,
] as const

export const RESOURCE_MODE_SCOPE_FILENAME = ".evoflux.json" as const

export const RESOURCE_STATUS = {
  DRAFT: "draft",
  BETA: "beta",
  PUBLISHED: "published",
  ARCHIVED: "archived",
} as const

export type ResourceStatus = (typeof RESOURCE_STATUS)[keyof typeof RESOURCE_STATUS]

export const RESOURCE_VERSION_STATUS = {
  DRAFT: "draft",
  BETA: "beta",
  PUBLISHED: "published",
  DEPRECATED: "deprecated",
} as const

export type ResourceVersionStatus =
  (typeof RESOURCE_VERSION_STATUS)[keyof typeof RESOURCE_VERSION_STATUS]

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
export const RESOURCE_VERSION_REASON_MAX_LENGTH = 500
export const RESOURCE_IMPORT_ACCEPT = ".zip,application/zip,application/x-zip-compressed" as const
export const RESOURCE_INITIAL_VERSION = "0.1.0" as const
export const RESOURCE_INITIAL_CHANGELOG = "Initial template" as const
export const RESOURCE_ARCHIVE_ACCEPT = RESOURCE_IMPORT_ACCEPT
export const RESOURCE_ARCHIVE_EXTENSIONS = [".zip"] as const
export const RESOURCE_ARCHIVE_MAX_BYTES = 20 * 1024 * 1024

export const RESOURCE_CREATE_MODE = {
  UPLOAD: "upload",
  TEMPLATE: "template",
} as const

export type ResourceCreateMode =
  (typeof RESOURCE_CREATE_MODE)[keyof typeof RESOURCE_CREATE_MODE]

export const RESOURCE_CREATE_COPY: Record<
  ResourceKind,
  {
    title: string
    description: string
    templateTitle: string
    templateDescription: string
    sourceHint: string
    createLabel: string
    discardMessage: string
  }
> = {
  [RESOURCE_KIND.AGENT]: {
    title: "Add agent",
    description: "Import an EvoFlux Agent ZIP or start from its Markdown authoring contract.",
    templateTitle: "EvoFlux Agent Markdown",
    templateDescription: "Creates one root Markdown file with name, member role, description, and a system prompt body.",
    sourceHint: "<slug>.md",
    createLabel: "Create agent draft",
    discardMessage: "Discard this agent draft setup?",
  },
  [RESOURCE_KIND.SKILL]: {
    title: "Add skill",
    description: "Import an EvoFlux Skill ZIP or start from its portable bundle contract.",
    templateTitle: "EvoFlux portable Skill bundle",
    templateDescription: "Creates SKILL.md plus EvoFlux UI metadata and balanced trigger-eval starters; references, scripts, and assets remain optional.",
    sourceHint: "SKILL.md + agents/ + evals/",
    createLabel: "Create skill draft",
    discardMessage: "Discard this skill draft setup?",
  },
  [RESOURCE_KIND.PLUGIN]: {
    title: "Add plugin template",
    description: "Start with an editable Portable Agent Plugin template in Resource Studio.",
    templateTitle: "Agent Plugins 1.0 starter",
    templateDescription: "Creates plugin.json and a starter Skill. Use the Plugins page to import a package.",
    sourceHint: "plugin.json + skills/",
    createLabel: "Create plugin draft",
    discardMessage: "Discard this plugin draft setup?",
  },
  [RESOURCE_KIND.WORKFLOW]: {
    title: "Add workflow",
    description: "Create a governed Workflow draft and continue authoring it in Resource Studio.",
    templateTitle: "Workflow starter",
    templateDescription: "Creates an editable JSON source file scoped to this project.",
    sourceHint: "<slug>.json",
    createLabel: "Create workflow draft",
    discardMessage: "Discard this workflow draft setup?",
  },
  [RESOURCE_KIND.COMMAND]: {
    title: "Add command",
    description: "Create a governed Command draft and continue authoring it in Resource Studio.",
    templateTitle: "Command starter",
    templateDescription: "Creates an editable JSON source file scoped to this project.",
    sourceHint: "<slug>.json",
    createLabel: "Create command draft",
    discardMessage: "Discard this command draft setup?",
  },
}

export const PLUGIN_CREATE_MODE = {
  UPLOAD: "upload",
  TEMPLATE: "template",
} as const

export type PluginCreateMode = (typeof PLUGIN_CREATE_MODE)[keyof typeof PLUGIN_CREATE_MODE]

export const PLUGIN_ARCHIVE_ACCEPT =
  ".zip,.evoplugin,application/zip,application/x-zip-compressed" as const
export const PLUGIN_ARCHIVE_EXTENSIONS = [".zip", ".evoplugin"] as const
export const PLUGIN_ARCHIVE_MAX_BYTES = 20 * 1024 * 1024
export const PLUGIN_INITIAL_VERSION = "0.1.0" as const
export const PLUGIN_IMPORT_CHANGELOG = "Initial plugin package" as const

export const RESOURCE_VISIBILITY_OPTIONS = [
  { value: "shared", label: "Shared — all members by default" },
  { value: "private", label: "Private — owner only by default" },
] as const
