export const SECRET_SCOPE = {
  SUBSCRIBE_RESOURCES: "subscribe_resources",
  REPORT_TELEMETRY: "report_telemetry",
  SYNC_INVENTORY: "sync_inventory",
} as const

export type SecretScope = (typeof SECRET_SCOPE)[keyof typeof SECRET_SCOPE]

export const CONNECTION_SECRET_SCOPES: SecretScope[] = [
  SECRET_SCOPE.SUBSCRIBE_RESOURCES,
  SECRET_SCOPE.REPORT_TELEMETRY,
  SECRET_SCOPE.SYNC_INVENTORY,
]

export const SECRET_SCOPE_OPTIONS: Array<{
  value: SecretScope
  label: string
  description: string
}> = [
  {
    value: SECRET_SCOPE.SUBSCRIBE_RESOURCES,
    label: "Subscribe resources",
    description: "Pull shared Agents, Skills, Plugins, and workflows.",
  },
  {
    value: SECRET_SCOPE.REPORT_TELEMETRY,
    label: "Report telemetry",
    description: "Send usage and performance events to Conductor.",
  },
  {
    value: SECRET_SCOPE.SYNC_INVENTORY,
    label: "Sync inventory",
    description: "Synchronize the member's local EvoFlux inventory.",
  },
]
