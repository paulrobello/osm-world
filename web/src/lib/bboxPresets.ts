import type { BBox, SpawnPoint } from '../components/MapPicker';

export type BboxPreset = {
  id: string;
  label: string;
  description: string;
  bbox: BBox;
  spawnPoint?: SpawnPoint;
};

export const BBOX_PRESETS: BboxPreset[] = [
  {
    id: 'sacramento',
    label: 'Sacramento Midtown',
    description: 'Compact downtown/midtown Sacramento test area',
    bbox: [38.568, -121.505, 38.592, -121.475],
    spawnPoint: { lat: 38.5816, lon: -121.4944 },
  },
  {
    id: 'woodland',
    label: 'Woodland Downtown',
    description: 'Small Woodland area used for repeated renderer checks',
    bbox: [38.669, -121.785, 38.686, -121.755],
    spawnPoint: { lat: 38.677279, lon: -121.753596 },
  },
];

export function findBboxPreset(id: string): BboxPreset | undefined {
  return BBOX_PRESETS.find((preset) => preset.id === id);
}
