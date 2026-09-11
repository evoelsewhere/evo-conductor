import { PRIMARY_ROLE, PRIMARY_ROLE_LABELS } from "@/shared/constants/member"

export const MONITORING_ALL_FILTER = "all"

export const MONITORING_ROLE_OPTIONS = [
  { value: MONITORING_ALL_FILTER, label: "All roles" },
  { value: PRIMARY_ROLE.ADMIN, label: PRIMARY_ROLE_LABELS[PRIMARY_ROLE.ADMIN] },
  { value: PRIMARY_ROLE.CONTRIBUTE, label: PRIMARY_ROLE_LABELS[PRIMARY_ROLE.CONTRIBUTE] },
  { value: PRIMARY_ROLE.USER, label: PRIMARY_ROLE_LABELS[PRIMARY_ROLE.USER] },
] as const

export const MONITORING_TAB = {
  MODEL: "model",
  MEMBER: "member",
} as const

export type MonitoringTab = (typeof MONITORING_TAB)[keyof typeof MONITORING_TAB]
