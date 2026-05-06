'use client';

import dynamic from 'next/dynamic';
import { useCallback, useEffect, useMemo, useState } from 'react';
import {
  API_URL,
  defaultFilter,
  defaultSourceControls,
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
import type { BBox, SpawnPoint } from '@/components/MapPicker';

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

function sourceStatusLabel(status: string): string {
  return status.replaceAll('_', ' ');
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
  const [loadingMeta, setLoadingMeta] = useState(true);
  const [metaError, setMetaError] = useState<string | null>(null);
  const [prepareError, setPrepareError] = useState<string | null>(null);
  const [isPreparing, setIsPreparing] = useState(false);
  const [preparedArea, setPreparedArea] = useState<PrepareAreaResponse | null>(null);
  const [copyStatus, setCopyStatus] = useState<'idle' | 'copied' | 'failed'>('idle');
  const [launchStatus, setLaunchStatus] = useState<'idle' | 'launching' | 'launched' | 'failed'>('idle');
  const [launchMessage, setLaunchMessage] = useState<string | null>(null);

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

  const clearPreparedOutput = useCallback(() => {
    setPreparedArea(null);
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
    setCopyStatus('idle');
    setLaunchStatus('idle');
    setLaunchMessage(null);
  };

  const renamePreparedEntry = async (entry: PreparedAreaEntry) => {
    const displayName = window.prompt('Name this prepared area', entry.display_name ?? '');
    if (displayName === null) {
      return;
    }
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

  const handleBboxChange = (bbox: BBox) => {
    if (isPreparing) {
      return;
    }
    clearPreparedOutput();
    setPrepareError(null);
    setSelectedBbox(bbox);
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

  const copyCommand = async () => {
    if (!preparedArea?.command) {
      return;
    }

    try {
      await navigator.clipboard.writeText(preparedArea.command);
      setCopyStatus('copied');
    } catch {
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
      });
      setLaunchStatus('launched');
      setLaunchMessage(`Renderer launched as pid ${result.pid}.`);
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
              <strong>overpass</strong>
              <span>{health?.overpass_cache_dir ?? 'unavailable'}</span>
            </div>
            <div className="console-line">
              <strong>srtm</strong>
              <span>{health?.srtm_cache_dir ?? 'unavailable'}</span>
            </div>
            <div className="console-line">
              <strong>areas</strong>
              <span>{cacheAreas.length} cached · {formatBytes(cacheTotalSize)}</span>
            </div>
            <div className="console-line">
              <strong>prepared</strong>
              <span>{preparedAreas.length} saved · {favoritePreparedCount} pinned</span>
            </div>
            {metaError ? <p className="status-line error">{metaError}</p> : null}
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

          <section className="control-group" aria-labelledby="history-title">
            <div className="section-heading">
              <h2 id="history-title">Prepared history</h2>
              <span>{preparedAreas.length} cached</span>
            </div>
            {preparedAreas.length === 0 ? (
              <p className="microcopy">Prepared areas will appear here after the first successful prepare request.</p>
            ) : (
              <div className="history-list">
                {preparedAreas.map((entry) => (
                  <article className="history-entry" key={entry.cache_key}>
                    <button className="history-main" type="button" onClick={() => loadPreparedEntry(entry)} disabled={isPreparing}>
                      <strong>{entry.display_name || entry.cache_key.slice(0, 10)}</strong>
                      <span>{sourceStatusLabel(entry.source_status)} · {formatBbox(entry.bbox)}</span>
                    </button>
                    <div className="history-actions">
                      <button className="mini-button" type="button" onClick={() => void togglePreparedFavorite(entry)}>
                        {entry.favorite ? '★' : '☆'}
                      </button>
                      <button className="mini-button" type="button" onClick={() => void renamePreparedEntry(entry)}>
                        Name
                      </button>
                    </div>
                  </article>
                ))}
              </div>
            )}
          </section>

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
            {prepareError ? <p className="status-line error">{prepareError}</p> : null}
            {isPreparing ? <p className="status-line pending">Request in flight. Large areas may take a moment.</p> : null}
          </section>

          {preparedArea ? (
            <section className="result-card" aria-labelledby="result-title">
              <div className="section-heading">
                <h2 id="result-title">Prepared output</h2>
                <span>{preparedArea.cache_status}</span>
              </div>
              <dl className="result-list">
                <div>
                  <dt>OSM path</dt>
                  <dd>{preparedArea.osm_path}</dd>
                </div>
                {preparedArea.srtm_dir ? (
                  <div>
                    <dt>SRTM dir</dt>
                    <dd>{preparedArea.srtm_dir}</dd>
                  </div>
                ) : null}
                <div>
                  <dt>Cache key</dt>
                  <dd>{preparedArea.cache_key}</dd>
                </div>
                <div>
                  <dt>Source status</dt>
                  <dd>{sourceStatusLabel(preparedArea.source_status)}</dd>
                </div>
                {preparedArea.spawn_lat !== null && preparedArea.spawn_lon !== null ? (
                  <div>
                    <dt>Spawn point</dt>
                    <dd>{preparedArea.spawn_lat.toFixed(6)}, {preparedArea.spawn_lon.toFixed(6)}</dd>
                  </div>
                ) : null}
              </dl>
              {preparedArea.warnings.length > 0 ? (
                <div className="warning-stack" role="status">
                  {preparedArea.warnings.map((warning) => (
                    <p className="status-line pending" key={warning}>{warning}</p>
                  ))}
                </div>
              ) : null}
              <label className="command-box">
                <span>Launch command</span>
                <textarea readOnly value={preparedArea.command} rows={4} />
              </label>
              <div className="button-row result-actions">
                <button className="ghost-button copy-button" type="button" onClick={() => void copyCommand()}>
                  {copyStatus === 'copied' ? 'Copied command' : 'Copy command'}
                </button>
                <button className="ghost-button copy-button" type="button" onClick={() => void handleLaunchRenderer()} disabled={launchStatus === 'launching'}>
                  {launchStatus === 'launching' ? 'Launching…' : 'Launch renderer'}
                </button>
              </div>
              {copyStatus === 'failed' ? <p className="status-line error">Clipboard permission denied. Select the command manually.</p> : null}
              {launchMessage ? (
                <p className={`status-line ${launchStatus === 'failed' ? 'error' : 'success'}`}>{launchMessage}</p>
              ) : null}
            </section>
          ) : null}
        </div>
      </section>

      <MapPicker
        cachedAreas={cacheAreas}
        selectedBbox={selectedBbox}
        onBboxChange={handleBboxChange}
        spawnPoint={spawnPoint}
        onSpawnChange={handleSpawnChange}
        spawnMode={spawnMode}
        disabled={isPreparing}
      />
    </main>
  );
}
