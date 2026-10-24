import { compressToEncodedURIComponent, decompressFromEncodedURIComponent } from 'lz-string'

const STORAGE_KEY = 'fratm:code'

/** Share URL with the code compressed in the hash (never sent to a server). */
export function buildShareUrl(code: string): string {
  const { origin, pathname } = window.location
  return `${origin}${pathname}#code=${compressToEncodedURIComponent(code)}`
}

/**
 * Embed mode (`?embed=1`): just the editor, the output and Run, to fit in an
 * iframe. The code comes from the link and is never autosaved.
 */
export function isEmbed(): boolean {
  return new URLSearchParams(window.location.search).has('embed')
}

/** `<iframe>` snippet that embeds the playground with this code. */
export function buildEmbedSnippet(code: string): string {
  const { origin, pathname } = window.location
  const src = `${origin}${pathname}?embed=1#code=${compressToEncodedURIComponent(code)}`
  return `<iframe src="${src}" title="FratmScript playground" width="100%" height="420" style="border: 1px solid #8884; border-radius: 8px" loading="lazy"></iframe>`
}

/**
 * Code from a share link, if any: `#code=` (lz-string) or the older
 * `?code=` (base64) links, which keep working.
 */
export function readSharedCode(): string | null {
  const hash = new URLSearchParams(window.location.hash.slice(1)).get('code')
  if (hash) {
    const code = decompressFromEncodedURIComponent(hash)
    if (code) return code
  }
  const legacy = new URLSearchParams(window.location.search).get('code')
  if (legacy) {
    try {
      return decodeURIComponent(atob(legacy))
    } catch (e) {
      console.error('Failed to load code from URL:', e)
    }
  }
  return null
}

export function loadSavedCode(): string | null {
  try {
    return localStorage.getItem(STORAGE_KEY)
  } catch {
    return null
  }
}

export function saveCode(code: string) {
  try {
    localStorage.setItem(STORAGE_KEY, code)
  } catch {
    // Storage full or unavailable: autosave is best effort
  }
}
