import { describe, expect, test } from 'bun:test';
import { defaultFilter, defaultSourceControls } from './api';
import {
  DEFAULT_RENDERER_OPTIONS,
  exportSettingsProfile,
  importSettingsProfile,
  type SettingsProfile,
} from './settingsProfiles';

describe('settings profiles', () => {
  test('exports and imports renderer prep and launch settings', () => {
    const profile: SettingsProfile = {
      version: 1,
      name: 'Golden hour labels',
      filter: { ...defaultFilter, railways: false },
      sourceControls: { ...defaultSourceControls, poi_source_mode: 'both', overture_timeout: 45 },
      useElevation: true,
      forceRefresh: false,
      renderer: {
        ...DEFAULT_RENDERER_OPTIONS,
        timeOfDay: 18.25,
        visualPreset: 'showcase',
        labels: { poi: true, addresses: false, streetSigns: true },
        minimap: { visible: true, rotateWithCamera: false },
      },
    };

    const json = exportSettingsProfile(profile);
    const imported = importSettingsProfile(json);

    expect(imported).toEqual(profile);
  });

  test('rejects profiles with missing required sections', () => {
    expect(() => importSettingsProfile('{"version":1}')).toThrow('settings profile');
  });
});
