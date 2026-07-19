/**
 * Typed client for the osm-world Rust API server.
 *
 * @module api
 */

/** API base URL, read from `NEXT_PUBLIC_OSM_WORLD_API_URL` or defaulted to `http://127.0.0.1:3030`. */
export const API_URL = process.env.NEXT_PUBLIC_OSM_WORLD_API_URL ?? 'http://127.0.0.1:3030';

/** Feature type toggles controlling which OSM features are included in a prepare request. */
export interface FeatureFilter {
  roads: boolean;
  buildings: boolean;
  water: boolean;
  landuse: boolean;
  railways: boolean;
}

/** Default feature filter with all feature types enabled. */
export const defaultFilter: FeatureFilter = {
  roads: true,
  buildings: true,
  water: true,
  landuse: true,
  railways: true,
};

/** POI source selection mode controlling whether OSM, Overture, or both are used. */
export type PoiSourceMode = 'osm_only' | 'overture_only' | 'both' | 'overture_preferred';
/** Behavior when Overture data fetch fails. */
export type OvertureFailureMode = 'fallback_to_osm' | 'fail';

/** Overture and POI source configuration for prepare requests. */
export interface SourceControls {
  poi_source_mode: PoiSourceMode;
  overture_themes: string[];
  overture_failure_mode: OvertureFailureMode;
  overture_timeout: number;
}

/** Default source controls: OSM only, all Overture themes, fallback on failure. */
export const defaultSourceControls: SourceControls = {
  poi_source_mode: 'osm_only',
  overture_themes: ['address', 'base', 'building', 'place', 'transportation'],
  overture_failure_mode: 'fallback_to_osm',
  overture_timeout: 120,
};

/** Cached Overpass area entry from the shared par-osm-rust cache. */
export interface CacheEntry {
  key: string;
  bbox: [number, number, number, number];
  created_at: string;
  size_bytes: number;
}

/** Response from `POST /areas/prepare` with prepared file paths and launch command. */
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

/** Prepared area entry from `GET /areas/prepared`, including display metadata. */
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

/** Response from `DELETE /areas/prepared/{cacheKey}`. */
export interface DeletePreparedAreaResponse {
  status: string;
  cache_key: string;
}

/**
 * Response from `POST /renderer/launch`.
 *
 * The server only confirms that the renderer process was spawned; process
 * identifiers and resolved command details are not exposed over the API.
 * Matches `LaunchRendererResponse` in `src/server/types.rs`.
 */
export interface LaunchRendererResponse {
  status: string;
}

/**
 * Internal fetch wrapper that throws on non-OK responses with the server error message.
 * @typeParam T - Expected response type
 */
async function apiJson<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(`${API_URL}${path}`, init);

  if (!response.ok) {
    const body = (await response.json().catch(() => null)) as { error?: string } | null;
    throw new Error(body?.error ?? `HTTP ${response.status}`);
  }

  return response.json() as Promise<T>;
}

/**
 * Fetches the server health status.
 *
 * Returns only the API status string; cache directory paths are not exposed
 * over the API. Matches `HealthResponse` in `src/server/types.rs`.
 */
export function fetchHealth(): Promise<{ status: string }> {
  return apiJson('/health');
}

/** Lists cached Overpass areas from the shared cache. */
export function fetchCacheAreas(): Promise<CacheEntry[]> {
  return apiJson('/cache/areas');
}

/** Lists all prepared renderer input areas. */
export function fetchPreparedAreas(): Promise<PreparedAreaEntry[]> {
  return apiJson('/areas/prepared');
}

/** Updates the display name or favorite flag of a prepared area. */
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

/** Deletes a prepared area and its metadata. */
export function deletePreparedArea(cacheKey: string): Promise<DeletePreparedAreaResponse> {
  return apiJson(`/areas/prepared/${cacheKey}`, {
    method: 'DELETE',
  });
}

/** Prepares an area by fetching source data, optionally downloading SRTM, and building a renderer command. */
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

/** Launches the local renderer process for a prepared area. */
export function launchRenderer(body: {
  osm_path: string;
  srtm_dir?: string | null;
  spawn_lat?: number | null;
  spawn_lon?: number | null;
  extra_args?: string[];
}): Promise<LaunchRendererResponse> {
  return apiJson('/renderer/launch', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  });
}
