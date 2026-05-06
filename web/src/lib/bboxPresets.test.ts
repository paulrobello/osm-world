import { describe, expect, test } from 'bun:test';
import { BBOX_PRESETS, findBboxPreset } from './bboxPresets';

describe('bbox presets', () => {
  test('includes Sacramento and Woodland quick-select areas', () => {
    expect(BBOX_PRESETS.map((preset) => preset.id)).toContain('sacramento');
    expect(BBOX_PRESETS.map((preset) => preset.id)).toContain('woodland');
  });

  test('presets use normalized south west north east ordering', () => {
    for (const preset of BBOX_PRESETS) {
      const [south, west, north, east] = preset.bbox;
      expect(south).toBeLessThan(north);
      expect(west).toBeLessThan(east);
    }
  });

  test('findBboxPreset returns the requested preset', () => {
    expect(findBboxPreset('woodland')?.label).toContain('Woodland');
    expect(findBboxPreset('missing')).toBeUndefined();
  });
});
