/**
 * Generates debug, release, screenshot, and no-streaming renderer command variants.
 *
 * @module commandVariants
 */

import type { PrepareAreaResponse } from './api';
import type { RendererOptions } from './settingsProfiles';

/** Subset of prepare response fields needed to build command variants. */
export type CommandVariantInput = Pick<
  PrepareAreaResponse,
  'cache_key' | 'command_cwd' | 'command_program' | 'command_args'
>;

/** A single command variant with id, label, description, and shell command string. */
export interface CommandVariant {
  id: 'debug' | 'release' | 'screenshot' | 'no-streaming';
  label: string;
  description: string;
  command: string;
}

function shellQuote(value: string): string {
  if (/^[A-Za-z0-9_./:=+,%@-]+$/.test(value)) {
    return value;
  }

  return `'${value.replaceAll("'", "'\\''")}'`;
}

function shellCommand(program: string, args: string[]): string {
  return [program, ...args].map(shellQuote).join(' ');
}

function insertCargoRelease(args: string[]): string[] {
  if (args[0] !== 'run' || args.includes('--release')) {
    return args;
  }

  return ['run', '--release', ...args.slice(1)];
}

/** Converts renderer options to an array of CLI flag strings. */
export function rendererOptionArgs(renderer: RendererOptions): string[] {
  const args: string[] = [];
  args.push('--time-of-day', renderer.timeOfDay.toString());
  args.push('--visual-preset', renderer.visualPreset);
  if (renderer.showSettings) {
    args.push('--show-settings');
  }
  if (renderer.streamRadius !== 15000) {
    args.push('--stream-radius', renderer.streamRadius.toString());
  }
  if (renderer.uploadBudgetMb !== 4) {
    args.push('--upload-budget-mb', renderer.uploadBudgetMb.toString());
  }
  if (renderer.maxUploadedTiles !== 256) {
    args.push('--max-uploaded-tiles', renderer.maxUploadedTiles.toString());
  }
  if (!renderer.labels.poi) {
    args.push('--hide-poi-labels');
  }
  if (!renderer.labels.addresses) {
    args.push('--hide-address-labels');
  }
  if (!renderer.labels.streetSigns) {
    args.push('--hide-street-sign-labels');
  }
  if (!renderer.minimap.visible) {
    args.push('--hide-minimap');
  }
  if (renderer.minimap.rotateWithCamera) {
    args.push('--rotate-minimap');
  }
  return args;
}

/** Builds all four command variants (debug, release, screenshot, no-streaming) from a prepare response. */
export function buildCommandVariants(input: CommandVariantInput, renderer: RendererOptions): CommandVariant[] {
  const debugArgs = [...input.command_args, ...rendererOptionArgs(renderer)];
  const screenshotPath = `screenshots/osm-world-${input.cache_key.slice(0, 12)}.png`;

  return [
    {
      id: 'debug',
      label: 'Debug',
      description: 'Fast local build with the selected renderer profile flags.',
      command: shellCommand(input.command_program, debugArgs),
    },
    {
      id: 'release',
      label: 'Release',
      description: 'Optimized renderer build for smoother city walkthroughs.',
      command: shellCommand(input.command_program, insertCargoRelease(debugArgs)),
    },
    {
      id: 'screenshot',
      label: 'Screenshot',
      description: 'Captures a PNG after startup, then exits for repeatable checks.',
      command: shellCommand(input.command_program, [
        ...debugArgs,
        '--screenshot',
        screenshotPath,
        '--screenshot-delay',
        '5',
        '--auto-exit',
        '8',
      ]),
    },
    {
      id: 'no-streaming',
      label: 'No streaming',
      description: 'Legacy single-mesh launch for comparing streaming artifacts.',
      command: shellCommand(input.command_program, [...debugArgs, '--no-streaming']),
    },
  ];
}
