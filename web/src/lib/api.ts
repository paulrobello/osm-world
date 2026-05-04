export const API_URL = process.env.NEXT_PUBLIC_OSM_WORLD_API_URL ?? 'http://127.0.0.1:3030';

export interface FeatureFilter {
  roads: boolean;
  buildings: boolean;
  water: boolean;
  landuse: boolean;
  railways: boolean;
}

export const defaultFilter: FeatureFilter = {
  roads: true,
  buildings: true,
  water: true,
  landuse: true,
  railways: true,
};

export interface CacheEntry {
  key: string;
  bbox: [number, number, number, number];
  created_at: string;
  size_bytes: number;
}

export interface PrepareAreaResponse {
  bbox: [number, number, number, number];
  cache_key: string;
  cache_status: string;
  osm_path: string;
  srtm_dir: string | null;
  spawn_lat: number | null;
  spawn_lon: number | null;
  command: string;
  command_cwd: string;
  command_program: string;
  command_args: string[];
}

async function apiJson<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(`${API_URL}${path}`, init);

  if (!response.ok) {
    const body = (await response.json().catch(() => null)) as { error?: string } | null;
    throw new Error(body?.error ?? `HTTP ${response.status}`);
  }

  return response.json() as Promise<T>;
}

export function fetchHealth(): Promise<{ status: string; overpass_cache_dir: string; srtm_cache_dir: string }> {
  return apiJson('/health');
}

export function fetchCacheAreas(): Promise<CacheEntry[]> {
  return apiJson('/cache/areas');
}

export function prepareArea(body: {
  bbox: [number, number, number, number];
  filter: FeatureFilter;
  use_elevation: boolean;
  force_refresh: boolean;
  spawn_lat?: number;
  spawn_lon?: number;
}): Promise<PrepareAreaResponse> {
  return apiJson('/areas/prepare', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  });
}
