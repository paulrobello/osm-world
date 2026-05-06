import { describe, expect, test } from 'bun:test';
import { buildCommandVariants, type CommandVariantInput } from './commandVariants';
import { DEFAULT_RENDERER_OPTIONS } from './settingsProfiles';

const baseCommand: CommandVariantInput = {
  cache_key: 'abc123',
  command_cwd: '/tmp/osm world',
  command_program: 'cargo',
  command_args: [
    'run',
    '--manifest-path',
    '/tmp/osm world/Cargo.toml',
    '--',
    '--input',
    '/tmp/prepared/city.osm',
    '--spawn-lat',
    '38.5',
    '--spawn-lon',
    '-121.5',
  ],
};

describe('command variants', () => {
  test('builds debug, release, screenshot, and no-streaming launch commands', () => {
    const variants = buildCommandVariants(baseCommand, {
      ...DEFAULT_RENDERER_OPTIONS,
      timeOfDay: 18.5,
      visualPreset: 'showcase',
      showSettings: true,
      streamRadius: 9000,
      uploadBudgetMb: 8,
      maxUploadedTiles: 128,
    });

    expect(variants.map((variant) => variant.id)).toEqual(['debug', 'release', 'screenshot', 'no-streaming']);
    expect(variants[0].command).toContain("cargo run --manifest-path '/tmp/osm world/Cargo.toml'");
    expect(variants[0].command).toContain('--time-of-day 18.5');
    expect(variants[0].command).toContain('--visual-preset showcase');
    expect(variants[0].command).toContain('--show-settings');
    expect(variants[0].command).toContain('--stream-radius 9000');
    expect(variants[0].command).toContain('--upload-budget-mb 8');
    expect(variants[0].command).toContain('--max-uploaded-tiles 128');
    expect(variants[1].command).toContain('cargo run --release --manifest-path');
    expect(variants[2].command).toContain('--screenshot screenshots/osm-world-abc123.png');
    expect(variants[2].command).toContain('--auto-exit 8');
    expect(variants[3].command).toContain('--no-streaming');
  });

  test('adds label and minimap startup flags from the renderer profile', () => {
    const variants = buildCommandVariants(baseCommand, {
      ...DEFAULT_RENDERER_OPTIONS,
      labels: { poi: false, addresses: false, streetSigns: false },
      minimap: { visible: false, rotateWithCamera: true },
    });

    expect(variants[0].command).toContain('--hide-poi-labels');
    expect(variants[0].command).toContain('--hide-address-labels');
    expect(variants[0].command).toContain('--hide-street-sign-labels');
    expect(variants[0].command).toContain('--hide-minimap');
    expect(variants[0].command).toContain('--rotate-minimap');
  });
});
