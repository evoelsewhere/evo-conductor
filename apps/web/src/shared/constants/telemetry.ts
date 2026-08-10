import { MILLISECONDS_PER_DAY, MILLISECONDS_PER_SECOND } from "@/shared/constants/time"

export const TelemetryEventType = {
  ModelCall: "model_call",
  ToolCall: "tool_call",
} as const
export type TelemetryEventType =
  (typeof TelemetryEventType)[keyof typeof TelemetryEventType]

export const TelemetryEventStatus = {
  Success: "success",
  Error: "error",
  Blocked: "blocked",
} as const
export type TelemetryEventStatus =
  (typeof TelemetryEventStatus)[keyof typeof TelemetryEventStatus]

export const TelemetryToolCategory = {
  Mcp: "mcp",
  Filesystem: "filesystem",
  Web: "web",
  VersionControl: "version_control",
  Collaboration: "collaboration",
  Other: "other",
} as const
export type TelemetryToolCategory =
  (typeof TelemetryToolCategory)[keyof typeof TelemetryToolCategory]

export const UsageRangePreset = {
  Day: "day",
  Week: "week",
  Month: "month",
  Custom: "custom",
} as const
export type UsageRangePreset =
  (typeof UsageRangePreset)[keyof typeof UsageRangePreset]

export const TELEMETRY_STATUS_FILTER_ALL = "all" as const
export type TelemetryStatusFilter =
  | typeof TELEMETRY_STATUS_FILTER_ALL
  | TelemetryEventStatus

export const TELEMETRY_STATUS_OPTIONS = [
  { value: TELEMETRY_STATUS_FILTER_ALL, label: "Any status" },
  { value: TelemetryEventStatus.Success, label: "Success" },
  { value: TelemetryEventStatus.Error, label: "Error" },
  { value: TelemetryEventStatus.Blocked, label: "Blocked" },
] as const

export const USAGE_RANGE_PRESET_OPTIONS = [
  UsageRangePreset.Day,
  UsageRangePreset.Week,
  UsageRangePreset.Month,
] as const

export const USAGE_RANGE_DAYS: Record<
  Exclude<UsageRangePreset, typeof UsageRangePreset.Custom>,
  number
> = {
  [UsageRangePreset.Day]: 1,
  [UsageRangePreset.Week]: 7,
  [UsageRangePreset.Month]: 30,
}

export const DEFAULT_USAGE_RANGE_PRESET = UsageRangePreset.Month
export const DEFAULT_CUSTOM_RANGE_DAYS = USAGE_RANGE_DAYS[UsageRangePreset.Month]
export const TELEMETRY_ACTIVITY_PAGE_SIZE = 100
export const TELEMETRY_RECENT_ACTIVITY_LIMIT = 5
export const TELEMETRY_TOP_TOOLS_LIMIT = 10
export const TELEMETRY_PERCENT_SCALE = 100
export const TOKEN_MILLION = 1_000_000
export const TOKEN_THOUSAND = 1_000
export const TOKEN_COMPACT_INTEGER_THRESHOLD = 100_000
export const DURATION_SECOND_MS = MILLISECONDS_PER_SECOND
export const DURATION_INTEGER_SECONDS_THRESHOLD_MS = 10 * MILLISECONDS_PER_SECOND
export { MILLISECONDS_PER_DAY }

export const TELEMETRY_CHART_SERIES = [
  "var(--chart-series-1)",
  "var(--chart-series-2)",
  "var(--chart-series-3)",
  "var(--chart-series-4)",
  "var(--chart-series-5)",
  "var(--chart-series-6)",
] as const

export const TELEMETRY_STATUS_TONES = {
  [TelemetryEventStatus.Success]: "success",
  [TelemetryEventStatus.Error]: "danger",
  [TelemetryEventStatus.Blocked]: "warning",
} as const

export const TELEMETRY_FALLBACK_LABELS = {
  agent: "agent",
  model: "Unknown model",
  modelName: "Unknown",
  modelIdentifier: "model",
  provider: "Unknown provider",
  providerName: "Unknown",
  tool: "Unknown tool",
} as const

export const TELEMETRY_QUERY_KEYS = {
  summary: (userId: string, from?: string, to?: string) =>
    ["member-usage", userId, from, to] as const,
  activity: (userId: string, from?: string, to?: string, limit?: number) =>
    ["member-activity", userId, from, to, limit] as const,
  request: (userId: string, requestId: string) =>
    ["member-request", userId, requestId] as const,
  tools: (userId: string, from?: string, to?: string) =>
    ["member-tools", userId, from, to] as const,
} as const
