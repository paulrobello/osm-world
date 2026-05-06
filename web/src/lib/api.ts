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

export type PoiSourceMode = 'osm_only' | 'overture_only' | 'both' | 'overture_preferred';
export type OvertureFailureMode = 'fallback_to_osm' | 'fail';

export interface SourceControls {
  poi_source_mode: PoiSourceMode;
  overture_themes: string[];
  overture_failure_mode: OvertureFailureMode;
  overture_timeout: number;
}

export const defaultSourceControls: SourceControls = {
  poi_source_mode: 'osm_only',
  overture_themes: ['address', 'base', 'building', 'place', 'transportation'],
  overture_failure_mode: 'fallback_to_osm',
  overture_timeout: 120,
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
  source_status: string;
  warnings: string[];
  osm_path: string;
  srtm_dir: string | null;
  spawn_lat: number | null;
  spawn_lon: number | null;
  command: string;
  command_cwd: string;
  command_program: string;
  command_args: string[];
}

export interface PreparedAreaEntry extends Omit<PrepareAreaResponse, 'cache_status'> {
  display_name: string | null;
  favorite: boolean;
  filter: FeatureFilter;
  use_elevation: boolean;
  overture: boolean;
  overture_themes: string[];
  poi_source_mode: PoiSourceMode | null;
  overture_failure_mode: OvertureFailureMode | null;
  overture_timeout: number | null;
}

export interface DeletePreparedAreaResponse {
  status: string;
  cache_key: string;
}

export interface LaunchRendererResponse {
  status: string;
  pid: number;
  program: string;
  args: string[];
  command: string;
  command_cwd: string;
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

export function fetchPreparedAreas(): Promise<PreparedAreaEntry[]> {
  return apiJson('/areas/prepared');
}

export function updatePreparedArea(
  cacheKey: string,
  body: { display_name?: string; favorite?: boolean },
): Promise<PreparedAreaEntry> {
  return apiJson(`/areas/prepared/${cacheKey}`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  });
}

export function deletePreparedArea(cacheKey: string): Promise<DeletePreparedAreaResponse> {
  return apiJson(`/areas/prepared/${cacheKey}`, {
    method: 'DELETE',
  });
}

export function prepareArea(body: {
  bbox: [number, number, number, number];
  filter: FeatureFilter;
  use_elevation: boolean;
  force_refresh: boolean;
  spawn_lat?: number;
  spawn_lon?: number;
  overture: boolean;
  overture_themes: string[];
  poi_source_mode: PoiSourceMode;
  overture_failure_mode: OvertureFailureMode;
  overture_timeout: number;
}): Promise<PrepareAreaResponse> {
  return apiJson('/areas/prepare', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  });
}

export function launchRenderer(body: {
  osm_path: string;
  srtm_dir?: string | null;
  spawn_lat?: number | null;
  spawn_lon?: number | null;
}): Promise<LaunchRendererResponse> {
  return apiJson('/renderer/launch', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  });
}
