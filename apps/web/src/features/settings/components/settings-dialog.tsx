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
      description="Identity, connectivity, data policy, object storage, and authentication."
      className="sm:h-[min(78dvh,44rem)] sm:w-[min(88vw,64rem)] sm:max-w-5xl"
      contentClassName="min-h-0 flex-1 overflow-hidden p-0"
    >
      {/* Remount when opened so form state resets from the latest API payload. */}
      {open && <SettingsForm />}
    </Dialog>
  )
}
