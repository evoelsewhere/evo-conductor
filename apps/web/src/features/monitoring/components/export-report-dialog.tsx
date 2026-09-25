import { useState } from "react"

import { useMutation } from "@tanstack/react-query"

import {
  api,
  ApiError,
  type ReportDestination,
  type ReportFormat,
  type ReportKind,
} from "@/shared/api/client"
import { Button } from "@/shared/ui/button"
import { Dialog } from "@/shared/ui/dialog"
import { Input } from "@/shared/ui/input"
import { Select, type SelectOption } from "@/shared/ui/select"

const FORMAT_OPTIONS: SelectOption<ReportFormat>[] = [
  { value: "xlsx", label: "Excel (.xlsx)" },
  { value: "pptx", label: "PowerPoint (.pptx)" },
]

const KIND_OPTIONS: { value: ReportKind; label: string }[] = [
  { value: "member", label: "Members" },
  { value: "model", label: "Models" },
  { value: "jira", label: "Jira tasks" },
]

export function ExportReportDialog({
  open,
  onClose,
  from,
  to,
}: {
  open: boolean
  onClose: () => void
  from?: string
  to?: string
}) {
  const [format, setFormat] = useState<ReportFormat>("xlsx")
  const [kinds, setKinds] = useState<ReportKind[]>(["member", "model", "jira"])
  const [destinationType, setDestinationType] = useState<"email" | "jira">("email")
  const [address, setAddress] = useState("")
  const [issueKey, setIssueKey] = useState("")
  const [downloadError, setDownloadError] = useState<string | null>(null)

  const toggleKind = (kind: ReportKind) => {
    setKinds((current) =>
      current.includes(kind) ? current.filter((value) => value !== kind) : [...current, kind],
    )
  }

  const download = useMutation({
    mutationFn: () => {
      setDownloadError(null)
      return api.downloadReportExport({ format, from, to, kinds })
    },
    onError: (error) =>
      setDownloadError(error instanceof ApiError ? error.message : "Download failed."),
  })

  const destination: ReportDestination =
    destinationType === "email" ? { type: "email", address } : { type: "jira", issue_key: issueKey }

  const deliver = useMutation({
    mutationFn: () => {
      if (!from || !to) throw new Error("Pick a date range first.")
      return api.deliverReport({ format, from, to, kinds, destination })
    },
  })

  const destinationReady =
    destinationType === "email" ? address.trim().length > 3 : issueKey.trim().length > 0

  return (
    <Dialog
      open={open}
      onClose={onClose}
      title="Export report"
      description="Excel or PowerPoint, covering the range and sections you pick below."
      footer={
        <div className="flex w-full items-center justify-between gap-2">
          <span className="text-xs text-(--color-text-subtle)">
            {kinds.length === 0 && "Pick at least one section."}
          </span>
          <div className="flex gap-2">
            <Button variant="ghost" onClick={onClose}>
              Close
            </Button>
            <Button
              disabled={kinds.length === 0 || download.isPending}
              onClick={() => download.mutate()}
            >
              {download.isPending ? "Preparing…" : "Download"}
            </Button>
          </div>
        </div>
      }
    >
      <div className="flex flex-col gap-4 px-5 py-4">
        <div className="grid gap-1.5">
          <label className="text-xs font-medium text-(--color-text-muted)">Format</label>
          <Select value={format} onValueChange={(next) => setFormat(next)} options={FORMAT_OPTIONS} />
        </div>

        <div className="grid gap-1.5">
          <label className="text-xs font-medium text-(--color-text-muted)">Sections</label>
          <div className="flex flex-wrap gap-3">
            {KIND_OPTIONS.map((option) => (
              <label key={option.value} className="flex items-center gap-1.5 text-sm">
                <input
                  type="checkbox"
                  checked={kinds.includes(option.value)}
                  onChange={() => toggleKind(option.value)}
                />
                {option.label}
              </label>
            ))}
          </div>
        </div>

        {downloadError && <p className="text-xs text-(--color-danger)">{downloadError}</p>}

        <div className="border-t border-(--border-soft) pt-4">
          <label className="text-xs font-medium text-(--color-text-muted)">Send to…</label>
          <div className="mt-1.5 flex gap-3 text-sm">
            <label className="flex items-center gap-1.5">
              <input
                type="radio"
                name="destination-type"
                checked={destinationType === "email"}
                onChange={() => setDestinationType("email")}
              />
              Email
            </label>
            <label className="flex items-center gap-1.5">
              <input
                type="radio"
                name="destination-type"
                checked={destinationType === "jira"}
                onChange={() => setDestinationType("jira")}
              />
              Jira issue
            </label>
          </div>

          {destinationType === "email" ? (
            <Input
              className="mt-2"
              placeholder="finance@example.com"
              value={address}
              onChange={(event) => setAddress(event.target.value)}
            />
          ) : (
            <Input
              className="mt-2"
              placeholder="SCRUM-30"
              value={issueKey}
              onChange={(event) => setIssueKey(event.target.value.toUpperCase())}
            />
          )}

          <div className="mt-2 flex items-center gap-2">
            <Button
              variant="outline"
              size="sm"
              disabled={!destinationReady || kinds.length === 0 || deliver.isPending}
              onClick={() => deliver.mutate()}
            >
              {deliver.isPending ? "Sending…" : "Send"}
            </Button>
            {deliver.isSuccess && (
              <span className="text-xs text-(--color-success)">Sent.</span>
            )}
            {deliver.isError && (
              <span className="text-xs text-(--color-danger)">
                {deliver.error instanceof ApiError ? deliver.error.message : "Delivery failed."}
              </span>
            )}
          </div>
        </div>
      </div>
    </Dialog>
  )
}
