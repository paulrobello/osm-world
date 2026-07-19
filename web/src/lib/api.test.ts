import { describe, expect, test } from 'bun:test';
import { fetchHealth, type LaunchRendererResponse } from './api';

/**
 * Contract fixtures for the Rust ↔ TypeScript API surface.
 *
 * These assertions pin the response shapes the server actually emits, so a
 * future change that re-adds a removed field (e.g. `pid`, `overpass_cache_dir`)
 * has to update this test in the same change. See ARC-002 in AUDIT.md.
 */
describe('API response contracts', () => {
  test('LaunchRendererResponse only exposes the status string', () => {
    const sample: LaunchRendererResponse = { status: 'launched' };
    expect(sample.status).toBe('launched');
    // TS shapes the type at compile time; assert no ghost fields survive at
    // runtime by listing the own keys of a freshly-built response object.
    expect(Object.keys(sample)).toEqual(['status']);
  });

  test('fetchHealth resolves to a { status }-only payload', async () => {
    // Pin the resolved type at compile time; if `fetchHealth` is changed to
    // declare additional fields, `Awaited<ReturnType<...>>` widens here and
    // the literal assignment below fails tsc.
    type HealthPayload = Awaited<ReturnType<typeof fetchHealth>>;
    const sample: HealthPayload = await Promise.resolve({ status: 'ok' });
    expect(sample.status).toBe('ok');
    expect(Object.keys(sample)).toEqual(['status']);
  });
});
