/**
 * Interactive OpenLayers map picker for selecting an area to prepare.
 *
 * Renders an OSM base layer plus a bounding-box draw interaction and an
 * optional spawn-point marker. The drawn bbox and spawn coordinate are
 * reported back through the `onBboxChange` and `onSpawnChange` callbacks so
 * the parent page can drive prepare/launch actions. Also overlays cached
 * prepared-area tiles when a cache directory is provided.
 */
'use client';

import { useEffect, useRef } from 'react';
import Feature from 'ol/Feature';
import Map from 'ol/Map';
import View from 'ol/View';
import { fromLonLat, toLonLat, transformExtent } from 'ol/proj';
import TileLayer from 'ol/layer/Tile';
import VectorLayer from 'ol/layer/Vector';
import OSM from 'ol/source/OSM';
import VectorSource from 'ol/source/Vector';
import Draw, { createBox } from 'ol/interaction/Draw';
import { Circle as CircleStyle, Fill, Stroke, Style } from 'ol/style';
import Point from 'ol/geom/Point';
import { fromExtent } from 'ol/geom/Polygon';
import type { Geometry } from 'ol/geom';
import { unByKey } from 'ol/Observable';
import type { CacheEntry } from '@/lib/api';

export type BBox = [south: number, west: number, north: number, east: number];
export type SpawnPoint = { lat: number; lon: number };

interface MapPickerProps {
  cachedAreas: CacheEntry[];
  selectedBbox: BBox | null;
  onBboxChange: (bbox: BBox) => void;
  spawnPoint: SpawnPoint | null;
  onSpawnChange: (spawnPoint: SpawnPoint) => void;
  spawnMode: boolean;
  disabled?: boolean;
}

const SACRAMENTO_CENTER: [number, number] = [-121.4944, 38.5816];

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

function normalizeBbox([south, west, north, east]: BBox): BBox {
  return [
    clamp(Math.min(south, north), -90, 90),
    clamp(Math.min(west, east), -180, 180),
    clamp(Math.max(south, north), -90, 90),
    clamp(Math.max(west, east), -180, 180),
  ];
}

function normalizeSpawnPoint({ lat, lon }: SpawnPoint): SpawnPoint {
  return {
    lat: clamp(lat, -90, 90),
    lon: clamp(lon, -180, 180),
  };
}

function bboxToFeature(bbox: BBox, label: string): Feature {
  const [south, west, north, east] = normalizeBbox(bbox);
  const mercatorExtent = transformExtent([west, south, east, north], 'EPSG:4326', 'EPSG:3857');
  return new Feature({ geometry: fromExtent(mercatorExtent), label });
}

function spawnToFeature(spawnPoint: SpawnPoint): Feature {
  const { lat, lon } = normalizeSpawnPoint(spawnPoint);
  return new Feature({ geometry: new Point(fromLonLat([lon, lat])), label: 'spawn point' });
}

function extentToBbox(geometry: Geometry): BBox {
  const [west, south, east, north] = transformExtent(geometry.getExtent(), 'EPSG:3857', 'EPSG:4326');
  return normalizeBbox([south, west, north, east]);
}

const selectedStyle = new Style({
  fill: new Fill({ color: 'rgba(101, 240, 162, 0.16)' }),
  stroke: new Stroke({ color: '#65f0a2', width: 3, lineDash: [10, 8] }),
});

const cachedStyle = new Style({
  fill: new Fill({ color: 'rgba(255, 178, 62, 0.1)' }),
  stroke: new Stroke({ color: 'rgba(255, 178, 62, 0.82)', width: 2, lineDash: [4, 7] }),
});

const drawStyle = new Style({
  fill: new Fill({ color: 'rgba(101, 240, 162, 0.22)' }),
  stroke: new Stroke({ color: '#e8f6dc', width: 2 }),
});

const spawnStyle = new Style({
  image: new CircleStyle({
    radius: 8,
    fill: new Fill({ color: '#ff4fd8' }),
    stroke: new Stroke({ color: '#fff0fb', width: 3 }),
  }),
});

export default function MapPicker({
  cachedAreas,
  selectedBbox,
  onBboxChange,
  spawnPoint,
  onSpawnChange,
  spawnMode,
  disabled = false,
}: MapPickerProps) {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const mapRef = useRef<Map | null>(null);
  const selectedSourceRef = useRef<VectorSource | null>(null);
  const cacheSourceRef = useRef<VectorSource | null>(null);
  const spawnSourceRef = useRef<VectorSource | null>(null);
  const drawRef = useRef<Draw | null>(null);
  const onBboxChangeRef = useRef(onBboxChange);
  const onSpawnChangeRef = useRef(onSpawnChange);
  const disabledRef = useRef(disabled);
  const spawnModeRef = useRef(spawnMode);

  useEffect(() => {
    onBboxChangeRef.current = onBboxChange;
  }, [onBboxChange]);

  useEffect(() => {
    onSpawnChangeRef.current = onSpawnChange;
  }, [onSpawnChange]);

  useEffect(() => {
    disabledRef.current = disabled;
  }, [disabled]);

  useEffect(() => {
    spawnModeRef.current = spawnMode;
  }, [spawnMode]);

  useEffect(() => {
    if (!containerRef.current || mapRef.current) {
      return;
    }

    const selectedSource = new VectorSource();
    const cacheSource = new VectorSource();
    const spawnSource = new VectorSource();
    selectedSourceRef.current = selectedSource;
    cacheSourceRef.current = cacheSource;
    spawnSourceRef.current = spawnSource;

    const cacheLayer = new VectorLayer({ source: cacheSource, style: cachedStyle, zIndex: 5 });
    const selectedLayer = new VectorLayer({ source: selectedSource, style: selectedStyle, zIndex: 10 });
    const spawnLayer = new VectorLayer({ source: spawnSource, style: spawnStyle, zIndex: 15 });

    const map = new Map({
      target: containerRef.current,
      layers: [
        new TileLayer({ source: new OSM(), zIndex: 0 }),
        cacheLayer,
        selectedLayer,
        spawnLayer,
      ],
      view: new View({
        center: fromLonLat(SACRAMENTO_CENTER),
        zoom: 11,
        minZoom: 3,
        maxZoom: 18,
      }),
      controls: undefined,
    });

    const draw = new Draw({
      source: selectedSource,
      type: 'Circle',
      geometryFunction: createBox(),
      style: drawStyle,
    });

    draw.on('drawstart', () => {
      selectedSource.clear();
    });

    draw.on('drawend', (event) => {
      const geometry = event.feature.getGeometry();
      if (!geometry) {
        return;
      }
      onBboxChangeRef.current(extentToBbox(geometry));
    });

    const singleClickKey = map.on('singleclick', (event) => {
      if (disabledRef.current || !spawnModeRef.current) {
        return;
      }

      const [lon, lat] = toLonLat(event.coordinate);
      onSpawnChangeRef.current(normalizeSpawnPoint({ lat, lon }));
    });

    draw.setActive(!disabled && !spawnMode);
    map.addInteraction(draw);
    drawRef.current = draw;
    mapRef.current = map;

    return () => {
      unByKey(singleClickKey);
      map.removeInteraction(draw);
      selectedSource.clear();
      cacheSource.clear();
      spawnSource.clear();
      map.setTarget(undefined);
      map.dispose();
      mapRef.current = null;
      selectedSourceRef.current = null;
      cacheSourceRef.current = null;
      spawnSourceRef.current = null;
      drawRef.current = null;
    };
  }, []);

  useEffect(() => {
    drawRef.current?.setActive(!disabled && !spawnMode);
  }, [disabled, spawnMode]);

  useEffect(() => {
    const cacheSource = cacheSourceRef.current;
    if (!cacheSource) {
      return;
    }

    cacheSource.clear();
    for (const area of cachedAreas) {
      cacheSource.addFeature(bboxToFeature(area.bbox, area.key));
    }
  }, [cachedAreas]);

  useEffect(() => {
    const selectedSource = selectedSourceRef.current;
    if (!selectedSource) {
      return;
    }

    selectedSource.clear();
    if (selectedBbox) {
      const feature = bboxToFeature(selectedBbox, 'selected bbox');
      selectedSource.addFeature(feature);
      const geometry = feature.getGeometry();
      if (geometry) {
        mapRef.current?.getView().fit(geometry.getExtent(), {
          padding: [60, 60, 60, 60],
          maxZoom: 15,
          duration: 250,
        });
      }
    }
  }, [selectedBbox]);

  useEffect(() => {
    const spawnSource = spawnSourceRef.current;
    if (!spawnSource) {
      return;
    }

    spawnSource.clear();
    if (spawnPoint) {
      spawnSource.addFeature(spawnToFeature(spawnPoint));
    }
  }, [spawnPoint]);

  return (
    <section className="map-canvas" aria-label="Interactive OpenStreetMap bounding box and spawn point picker">
      <div
        ref={containerRef}
        className={`ol-map${disabled ? ' ol-map-disabled' : ''}`}
        tabIndex={disabled ? -1 : 0}
        role="application"
        aria-label="Interactive map bbox drawing and spawn point selection surface"
        aria-describedby="map-picker-instructions"
        aria-disabled={disabled}
      />
      <div className="map-hud" aria-hidden="true">
        <span>{disabled ? 'Preparing' : spawnMode ? 'Spawn mode' : 'Draw mode'}</span>
        <strong>
          {disabled
            ? 'bbox and spawn edits locked during request'
            : spawnMode
              ? 'click the map to place the spawn point'
              : 'drag a rectangle to replace the active bbox'}
        </strong>
      </div>
      <p id="map-picker-instructions" className="sr-only">
        Pointer users can drag a rectangle on the map. Enable set-on-map spawn mode to click a spawn point. Keyboard users can enter south, west, north, east, spawn latitude, and spawn longitude values in the manual forms.
      </p>
      <div className="map-legend" aria-label="Map overlay legend">
        <span><i className="legend-swatch selected" /> selected bbox</span>
        <span><i className="legend-swatch cached" /> cached area</span>
        <span><i className="legend-swatch spawn" /> spawn point</span>
      </div>
    </section>
  );
}
