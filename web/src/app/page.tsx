'use client';

import dynamic from 'next/dynamic';
import { useCallback, useEffect, useMemo, useState } from 'react';
import {
  API_URL,
  defaultFilter,
  defaultSourceControls,
  deletePreparedArea,
  fetchCacheAreas,
  fetchHealth,
  fetchPreparedAreas,
  launchRenderer,
  prepareArea,
  updatePreparedArea,
  type CacheEntry,
  type FeatureFilter,
  type PoiSourceMode,
  type PrepareAreaResponse,
  type PreparedAreaEntry,
  type SourceControls,
} from '@/lib/api';
import { BBOX_PRESETS, type BboxPreset } from '@/lib/bboxPresets';
import { buildCommandVariants, rendererOptionArgs } from '@/lib/commandVariants';
import { errorHintForMessage } from '@/lib/errorHints';
import {
  DEFAULT_RENDERER_OPTIONS,
  createSettingsProfile,
  exportSettingsProfile,
  importSettingsProfile,
  type RendererOptions,
  type SettingsProfile,
} from '@/lib/settingsProfiles';
import type { BBox, SpawnPoint } from '@/components/MapPicker';
import { ConfirmDialog, PromptDialog } from '@/components/Dialog';
import { HelpOverlay } from '@/components/HelpOverlay';
import { PreparedHistorySection } from '@/components/PreparedHistorySection';
import { PreparedOutputSection } from '@/components/PreparedOutputSection';

const MapPicker = dynamic(() => import('@/components/MapPicker'), {
  ssr: false,
  loading: () => <section className="map-canvas map-loading" aria-label="Loading map">Calibrating map grid…</section>,
});

const FEATURE_LABELS: Array<[keyof FeatureFilter, string]> = [
  ['roads', 'Roads'],
  ['buildings', 'Buildings'],
  ['water', 'Water'],
  ['landuse', 'Land use'],
  ['railways', 'Railways'],
];

const SOURCE_MODE_LABELS: Array<[PoiSourceMode, string]> = [
  ['osm_only', 'OSM only'],
  ['overture_only', 'Overture only'],
  ['both', 'OSM + Overture'],
  ['overture_preferred', 'Overture preferred'],
];

const OVERTURE_THEME_LABELS: Array<[string, string]> = [
  ['address', 'Addresses'],
  ['base', 'Base / land + water'],
  ['building', 'Buildings'],
  ['place', 'Places / POIs'],
  ['transportation', 'Transportation'],
];

const SETTINGS_PROFILE_STORAGE_KEY = 'osm-world-web-settings-profile';
const VISUAL_PRESET_LABELS: Array<[RendererOptions['visualPreset'], string]> = [
  ['performance', 'Performance'],
  ['balanced', 'Balanced'],
  ['showcase', 'Showcase'],
];

type HealthState = Awaited<ReturnType<typeof fetchHealth>>;

function formatBbox(bbox: BBox | null): string {
  if (!bbox) {
    return 'No bbox selected';
  }

  return bbox.map((value) => value.toFixed(5)).join(', ');
}

function formatSpawnPoint(spawnPoint: SpawnPoint | null): string {
  if (!spawnPoint) {
    return 'No spawn point selected';
  }

  return `${spawnPoint.lat.toFixed(5)}, ${spawnPoint.lon.toFixed(5)}`;
}

function spawnInsideBbox(spawnPoint: SpawnPoint, bbox: BBox): boolean {
  const [south, west, north, east] = bbox;
  return spawnPoint.lat >= south && spawnPoint.lat <= north && spawnPoint.lon >= west && spawnPoint.lon <= east;
}

function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) {
    return '0 B';
  }

  const units = ['B', 'KB', 'MB', 'GB'];
  const power = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
  return `${(bytes / 1024 ** power).toFixed(power === 0 ? 0 : 1)} ${units[power]}`;
}

function preparedEntryToResult(entry: PreparedAreaEntry): PrepareAreaResponse {
  return {
    bbox: entry.bbox,
    cache_key: entry.cache_key,
    cache_status: 'prepared_history',
    source_status: entry.source_status,
    warnings: entry.warnings,
    osm_path: entry.osm_path,
    srtm_dir: entry.srtm_dir,
    spawn_lat: entry.spawn_lat,
    spawn_lon: entry.spawn_lon,
    command: entry.command,
    command_cwd: entry.command_cwd,
    command_program: entry.command_program,
    command_args: entry.command_args,
  };
}

export default function Home() {
  const [health, setHealth] = useState<HealthState | null>(null);
  const [cacheAreas, setCacheAreas] = useState<CacheEntry[]>([]);
  const [preparedAreas, setPreparedAreas] = useState<PreparedAreaEntry[]>([]);
  const [selectedBbox, setSelectedBbox] = useState<BBox | null>(null);
  const [manualBbox, setManualBbox] = useState({
    south: '',
    west: '',
    north: '',
    east: '',
  });
  const [spawnPoint, setSpawnPoint] = useState<SpawnPoint | null>(null);
  const [manualSpawn, setManualSpawn] = useState({
    lat: '',
    lon: '',
  });
  const [spawnMode, setSpawnMode] = useState(false);
  const [filter, setFilter] = useState<FeatureFilter>(defaultFilter);
  const [sourceControls, setSourceControls] = useState<SourceControls>(defaultSourceControls);
  const [useElevation, setUseElevation] = useState(false);
  const [forceRefresh, setForceRefresh] = useState(false);
  const [rendererOptions, setRendererOptions] = useState<RendererOptions>(DEFAULT_RENDERER_OPTIONS);
  const [profileName, setProfileName] = useState('Default renderer profile');
  const [profileJson, setProfileJson] = useState('');
  const [profileStatus, setProfileStatus] = useState<string | null>(null);
  const [loadingMeta, setLoadingMeta] = useState(true);
  const [metaError, setMetaError] = useState<string | null>(null);
  const [prepareError, setPrepareError] = useState<string | null>(null);
  const [isPreparing, setIsPreparing] = useState(false);
  const [preparedArea, setPreparedArea] = useState<PrepareAreaResponse | null>(null);
  const [copiedCommandVariant, setCopiedCommandVariant] = useState<string | null>(null);
  const [copyStatus, setCopyStatus] = useState<'idle' | 'copied' | 'failed'>('idle');
  const [launchStatus, setLaunchStatus] = useState<'idle' | 'launching' | 'launched' | 'failed'>('idle');
  const [launchMessage, setLaunchMessage] = useState<string | null>(null);
  const [renameTarget, setRenameTarget] = useState<PreparedAreaEntry | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<PreparedAreaEntry | null>(null);

  const selectedFeatureCount = useMemo(
    () => Object.values(filter).filter(Boolean).length,
    [filter],
  );

  const cacheTotalSize = useMemo(
    () => cacheAreas.reduce((sum, area) => sum + area.size_bytes, 0),
    [cacheAreas],
  );

  const favoritePreparedCount = useMemo(
    () => preparedAreas.filter((area) => area.favorite).length,
    [preparedAreas],
  );

  const spawnBboxError = useMemo(() => {
    if (!selectedBbox || !spawnPoint || spawnInsideBbox(spawnPoint, selectedBbox)) {
      return null;
    }

    return 'Spawn point must be inside the selected bbox.';
  }, [selectedBbox, spawnPoint]);

  const currentSettingsProfile = useMemo(
    () => createSettingsProfile({
      name: profileName.trim() || 'Unnamed renderer profile',
      filter,
      sourceControls,
      useElevation,
      forceRefresh,
      renderer: rendererOptions,
    }),
    [filter, forceRefresh, profileName, rendererOptions, sourceControls, useElevation],
  );

  const commandVariants = useMemo(
    () => (preparedArea ? buildCommandVariants(preparedArea, rendererOptions) : []),
    [preparedArea, rendererOptions],
  );

  const clearPreparedOutput = useCallback(() => {
    setPreparedArea(null);
    setCopiedCommandVariant(null);
    setCopyStatus('idle');
    setLaunchStatus('idle');
    setLaunchMessage(null);
  }, []);

  const loadMeta = useCallback(async () => {
    setLoadingMeta(true);
    setMetaError(null);

    const [healthResult, cacheResult, preparedResult] = await Promise.allSettled([
      fetchHealth(),
      fetchCacheAreas(),
      fetchPreparedAreas(),
    ]);

    if (healthResult.status === 'fulfilled') {
      setHealth(healthResult.value);
    } else {
      setHealth(null);
      setMetaError(healthResult.reason instanceof Error ? healthResult.reason.message : 'Unable to read API health');
    }

    if (cacheResult.status === 'fulfilled') {
      setCacheAreas(cacheResult.value);
    } else {
      setCacheAreas([]);
      setMetaError((previous) => {
        const cacheMessage = cacheResult.reason instanceof Error ? cacheResult.reason.message : 'Unable to read cache areas';
        return previous ? `${previous}; ${cacheMessage}` : cacheMessage;
      });
    }

    if (preparedResult.status === 'fulfilled') {
      setPreparedAreas(preparedResult.value);
    } else {
      setPreparedAreas([]);
      setMetaError((previous) => {
        const preparedMessage = preparedResult.reason instanceof Error ? preparedResult.reason.message : 'Unable to read prepared history';
        return previous ? `${previous}; ${preparedMessage}` : preparedMessage;
      });
    }

    setLoadingMeta(false);
  }, []);

  useEffect(() => {
    void loadMeta();
  }, [loadMeta]);

  useEffect(() => {
    if (!selectedBbox) {
      return;
    }
    const [south, west, north, east] = selectedBbox;
    setManualBbox({
      south: south.toFixed(6),
      west: west.toFixed(6),
      north: north.toFixed(6),
      east: east.toFixed(6),
    });
  }, [selectedBbox]);

  useEffect(() => {
    if (!spawnPoint) {
      return;
    }
    setManualSpawn({
      lat: spawnPoint.lat.toFixed(6),
      lon: spawnPoint.lon.toFixed(6),
    });
  }, [spawnPoint]);

  const toggleFeature = (name: keyof FeatureFilter) => {
    if (isPreparing) {
      return;
    }
    clearPreparedOutput();
    setFilter((current) => ({ ...current, [name]: !current[name] }));
  };

  const setSourceControl = <K extends keyof SourceControls>(name: K, value: SourceControls[K]) => {
    if (isPreparing) {
      return;
    }
    clearPreparedOutput();
    setSourceControls((current) => ({ ...current, [name]: value }));
  };

  const toggleOvertureTheme = (theme: string) => {
    if (isPreparing) {
      return;
    }
    clearPreparedOutput();
    setSourceControls((current) => {
      const themes = current.overture_themes.includes(theme)
        ? current.overture_themes.filter((value) => value !== theme)
        : [...current.overture_themes, theme];
      return { ...current, overture_themes: themes };
    });
  };

  const loadPreparedEntry = (entry: PreparedAreaEntry) => {
    if (isPreparing) {
      return;
    }
    setSelectedBbox(entry.bbox);
    setFilter(entry.filter);
    setUseElevation(entry.use_elevation);
    setSpawnPoint(
      entry.spawn_lat !== null && entry.spawn_lon !== null
        ? { lat: entry.spawn_lat, lon: entry.spawn_lon }
        : null,
    );
    setSourceControls({
      poi_source_mode: entry.overture ? (entry.poi_source_mode ?? 'overture_preferred') : 'osm_only',
      overture_themes: entry.overture_themes.length > 0 ? entry.overture_themes : defaultSourceControls.overture_themes,
      overture_failure_mode: entry.overture_failure_mode ?? defaultSourceControls.overture_failure_mode,
      overture_timeout: entry.overture_timeout ?? defaultSourceControls.overture_timeout,
    });
    setPreparedArea(preparedEntryToResult(entry));
    setPrepareError(null);
    setCopiedCommandVariant(null);
    setCopyStatus('idle');
    setLaunchStatus('idle');
    setLaunchMessage(null);
  };

  const renamePreparedEntry = async (entry: PreparedAreaEntry, displayName: string) => {
    try {
      const updated = await updatePreparedArea(entry.cache_key, { display_name: displayName });
      setPreparedAreas((current) => current.map((area) => (area.cache_key === updated.cache_key ? updated : area)));
      if (preparedArea?.cache_key === updated.cache_key) {
        setPreparedArea(preparedEntryToResult(updated));
      }
    } catch (error) {
      setMetaError(error instanceof Error ? error.message : 'Unable to rename prepared area');
    }
  };

  const togglePreparedFavorite = async (entry: PreparedAreaEntry) => {
    try {
      const updated = await updatePreparedArea(entry.cache_key, { favorite: !entry.favorite });
      setPreparedAreas((current) => current.map((area) => (area.cache_key === updated.cache_key ? updated : area)));
    } catch (error) {
      setMetaError(error instanceof Error ? error.message : 'Unable to update favorite');
    }
  };

  const deletePreparedEntry = async (entry: PreparedAreaEntry) => {
    try {
      await deletePreparedArea(entry.cache_key);
      setPreparedAreas((current) => current.filter((area) => area.cache_key !== entry.cache_key));
      if (preparedArea?.cache_key === entry.cache_key) {
        clearPreparedOutput();
      }
    } catch (error) {
      setMetaError(error instanceof Error ? error.message : 'Unable to delete prepared area');
    }
  };

  const setRendererOption = <K extends keyof RendererOptions>(name: K, value: RendererOptions[K]) => {
    clearPreparedOutput();
    setRendererOptions((current) => ({ ...current, [name]: value }));
  };

  const applySettingsProfile = (profile: SettingsProfile) => {
    clearPreparedOutput();
    setProfileName(profile.name);
    setFilter(profile.filter);
    setSourceControls(profile.sourceControls);
    setUseElevation(profile.useElevation);
    setForceRefresh(profile.forceRefresh);
    setRendererOptions(profile.renderer);
  };

  const exportProfile = async () => {
    const json = exportSettingsProfile(currentSettingsProfile);
    setProfileJson(json);
    setProfileStatus('Profile exported below. Copy or save the JSON to reuse it later.');
    try {
      await navigator.clipboard.writeText(json);
      setProfileStatus('Profile exported and copied to clipboard.');
    } catch {
      // Clipboard access is optional; the textarea still contains the export.
    }
  };

  const importProfile = () => {
    try {
      const profile = importSettingsProfile(profileJson);
      applySettingsProfile(profile);
      setProfileStatus(`Imported “${profile.name}”.`);
    } catch (error) {
      setProfileStatus(error instanceof Error ? error.message : 'Unable to import settings profile');
    }
  };

  const saveProfileToBrowser = () => {
    const json = exportSettingsProfile(currentSettingsProfile);
    window.localStorage.setItem(SETTINGS_PROFILE_STORAGE_KEY, json);
    setProfileJson(json);
    setProfileStatus(`Saved “${currentSettingsProfile.name}” in this browser.`);
  };

  const loadProfileFromBrowser = () => {
    const json = window.localStorage.getItem(SETTINGS_PROFILE_STORAGE_KEY);
    if (!json) {
      setProfileStatus('No saved profile found in this browser.');
      return;
    }
    try {
      const profile = importSettingsProfile(json);
      applySettingsProfile(profile);
      setProfileJson(json);
      setProfileStatus(`Loaded “${profile.name}”.`);
    } catch (error) {
      setProfileStatus(error instanceof Error ? error.message : 'Unable to load saved profile');
    }
  };

  const handleBboxChange = (bbox: BBox) => {
    if (isPreparing) {
      return;
    }
    clearPreparedOutput();
    setPrepareError(null);
    setSelectedBbox(bbox);
  };

  const applyBboxPreset = (preset: BboxPreset) => {
    if (isPreparing) {
      return;
    }
    clearPreparedOutput();
    setPrepareError(null);
    setSelectedBbox(preset.bbox);
    setSpawnPoint(preset.spawnPoint ?? null);
  };

  const handleSpawnChange = (nextSpawnPoint: SpawnPoint) => {
    if (isPreparing) {
      return;
    }
    clearPreparedOutput();
    setPrepareError(null);
    setSpawnPoint(nextSpawnPoint);
  };

  const applyManualBbox = () => {
    const south = Number(manualBbox.south);
    const west = Number(manualBbox.west);
    const north = Number(manualBbox.north);
    const east = Number(manualBbox.east);

    if (![south, west, north, east].every(Number.isFinite)) {
      setPrepareError('Manual bbox values must be finite numbers.');
      return;
    }
    if (south >= north || west >= east) {
      setPrepareError('Manual bbox must satisfy south < north and west < east.');
      return;
    }

    handleBboxChange([south, west, north, east]);
  };

  const applyManualSpawn = () => {
    const lat = Number(manualSpawn.lat);
    const lon = Number(manualSpawn.lon);

    if (![lat, lon].every(Number.isFinite)) {
      setPrepareError('Manual spawn values must be finite numbers.');
      return;
    }
    if (lat < -90 || lat > 90) {
      setPrepareError('Manual spawn latitude must be in -90..=90.');
      return;
    }
    if (lon < -180 || lon > 180) {
      setPrepareError('Manual spawn longitude must be in -180..=180.');
      return;
    }

    handleSpawnChange({ lat, lon });
  };

  const clearSpawn = () => {
    clearPreparedOutput();
    setSpawnPoint(null);
    setManualSpawn({ lat: '', lon: '' });
    setPrepareError(null);
  };

  const handlePrepare = async () => {
    if (!selectedBbox) {
      setPrepareError('Draw a bounding box before preparing area data.');
      return;
    }
    if (spawnBboxError) {
      setPrepareError(spawnBboxError);
      return;
    }

    setIsPreparing(true);
    setPrepareError(null);
    setPreparedArea(null);
    setCopiedCommandVariant(null);
    setCopyStatus('idle');

    try {
      const prepared = await prepareArea({
        bbox: selectedBbox,
        filter,
        use_elevation: useElevation,
        force_refresh: forceRefresh,
        overture: sourceControls.poi_source_mode !== 'osm_only',
        overture_themes: sourceControls.overture_themes,
        poi_source_mode: sourceControls.poi_source_mode,
        overture_failure_mode: sourceControls.overture_failure_mode,
        overture_timeout: sourceControls.overture_timeout,
        ...(spawnPoint ? { spawn_lat: spawnPoint.lat, spawn_lon: spawnPoint.lon } : {}),
      });
      setPreparedArea(prepared);
      const [refreshedAreas, refreshedPreparedAreas] = await Promise.all([
        fetchCacheAreas().catch(() => null),
        fetchPreparedAreas().catch(() => null),
      ]);
      if (refreshedAreas) {
        setCacheAreas(refreshedAreas);
      }
      if (refreshedPreparedAreas) {
        setPreparedAreas(refreshedPreparedAreas);
      }
    } catch (error) {
      setPrepareError(error instanceof Error ? error.message : 'Prepare request failed');
    } finally {
      setIsPreparing(false);
    }
  };

  const copyCommand = async (variantId: string, command: string) => {
    try {
      await navigator.clipboard.writeText(command);
      setCopiedCommandVariant(variantId);
      setCopyStatus('copied');
    } catch {
      setCopiedCommandVariant(null);
      setCopyStatus('failed');
    }
  };

  const handleLaunchRenderer = async () => {
    if (!preparedArea) {
      return;
    }
    setLaunchStatus('launching');
    setLaunchMessage(null);
    try {
      const result = await launchRenderer({
        osm_path: preparedArea.osm_path,
        srtm_dir: preparedArea.srtm_dir,
        spawn_lat: preparedArea.spawn_lat,
        spawn_lon: preparedArea.spawn_lon,
        extra_args: rendererOptionArgs(rendererOptions),
      });
      setLaunchStatus('launched');
      setLaunchMessage(`Renderer launched (status: ${result.status}).`);
    } catch (error) {
      setLaunchStatus('failed');
      setLaunchMessage(error instanceof Error ? error.message : 'Renderer launch failed');
    }
  };

  return (
    <main className="app-shell">
      <section className="panel control-panel" aria-labelledby="page-title">
        <div className="panel-scroll">
          <header className="hero-block">
            <p className="eyebrow">osm-world web picker</p>
            <h1 id="page-title">
              Area <span className="accent">Picker</span>
            </h1>
            <p className="lede">
              Draw a geographic bounding box, inspect shared cache coverage, and prepare local OSM inputs for the renderer.
            </p>
          </header>

          <section className="console-card api-card" aria-labelledby="api-title">
            <h2 id="api-title">API telemetry</h2>
            <div className="console-line">
              <strong>base</strong>
              <span>{API_URL}</span>
            </div>
            <div className="console-line">
              <strong>health</strong>
              <span>{loadingMeta ? 'checking…' : health?.status ?? 'offline'}</span>
            </div>
            <div className="console-line">
              <strong>areas</strong>
              <span>{cacheAreas.length} cached · {formatBytes(cacheTotalSize)}</span>
            </div>
            <div className="console-line">
              <strong>prepared</strong>
              <span>{preparedAreas.length} saved · {favoritePreparedCount} pinned</span>
            </div>
            {metaError ? (
              <div className="status-stack">
                <p className="status-line error">{metaError}</p>
                {errorHintForMessage(metaError) ? <p className="status-line hint">{errorHintForMessage(metaError)}</p> : null}
              </div>
            ) : null}
            <button className="ghost-button" type="button" onClick={() => void loadMeta()} disabled={loadingMeta}>
              Refresh telemetry
            </button>
          </section>

          <section className="bbox-readout" aria-labelledby="selection-title">
            <h2 id="selection-title">Selected area</h2>
            <code>[south, west, north, east]</code>
            <output>{formatBbox(selectedBbox)}</output>
            <code>[spawn lat, spawn lon]</code>
            <output>{formatSpawnPoint(spawnPoint)}</output>
          </section>

          <PreparedHistorySection
            entries={preparedAreas}
            disabled={isPreparing}
            onLoadEntry={loadPreparedEntry}
            onToggleFavorite={(entry) => void togglePreparedFavorite(entry)}
            onRename={setRenameTarget}
            onDelete={setDeleteTarget}
          />

          <section className="control-group" aria-labelledby="features-title">
            <div className="section-heading">
              <h2 id="features-title">Feature filters</h2>
              <span>{selectedFeatureCount}/5 enabled</span>
            </div>
            <div className="form-grid">
              {FEATURE_LABELS.map(([name, label]) => (
                <label className="field" key={name}>
                  <span>{label}</span>
                  <input
                    type="checkbox"
                    checked={filter[name]}
                    disabled={isPreparing}
                    onChange={() => toggleFeature(name)}
                  />
                </label>
              ))}
            </div>
          </section>

          <section className="control-group" aria-labelledby="sources-title">
            <div className="section-heading">
              <h2 id="sources-title">Source controls</h2>
              <span>{SOURCE_MODE_LABELS.find(([mode]) => mode === sourceControls.poi_source_mode)?.[1]}</span>
            </div>
            <label className="coordinate-field full-width-field">
              <span>POI source mode</span>
              <select
                value={sourceControls.poi_source_mode}
                disabled={isPreparing}
                onChange={(event) => setSourceControl('poi_source_mode', event.target.value as PoiSourceMode)}
              >
                {SOURCE_MODE_LABELS.map(([mode, label]) => (
                  <option key={mode} value={mode}>{label}</option>
                ))}
              </select>
            </label>
            <div className="form-grid source-theme-grid">
              {OVERTURE_THEME_LABELS.map(([theme, label]) => (
                <label className="field" key={theme}>
                  <span>{label}</span>
                  <input
                    type="checkbox"
                    checked={sourceControls.overture_themes.includes(theme)}
                    disabled={isPreparing || sourceControls.poi_source_mode === 'osm_only'}
                    onChange={() => toggleOvertureTheme(theme)}
                  />
                </label>
              ))}
            </div>
            <div className="coordinate-grid source-options-grid">
              <label className="coordinate-field">
                <span>Fallback</span>
                <select
                  value={sourceControls.overture_failure_mode}
                  disabled={isPreparing || sourceControls.poi_source_mode === 'osm_only'}
                  onChange={(event) => setSourceControl('overture_failure_mode', event.target.value as SourceControls['overture_failure_mode'])}
                >
                  <option value="fallback_to_osm">Fallback to OSM</option>
                  <option value="fail">Fail request</option>
                </select>
              </label>
              <label className="coordinate-field">
                <span>Timeout sec</span>
                <input
                  type="number"
                  min="1"
                  step="1"
                  value={sourceControls.overture_timeout}
                  disabled={isPreparing || sourceControls.poi_source_mode === 'osm_only'}
                  onChange={(event) => setSourceControl('overture_timeout', Math.max(1, Number(event.target.value) || 1))}
                />
              </label>
            </div>
            <p className="microcopy">Overture settings are sent only when the mode is not OSM-only.</p>
          </section>

          <section className="control-group" aria-labelledby="bbox-presets-title">
            <div className="section-heading">
              <h2 id="bbox-presets-title">Area presets</h2>
              <span>quick select</span>
            </div>
            <p className="microcopy">
              Jump to common renderer test boxes without retyping coordinates.
            </p>
            <div className="button-row preset-buttons">
              {BBOX_PRESETS.map((preset) => (
                <button
                  className="ghost-button"
                  type="button"
                  key={preset.id}
                  onClick={() => applyBboxPreset(preset)}
                  disabled={isPreparing}
                  title={preset.description}
                >
                  {preset.label}
                </button>
              ))}
            </div>
          </section>

          <section className="control-group" aria-labelledby="manual-bbox-title">
            <div className="section-heading">
              <h2 id="manual-bbox-title">Manual bbox</h2>
              <span>keyboard accessible</span>
            </div>
            <p className="microcopy">
              Enter coordinates directly when drawing on the map is not practical.
            </p>
            <div className="coordinate-grid">
              {(['south', 'west', 'north', 'east'] as const).map((name) => (
                <label className="coordinate-field" key={name}>
                  <span>{name}</span>
                  <input
                    type="number"
                    step="0.000001"
                    value={manualBbox[name]}
                    disabled={isPreparing}
                    onChange={(event) => setManualBbox((current) => ({ ...current, [name]: event.target.value }))}
                    placeholder={name === 'south' || name === 'north' ? '38.58' : '-121.49'}
                  />
                </label>
              ))}
            </div>
            <button className="ghost-button" type="button" onClick={applyManualBbox} disabled={isPreparing}>
              Apply manual bbox
            </button>
          </section>
          <section className="control-group spawn-controls" aria-labelledby="spawn-title">
            <div className="section-heading">
              <h2 id="spawn-title">Spawn point</h2>
              <span>{spawnPoint ? 'set' : 'optional'}</span>
            </div>
            <p className="microcopy">
              Choose where the prepared scene should start, either by clicking the map or by entering coordinates.
            </p>
            <label className="toggle-row">
              <span>Set on map</span>
              <input
                type="checkbox"
                checked={spawnMode}
                disabled={isPreparing}
                onChange={() => setSpawnMode((value) => !value)}
              />
            </label>
            <div className="coordinate-grid">
              {(['lat', 'lon'] as const).map((name) => (
                <label className="coordinate-field" key={name}>
                  <span>{name}</span>
                  <input
                    type="number"
                    step="0.000001"
                    value={manualSpawn[name]}
                    disabled={isPreparing}
                    onChange={(event) => setManualSpawn((current) => ({ ...current, [name]: event.target.value }))}
                    placeholder={name === 'lat' ? '38.581600' : '-121.494400'}
                  />
                </label>
              ))}
            </div>
            <div className="button-row">
              <button className="ghost-button" type="button" onClick={applyManualSpawn} disabled={isPreparing}>
                Apply spawn
              </button>
              <button className="ghost-button danger-button" type="button" onClick={clearSpawn} disabled={isPreparing || !spawnPoint}>
                Clear spawn
              </button>
            </div>
            {spawnBboxError ? <p className="status-line error">{spawnBboxError}</p> : null}
          </section>


          <section className="control-group" aria-labelledby="renderer-profile-title">
            <div className="section-heading">
              <h2 id="renderer-profile-title">Renderer profile</h2>
              <span>{rendererOptions.visualPreset}</span>
            </div>
            <p className="microcopy">
              Save/load prep settings plus launch-time lighting and performance flags used by the command variants.
            </p>
            <label className="coordinate-field full-width-field">
              <span>Profile name</span>
              <input
                type="text"
                value={profileName}
                disabled={isPreparing}
                onChange={(event) => setProfileName(event.target.value)}
              />
            </label>
            <div className="coordinate-grid source-options-grid">
              <label className="coordinate-field">
                <span>Time of day</span>
                <input
                  type="number"
                  min="0"
                  max="24"
                  step="0.25"
                  value={rendererOptions.timeOfDay}
                  disabled={isPreparing}
                  onChange={(event) => setRendererOption('timeOfDay', Math.min(24, Math.max(0, Number(event.target.value) || 0)))}
                />
              </label>
              <label className="coordinate-field">
                <span>Visual preset</span>
                <select
                  value={rendererOptions.visualPreset}
                  disabled={isPreparing}
                  onChange={(event) => setRendererOption('visualPreset', event.target.value as RendererOptions['visualPreset'])}
                >
                  {VISUAL_PRESET_LABELS.map(([preset, label]) => (
                    <option key={preset} value={preset}>{label}</option>
                  ))}
                </select>
              </label>
              <label className="coordinate-field">
                <span>Stream radius</span>
                <input
                  type="number"
                  min="1"
                  step="500"
                  value={rendererOptions.streamRadius}
                  disabled={isPreparing}
                  onChange={(event) => setRendererOption('streamRadius', Math.max(1, Number(event.target.value) || 1))}
                />
              </label>
              <label className="coordinate-field">
                <span>Upload MiB</span>
                <input
                  type="number"
                  min="0.1"
                  step="0.5"
                  value={rendererOptions.uploadBudgetMb}
                  disabled={isPreparing}
                  onChange={(event) => setRendererOption('uploadBudgetMb', Math.max(0.1, Number(event.target.value) || 0.1))}
                />
              </label>
              <label className="coordinate-field">
                <span>Max tiles</span>
                <input
                  type="number"
                  min="1"
                  step="1"
                  value={rendererOptions.maxUploadedTiles}
                  disabled={isPreparing}
                  onChange={(event) => setRendererOption('maxUploadedTiles', Math.max(1, Math.floor(Number(event.target.value) || 1)))}
                />
              </label>
              <label className="toggle-row compact-toggle">
                <span>Open settings</span>
                <input
                  type="checkbox"
                  checked={rendererOptions.showSettings}
                  disabled={isPreparing}
                  onChange={() => setRendererOption('showSettings', !rendererOptions.showSettings)}
                />
              </label>
            </div>
            <div className="form-grid source-theme-grid">
              <label className="field">
                <span>POI labels profile</span>
                <input
                  type="checkbox"
                  checked={rendererOptions.labels.poi}
                  disabled={isPreparing}
                  onChange={() => {
                    clearPreparedOutput();
                    setRendererOptions((current) => ({ ...current, labels: { ...current.labels, poi: !current.labels.poi } }));
                  }}
                />
              </label>
              <label className="field">
                <span>Address labels profile</span>
                <input
                  type="checkbox"
                  checked={rendererOptions.labels.addresses}
                  disabled={isPreparing}
                  onChange={() => {
                    clearPreparedOutput();
                    setRendererOptions((current) => ({ ...current, labels: { ...current.labels, addresses: !current.labels.addresses } }));
                  }}
                />
              </label>
              <label className="field">
                <span>Street signs profile</span>
                <input
                  type="checkbox"
                  checked={rendererOptions.labels.streetSigns}
                  disabled={isPreparing}
                  onChange={() => {
                    clearPreparedOutput();
                    setRendererOptions((current) => ({ ...current, labels: { ...current.labels, streetSigns: !current.labels.streetSigns } }));
                  }}
                />
              </label>
              <label className="field">
                <span>Minimap visible profile</span>
                <input
                  type="checkbox"
                  checked={rendererOptions.minimap.visible}
                  disabled={isPreparing}
                  onChange={() => {
                    clearPreparedOutput();
                    setRendererOptions((current) => ({ ...current, minimap: { ...current.minimap, visible: !current.minimap.visible } }));
                  }}
                />
              </label>
              <label className="field">
                <span>Minimap rotation profile</span>
                <input
                  type="checkbox"
                  checked={rendererOptions.minimap.rotateWithCamera}
                  disabled={isPreparing}
                  onChange={() => {
                    clearPreparedOutput();
                    setRendererOptions((current) => ({ ...current, minimap: { ...current.minimap, rotateWithCamera: !current.minimap.rotateWithCamera } }));
                  }}
                />
              </label>
            </div>
            <div className="button-row profile-actions">
              <button className="ghost-button" type="button" onClick={() => void exportProfile()}>
                Export JSON
              </button>
              <button className="ghost-button" type="button" onClick={importProfile} disabled={!profileJson.trim()}>
                Import JSON
              </button>
              <button className="ghost-button" type="button" onClick={saveProfileToBrowser}>
                Save profile
              </button>
              <button className="ghost-button" type="button" onClick={loadProfileFromBrowser}>
                Load saved
              </button>
            </div>
            <label className="command-box profile-json-box">
              <span>Profile JSON</span>
              <textarea
                value={profileJson}
                rows={6}
                placeholder="Export a profile or paste one here to import it."
                onChange={(event) => setProfileJson(event.target.value)}
              />
            </label>
            {profileStatus ? <p className="status-line pending">{profileStatus}</p> : null}
          </section>

          <section className="control-group" aria-labelledby="prepare-title">
            <div className="section-heading">
              <h2 id="prepare-title">Prepare request</h2>
              <span>{forceRefresh ? 'fresh pull' : 'cache first'}</span>
            </div>
            <div className="form-grid">
              <label className="toggle-row">
                <span>Use elevation</span>
                <input
                  type="checkbox"
                  checked={useElevation}
                  disabled={isPreparing}
                  onChange={() => {
                    clearPreparedOutput();
                    setUseElevation((value) => !value);
                  }}
                />
              </label>
              <label className="toggle-row">
                <span>Force refresh</span>
                <input
                  type="checkbox"
                  checked={forceRefresh}
                  disabled={isPreparing}
                  onChange={() => {
                    clearPreparedOutput();
                    setForceRefresh((value) => !value);
                  }}
                />
              </label>
            </div>
            <button className="primary-action" type="button" onClick={handlePrepare} disabled={!selectedBbox || Boolean(spawnBboxError) || isPreparing}>
              {isPreparing ? 'Preparing…' : 'Prepare area'}
            </button>
            {prepareError ? (
              <div className="status-stack">
                <p className="status-line error">{prepareError}</p>
                {errorHintForMessage(prepareError) ? <p className="status-line hint">{errorHintForMessage(prepareError)}</p> : null}
              </div>
            ) : null}
            {isPreparing ? <p className="status-line pending">Request in flight. Large areas may take a moment.</p> : null}
          </section>

          {preparedArea ? (
            <PreparedOutputSection
              preparedArea={preparedArea}
              commandVariants={commandVariants}
              copiedCommandVariant={copiedCommandVariant}
              copyStatus={copyStatus}
              launchStatus={launchStatus}
              launchMessage={launchMessage}
              onCopyCommand={copyCommand}
              onLaunchRenderer={handleLaunchRenderer}
            />
          ) : null}
        </div>
      </section>

      <HelpOverlay />

      <MapPicker
        cachedAreas={cacheAreas}
        selectedBbox={selectedBbox}
        onBboxChange={handleBboxChange}
        spawnPoint={spawnPoint}
        onSpawnChange={handleSpawnChange}
        spawnMode={spawnMode}
        disabled={isPreparing}
      />

      <PromptDialog
        open={renameTarget !== null}
        title="Name this prepared area"
        defaultValue={renameTarget?.display_name ?? ''}
        confirmLabel="Save"
        onCancel={() => setRenameTarget(null)}
        onConfirm={(name) => {
          const entry = renameTarget;
          setRenameTarget(null);
          if (entry) {
            void renamePreparedEntry(entry, name);
          }
        }}
      />

      <ConfirmDialog
        open={deleteTarget !== null}
        title="Delete prepared area"
        message={`Delete prepared area "${deleteTarget?.display_name || deleteTarget?.cache_key.slice(0, 12)}" from the shared cache?`}
        confirmLabel="Delete"
        onCancel={() => setDeleteTarget(null)}
        onConfirm={() => {
          const entry = deleteTarget;
          setDeleteTarget(null);
          if (entry) {
            void deletePreparedEntry(entry);
          }
        }}
        dangerous
      />
    </main>
  );
}
