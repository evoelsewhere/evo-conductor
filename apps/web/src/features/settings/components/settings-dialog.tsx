import { SettingsForm } from "@/features/settings/components/settings-form"
import { Dialog } from "@/shared/ui/dialog"

export function SettingsDialog({
  open,
  onClose,
}: {
  open: boolean
  onClose: () => void
}) {
  return (
    <Dialog
      open={open}
      onClose={onClose}
      title="Project settings"
      description="Update project identity and SSO after initial setup."
      className="sm:max-w-lg"
    >
      {/* Remount when opened so form state resets from the latest API payload. */}
      {open && <SettingsForm />}
    </Dialog>
  )
}
