> ⚠️ Historical implementation plan (2026-05) — retained for reference; current behavior may differ. See `docs/ARCHITECTURE.md` and the source code.

# osm-world Web Picker Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a simplified web frontend for selecting real-world map areas, preparing shared-cache OSM/SRTM data through the osm-world API, and copying the generated launch command.

**Architecture:** Add a new `web/` Next.js app adapted conceptually from `osm-to-bedrock` but stripped of Minecraft export controls. The web app talks to the Rust API server (`cargo run -- --serve`) through a configurable API URL and uses OpenLayers for search/draw bbox interactions.

**Tech Stack:** Next.js, React, TypeScript, OpenLayers, Bun, existing Rust `--serve` API.

---

## File structure

- Create `web/package.json` — scripts and dependencies.
- Create `web/next.config.ts`, `web/tsconfig.json`, `web/postcss.config.mjs`, `web/src/app/layout.tsx`, `web/src/app/page.tsx`, `web/src/app/globals.css`.
- Create `web/src/components/MapPicker.tsx` — OpenLayers map, bbox drawing, cache overlay.
- Create `web/src/lib/api.ts` — typed calls to osm-world API.
- Modify `Makefile` — add `web-install`, `web-dev`, `web-build`, and `dev` helpers.

---

### Task 1: Scaffold the osm-world web app

**Files:**
- Create: `web/package.json`
- Create: `web/next.config.ts`
- Create: `web/tsconfig.json`
- Create: `web/postcss.config.mjs`
- Create: `web/src/app/layout.tsx`
- Create: `web/src/app/globals.css`
- Create: `web/src/lib/api.ts`

- [ ] **Step 1: Create package and config files**

Use Next.js with port `8032` to avoid osm-to-bedrock's `8031`.

`web/package.json`:

```json
{
  "name": "osm-world-web",
  "version": "0.1.0",
  "private": true,
  "scripts": {
    "dev": "next dev -p 8032",
    "build": "next build",
    "start": "next start -p 8032",
    "lint": "next lint"
  },
  "dependencies": {
    "next": "16.2.1",
    "ol": "^10.8.0",
    "react": "19.2.4",
    "react-dom": "19.2.4"
  },
  "devDependencies": {
    "@types/node": "^25.5.0",
    "@types/react": "^19.2.14",
    "@types/react-dom": "^19.2.3",
    "typescript": "^6.0.2"
  }
}
```

`web/next.config.ts`:

```ts
import type { NextConfig } from 'next';

const nextConfig: NextConfig = {};

export default nextConfig;
```

`web/tsconfig.json`:

```json
{
  "compilerOptions": {
    "target": "ES2017",
    "lib": ["dom", "dom.iterable", "esnext"],
    "allowJs": false,
    "skipLibCheck": true,
    "strict": true,
    "noEmit": true,
    "esModuleInterop": true,
    "module": "esnext",
    "moduleResolution": "bundler",
    "resolveJsonModule": true,
    "isolatedModules": true,
    "jsx": "react-jsx",
    "incremental": true,
    "plugins": [{ "name": "next" }],
    "paths": { "@/*": ["./src/*"] }
  },
  "include": ["next-env.d.ts", "**/*.ts", "**/*.tsx", ".next/types/**/*.ts"],
  "exclude": ["node_modules"]
}
```

`web/postcss.config.mjs`:

```js
const config = {};
export default config;
```

- [ ] **Step 2: Add app shell and API helpers**

`web/src/app/layout.tsx`:

```tsx
import './globals.css';
import type { Metadata } from 'next';

export const metadata: Metadata = {
  title: 'osm-world Area Picker',
  description: 'Prepare real-world OSM data for osm-world',
};

export default function RootLayout({ children }: Readonly<{ children: React.ReactNode }>) {
  return (
    <html lang="en">
      <body>{children}</body>
    </html>
  );
}
```

`web/src/lib/api.ts`:

```ts
export const API_URL = process.env.NEXT_PUBLIC_OSM_WORLD_API_URL ?? 'http://127.0.0.1:3030';

export interface FeatureFilter {
  roads: boolean;
  buildings: boolean;
  water: boolean;
  landuse: boolean;
  railways: boolean;
}

export const defaultFilter: FeatureFilter = {
  roads: true,
  buildings: true,
  water: true,
  landuse: true,
  railways: true,
};

export interface CacheEntry {
  key: string;
  bbox: [number, number, number, number];
  created_at: string;
  size_bytes: number;
}

export interface PrepareAreaResponse {
  bbox: [number, number, number, number];
  cache_key: string;
  cache_status: string;
  osm_path: string;
  srtm_dir: string | null;
  command: string;
  command_cwd: string;
  command_program: string;
  command_args: string[];
}

async function apiJson<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(`${API_URL}${path}`, init);
  if (!response.ok) {
    const body = (await response.json().catch(() => null)) as { error?: string } | null;
    throw new Error(body?.error ?? `HTTP ${response.status}`);
  }
  return response.json() as Promise<T>;
}

export function fetchHealth(): Promise<{ status: string; overpass_cache_dir: string; srtm_cache_dir: string }> {
  return apiJson('/health');
}

export function fetchCacheAreas(): Promise<CacheEntry[]> {
  return apiJson('/cache/areas');
}

export function prepareArea(body: {
  bbox: [number, number, number, number];
  filter: FeatureFilter;
  use_elevation: boolean;
  force_refresh: boolean;
}): Promise<PrepareAreaResponse> {
  return apiJson('/areas/prepare', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  });
}
```

- [ ] **Step 3: Add visual style**

Create `web/src/app/globals.css` with a dark cartographic command-console aesthetic: full viewport, map canvas, left glass panel, amber/green accent colors, clear focus states, no Minecraft terminology.

- [ ] **Step 4: Verify scaffold**

Run:

```bash
cd web
bun install
bun run build
```

Expected: build fails only because `page.tsx` does not exist yet, or passes if a temporary placeholder is added.

- [ ] **Step 5: Commit**

```bash
git add web
git commit -m "Scaffold osm-world web picker"
```

---

### Task 2: Add OpenLayers map picker UI

**Files:**
- Create: `web/src/components/MapPicker.tsx`
- Create/modify: `web/src/app/page.tsx`

- [ ] **Step 1: Implement map picker**

`MapPicker` must:

- render OpenStreetMap tiles,
- start near Sacramento by default,
- allow drawing/replacing one bbox with OpenLayers `Draw`,
- expose bbox as `[south, west, north, east]`,
- render cached area rectangles from `/cache/areas`,
- avoid SSR by being used through `next/dynamic`.

- [ ] **Step 2: Implement page controls**

`page.tsx` must:

- show API health/cache dirs,
- show selected bbox,
- include feature checkboxes: roads, buildings, water, landuse, railways,
- include `Use elevation` and `Force refresh` toggles,
- call `prepareArea()` and show status,
- show prepared `osm_path`, optional `srtm_dir`, cache status, and copyable command,
- avoid Minecraft/Bedrock wording.

- [ ] **Step 3: Verify frontend build**

Run:

```bash
cd web
bun run build
```

Expected: build succeeds.

- [ ] **Step 4: Commit**

```bash
git add web
git commit -m "Add web area picker UI"
```

---

### Task 3: Add Makefile helpers and full smoke verification

**Files:**
- Modify: `Makefile`

- [ ] **Step 1: Add Make targets**

Add:

```make
web-install:
	cd web && bun install

web-dev:
	cd web && bun run dev

web-build:
	cd web && bun run build

serve:
	cargo run -- --serve --host 127.0.0.1 --port 3030

dev:
	@cargo run -- --serve --host 127.0.0.1 --port 3030 &
	@cd web && bun run dev
```

Keep existing targets intact.

- [ ] **Step 2: Run full verification**

```bash
make checkall
make web-build
graphify update .
```

- [ ] **Step 3: Optional browser smoke**

Start backend and frontend, then manually confirm the page loads:

```bash
cargo run -- --serve --host 127.0.0.1 --port 3030
cd web && bun run dev
```

Open `http://localhost:8032`.

- [ ] **Step 4: Commit**

```bash
git add Makefile web graphify-out
git commit -m "Add web picker dev commands"
```

---

## Self-review checklist

- Spec coverage: first web picker can select bbox, inspect cache dirs, prepare area data, and copy command.
- Deferred requirements: direct browser-launched renderer and in-game streaming are not implemented in this phase.
- Verification: frontend build and Rust `make checkall` are required.
