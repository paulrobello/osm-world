'use client';

import dynamic from 'next/dynamic';
import { useCallback, useEffect, useMemo, useState } from 'react';
import {
  API_URL,
  defaultFilter,
  fetchCacheAreas,
  fetchHealth,
  prepareArea,
  type CacheEntry,
  type FeatureFilter,
  type PrepareAreaResponse,
} from '@/lib/api';
import type { BBox } from '@/components/MapPicker';

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

type HealthState = Awaited<ReturnType<typeof fetchHealth>>;

function formatBbox(bbox: BBox | null): string {
  if (!bbox) {
    return 'No bbox selected';
  }

  return bbox.map((value) => value.toFixed(5)).join(', ');
}

function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) {
    return '0 B';
  }

  const units = ['B', 'KB', 'MB', 'GB'];
  const power = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
  return `${(bytes / 1024 ** power).toFixed(power === 0 ? 0 : 1)} ${units[power]}`;
}

export default function Home() {
  const [health, setHealth] = useState<HealthState | null>(null);
  const [cacheAreas, setCacheAreas] = useState<CacheEntry[]>([]);
  const [selectedBbox, setSelectedBbox] = useState<BBox | null>(null);
  const [manualBbox, setManualBbox] = useState({
    south: '',
    west: '',
    north: '',
    east: '',
  });
  const [filter, setFilter] = useState<FeatureFilter>(defaultFilter);
  const [useElevation, setUseElevation] = useState(false);
  const [forceRefresh, setForceRefresh] = useState(false);
  const [loadingMeta, setLoadingMeta] = useState(true);
  const [metaError, setMetaError] = useState<string | null>(null);
  const [prepareError, setPrepareError] = useState<string | null>(null);
  const [isPreparing, setIsPreparing] = useState(false);
  const [preparedArea, setPreparedArea] = useState<PrepareAreaResponse | null>(null);
  const [copyStatus, setCopyStatus] = useState<'idle' | 'copied' | 'failed'>('idle');

  const selectedFeatureCount = useMemo(
    () => Object.values(filter).filter(Boolean).length,
    [filter],
  );

  const cacheTotalSize = useMemo(
    () => cacheAreas.reduce((sum, area) => sum + area.size_bytes, 0),
    [cacheAreas],
  );

  const loadMeta = useCallback(async () => {
    setLoadingMeta(true);
    setMetaError(null);

    const [healthResult, cacheResult] = await Promise.allSettled([fetchHealth(), fetchCacheAreas()]);

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

  const toggleFeature = (name: keyof FeatureFilter) => {
    if (isPreparing) {
      return;
    }
    setFilter((current) => ({ ...current, [name]: !current[name] }));
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

    setPrepareError(null);
    setSelectedBbox([south, west, north, east]);
  };

  const handlePrepare = async () => {
    if (!selectedBbox) {
      setPrepareError('Draw a bounding box before preparing area data.');
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
      });
      setPreparedArea(prepared);
      const refreshedAreas = await fetchCacheAreas().catch(() => null);
      if (refreshedAreas) {
        setCacheAreas(refreshedAreas);
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
            {metaError ? <p className="status-line error">{metaError}</p> : null}
            <button className="ghost-button" type="button" onClick={() => void loadMeta()} disabled={loadingMeta}>
              Refresh telemetry
            </button>
          </section>

          <section className="bbox-readout" aria-labelledby="bbox-title">
            <h2 id="bbox-title">Selected bbox</h2>
            <code>[south, west, north, east]</code>
            <output>{formatBbox(selectedBbox)}</output>
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

          <section className="control-group" aria-labelledby="prepare-title">
            <div className="section-heading">
              <h2 id="prepare-title">Prepare request</h2>
              <span>{forceRefresh ? 'fresh pull' : 'cache first'}</span>
            </div>
            <div className="form-grid">
              <label className="toggle-row">
                <span>Use elevation</span>
                <input type="checkbox" checked={useElevation} disabled={isPreparing} onChange={() => setUseElevation((value) => !value)} />
              </label>
              <label className="toggle-row">
                <span>Force refresh</span>
                <input type="checkbox" checked={forceRefresh} disabled={isPreparing} onChange={() => setForceRefresh((value) => !value)} />
              </label>
            </div>
            <button className="primary-action" type="button" onClick={handlePrepare} disabled={!selectedBbox || isPreparing}>
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
              </dl>
              <label className="command-box">
                <span>Launch command</span>
                <textarea readOnly value={preparedArea.command} rows={4} />
              </label>
              <button className="ghost-button copy-button" type="button" onClick={() => void copyCommand()}>
                {copyStatus === 'copied' ? 'Copied command' : 'Copy command'}
              </button>
              {copyStatus === 'failed' ? <p className="status-line error">Clipboard permission denied. Select the command manually.</p> : null}
            </section>
          ) : null}
        </div>
      </section>

      <MapPicker cachedAreas={cacheAreas} selectedBbox={selectedBbox} onBboxChange={setSelectedBbox} disabled={isPreparing} />
    </main>
  );
}
