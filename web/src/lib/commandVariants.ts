import type { PrepareAreaResponse } from './api';
import type { RendererOptions } from './settingsProfiles';

export type CommandVariantInput = Pick<
  PrepareAreaResponse,
  'cache_key' | 'command_cwd' | 'command_program' | 'command_args'
>;

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

function appendRendererOptions(args: string[], renderer: RendererOptions): string[] {
  const next = [...args];
  next.push('--time-of-day', renderer.timeOfDay.toString());
  next.push('--visual-preset', renderer.visualPreset);
  if (renderer.showSettings) {
    next.push('--show-settings');
  }
  if (renderer.streamRadius !== 15000) {
    next.push('--stream-radius', renderer.streamRadius.toString());
  }
  if (renderer.uploadBudgetMb !== 4) {
    next.push('--upload-budget-mb', renderer.uploadBudgetMb.toString());
  }
  if (renderer.maxUploadedTiles !== 256) {
    next.push('--max-uploaded-tiles', renderer.maxUploadedTiles.toString());
  }
  if (!renderer.labels.poi) {
    next.push('--hide-poi-labels');
  }
  if (!renderer.labels.addresses) {
    next.push('--hide-address-labels');
  }
  if (!renderer.labels.streetSigns) {
    next.push('--hide-street-sign-labels');
  }
  if (!renderer.minimap.visible) {
    next.push('--hide-minimap');
  }
  if (renderer.minimap.rotateWithCamera) {
    next.push('--rotate-minimap');
  }
  return next;
}

export function buildCommandVariants(input: CommandVariantInput, renderer: RendererOptions): CommandVariant[] {
  const debugArgs = appendRendererOptions(input.command_args, renderer);
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
