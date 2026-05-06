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
];

export function errorHintForMessage(message: string): string | null {
  const normalized = message.trim();
  if (!normalized) {
    return null;
  }

  return HINTS.find((entry) => entry.patterns.some((pattern) => pattern.test(normalized)))?.hint ?? null;
}
