export const PRIMARY_ROLE = {
  ADMIN: "admin",
  CONTRIBUTE: "contribute",
  USER: "user",
} as const

export type PrimaryRole = (typeof PRIMARY_ROLE)[keyof typeof PRIMARY_ROLE]

export const PRIMARY_ROLE_LABELS: Record<PrimaryRole, string> = {
  [PRIMARY_ROLE.ADMIN]: "Admin",
  [PRIMARY_ROLE.CONTRIBUTE]: "Contributor",
  [PRIMARY_ROLE.USER]: "User",
}

export const USER_STATUS = {
  PENDING: "pending",
  INVITED: "invited",
  ACTIVE: "active",
  DISABLED: "disabled",
} as const

export type UserStatus = (typeof USER_STATUS)[keyof typeof USER_STATUS]

export const CLIENT_PLATFORM = {
  MACOS: "macos",
  LINUX: "linux",
  WINDOWS: "windows",
} as const

export type ClientPlatform = (typeof CLIENT_PLATFORM)[keyof typeof CLIENT_PLATFORM]

export const PRIMARY_ROLE_OPTIONS = [
  { value: PRIMARY_ROLE.ADMIN, label: PRIMARY_ROLE_LABELS[PRIMARY_ROLE.ADMIN] },
  {
    value: PRIMARY_ROLE.CONTRIBUTE,
    label: PRIMARY_ROLE_LABELS[PRIMARY_ROLE.CONTRIBUTE],
  },
  { value: PRIMARY_ROLE.USER, label: PRIMARY_ROLE_LABELS[PRIMARY_ROLE.USER] },
] as const

export const USER_STATUS_FILTER_OPTIONS = [
  { value: "", label: "All" },
  { value: USER_STATUS.PENDING, label: "Pending" },
  { value: USER_STATUS.INVITED, label: "Invited" },
  { value: USER_STATUS.ACTIVE, label: "Active" },
  { value: USER_STATUS.DISABLED, label: "Disabled" },
] as const

export const CLIENT_PLATFORM_LABELS: Record<ClientPlatform, string> = {
  [CLIENT_PLATFORM.MACOS]: "macOS",
  [CLIENT_PLATFORM.LINUX]: "Linux",
  [CLIENT_PLATFORM.WINDOWS]: "Windows",
}

export const MEMBER_PRESENCE_ONLINE_WINDOW_MS = 150_000
export const MEMBER_LIST_PAGE_SIZE = 50

export const MEMBER_PRESENCE_STATUS = {
  ONLINE: "online",
  OFFLINE: "offline",
} as const

export const MEMBER_PRESENCE_LABELS = {
  [MEMBER_PRESENCE_STATUS.ONLINE]: "Online",
  [MEMBER_PRESENCE_STATUS.OFFLINE]: "Offline",
} as const

export const MEMBER_PRESENCE_TONES = {
  [MEMBER_PRESENCE_STATUS.ONLINE]: "success",
  [MEMBER_PRESENCE_STATUS.OFFLINE]: "neutral",
} as const

export const MEMBER_ANALYTICS_NAV_ITEMS = [
  { suffix: "", label: "Overview" },
  { suffix: "/activity", label: "Activity" },
  { suffix: "/tools", label: "Tools & Plugins" },
] as const

export const MEMBER_QUERY_KEYS = {
  detail: (userId: string) => ["member", userId] as const,
  list: ["members"] as const,
  installations: (userId: string) => ["member-installations", userId] as const,
  secrets: (userId: string) => ["member-secrets", userId] as const,
} as const

export const MEMBER_STATUS_TONES = {
  [USER_STATUS.ACTIVE]: "success",
  [USER_STATUS.PENDING]: "warning",
  [USER_STATUS.INVITED]: "accent",
  [USER_STATUS.DISABLED]: "danger",
} as const
