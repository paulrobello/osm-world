/**
 * User-facing error hint lookup for common prepare and launch failures.
 *
 * @module errorHints
 */

/**
 * Hint returned when the API rejects a request with HTTP 401. The server
 * returns 401 on every mutating endpoint whenever `OSM_WORLD_API_TOKEN` is set
 * and the request lacks a matching bearer token; the web client stores the
 * token via `setApiToken` in `./api`.
 */
const UNAUTHORIZED_HINT =
  'The server requires an API token. Open the settings dialog and paste your OSM_WORLD_API_TOKEN, then retry.';

/** Pattern-to-hint mapping for common API errors. */
const HINTS: Array<{ patterns: RegExp[]; hint: string }> = [
  {
    patterns: [/spawn.*bbox/i],
    hint: 'Move the spawn point inside the selected bbox, clear it, or click a new spawn while “Set on map” is enabled.',
  },
  {
    patterns: [/bbox.*(large|limit|span|area)/i, /south < north/i],
    hint: 'Try a smaller bbox. The API caps both bbox span and total area so the renderer does not start with an oversized city extract.',
  },
  {
    patterns: [/overpass/i, /failed to fetch map data/i, /bad gateway/i, /HTTP 502/i],
    hint: 'Overpass may be throttled or unavailable. Try Force refresh off, a smaller bbox, or retry after a short wait.',
  },
  {
    patterns: [/overture.*(cli|path|not found|missing)/i, /overturemaps/i],
    hint: 'Overture data needs the Overture CLI on PATH. Install it or switch POI source mode to OSM only.',
  },
  {
    patterns: [/srtm/i, /elevation/i, /failed to fetch elevation data/i],
    hint: 'Use elevation downloads SRTM tiles. Turn Use elevation off, check network access, or retry with a smaller bbox.',
  },
  {
    patterns: [/invalid request/i],
    hint: 'Check that bbox values are finite, ordered as south/west/north/east, and that spawn latitude/longitude are provided together.',
  },
  {
    patterns: [/^HTTP 401$/i, /\bunauthorized\b/i],
    hint: UNAUTHORIZED_HINT,
  },
];

/**
 * Returns a user-facing hint for a given error, or `null` if no hint matches.
 *
 * Pass the HTTP status code from an `ApiError` when available so auth failures
 * are recognized even when the server-supplied message is generic.
 *
 * @param message - The error message string from the API
 * @param status - Optional HTTP status code (e.g. from `ApiError.status`)
 */
export function errorHintForMessage(message: string, status?: number): string | null {
  if (status === 401) {
    return UNAUTHORIZED_HINT;
  }

  const normalized = message.trim();
  if (!normalized) {
    return null;
  }

  return HINTS.find((entry) => entry.patterns.some((pattern) => pattern.test(normalized)))?.hint ?? null;
}
