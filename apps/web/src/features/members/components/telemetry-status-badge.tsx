import {
  TELEMETRY_STATUS_TONES,
  type TelemetryEventStatus,
} from "@/shared/constants/telemetry"
import { Badge } from "@/shared/ui/badge"

export function TelemetryStatusBadge({ status }: { status: TelemetryEventStatus }) {
  return <Badge tone={TELEMETRY_STATUS_TONES[status]}>{status}</Badge>
}
