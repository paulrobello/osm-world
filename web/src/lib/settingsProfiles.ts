import { defaultFilter, defaultSourceControls, type FeatureFilter, type SourceControls } from './api';

export type VisualPreset = 'performance' | 'balanced' | 'showcase';

export interface RendererOptions {
  timeOfDay: number;
  visualPreset: VisualPreset;
  showSettings: boolean;
  streamRadius: number;
  uploadBudgetMb: number;
  maxUploadedTiles: number;
  labels: {
    poi: boolean;
    addresses: boolean;
    streetSigns: boolean;
  };
  minimap: {
    visible: boolean;
    rotateWithCamera: boolean;
  };
}

export interface SettingsProfile {
  version: 1;
  name: string;
  filter: FeatureFilter;
  sourceControls: SourceControls;
  useElevation: boolean;
  forceRefresh: boolean;
  renderer: RendererOptions;
}

export const DEFAULT_RENDERER_OPTIONS: RendererOptions = {
  timeOfDay: 14,
  visualPreset: 'balanced',
  showSettings: false,
  streamRadius: 15000,
  uploadBudgetMb: 4,
  maxUploadedTiles: 256,
  labels: {
    poi: true,
    addresses: true,
    streetSigns: true,
  },
  minimap: {
    visible: true,
    rotateWithCamera: true,
  },
};

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function assertSettingsProfile(value: unknown): asserts value is SettingsProfile {
  if (!isRecord(value) || value.version !== 1 || typeof value.name !== 'string') {
    throw new Error('Invalid settings profile: expected version 1 profile with a name.');
  }
  if (!isRecord(value.filter) || !isRecord(value.sourceControls) || !isRecord(value.renderer)) {
    throw new Error('Invalid settings profile: missing required settings profile sections.');
  }
}

export function createSettingsProfile(input: Omit<SettingsProfile, 'version'>): SettingsProfile {
  return {
    version: 1,
    ...input,
  };
}

export function exportSettingsProfile(profile: SettingsProfile): string {
  return `${JSON.stringify(profile, null, 2)}\n`;
}

export function importSettingsProfile(json: string): SettingsProfile {
  const parsed: unknown = JSON.parse(json);
  assertSettingsProfile(parsed);

  return {
    version: 1,
    name: parsed.name,
    filter: { ...defaultFilter, ...parsed.filter },
    sourceControls: { ...defaultSourceControls, ...parsed.sourceControls },
    useElevation: Boolean(parsed.useElevation),
    forceRefresh: Boolean(parsed.forceRefresh),
    renderer: {
      ...DEFAULT_RENDERER_OPTIONS,
      ...parsed.renderer,
      labels: {
        ...DEFAULT_RENDERER_OPTIONS.labels,
        ...(isRecord(parsed.renderer.labels) ? parsed.renderer.labels : {}),
      },
      minimap: {
        ...DEFAULT_RENDERER_OPTIONS.minimap,
        ...(isRecord(parsed.renderer.minimap) ? parsed.renderer.minimap : {}),
      },
    },
  };
}

export function defaultSettingsProfile(): SettingsProfile {
  return createSettingsProfile({
    name: 'Default renderer profile',
    filter: defaultFilter,
    sourceControls: defaultSourceControls,
    useElevation: false,
    forceRefresh: false,
    renderer: DEFAULT_RENDERER_OPTIONS,
  });
}
