/**
 * Reads a `redirect` query param and validates it's a same-origin relative
 * path, never an absolute URL — guards the auth pages' post-login bounce
 * against being turned into an open redirect.
 */
export function safeRedirectTarget(
  search: string = window.location.search,
): string | null {
  const value = new URLSearchParams(search).get("redirect")
  if (!value || !value.startsWith("/") || value.startsWith("//")) return null
  return value
}
