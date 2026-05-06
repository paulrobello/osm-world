import { describe, expect, test } from 'bun:test';
import { errorHintForMessage } from './errorHints';

describe('error hints', () => {
  test('adds actionable hints for common preparation failures', () => {
    expect(errorHintForMessage('failed to fetch map data')).toContain('Overpass');
    expect(errorHintForMessage('bbox span exceeds limit')).toContain('smaller bbox');
    expect(errorHintForMessage('overture CLI not found on PATH')).toContain('Overture');
    expect(errorHintForMessage('failed to fetch elevation data')).toContain('Use elevation');
    expect(errorHintForMessage('Spawn point must be inside the selected bbox.')).toContain('Move the spawn point');
  });

  test('returns null when no specialized hint is known', () => {
    expect(errorHintForMessage('something surprising')).toBeNull();
  });
});
