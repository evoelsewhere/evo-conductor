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
      description="Manage project identity, network, and SSO."
      className="sm:max-w-3xl"
    >
      {/* Remount when opened so form state resets from the latest API payload. */}
      {open && <SettingsForm />}
    </Dialog>
  )
}
