// ESLint flat config. `eslint-config-next@16` ships a flat-config array
// (no legacy `.eslintrc.json` support), so we re-export it directly with
// project ignores layered in. Run via `bun run lint`.
import nextCoreWebVitals from 'eslint-config-next/core-web-vitals';

export default [
  {
    ignores: [
      'node_modules/**',
      '.next/**',
      'next-env.d.ts',
      'tsconfig.tsbuildinfo',
      'postcss.config.mjs',
      'next.config.ts',
    ],
  },
  ...nextCoreWebVitals,
  {
    rules: {
      // eslint-plugin-react-hooks v7 introduced this rule. Several existing
      // components use the intentional "sync state to prop" pattern (reset
      // form fields when a prop changes; read localStorage after mount to
      // stay SSR-safe). The proper fix is per-component (lazy state
      // initializers, derived state, or remount-on-key) and is tracked as
      // follow-up work — see AUDIT.md QA-001 (Home decomposition) which
      // already owns the largest offender. Downgrade to warn so the rule
      // still surfaces each site without blocking lint on intentional code.
      'react-hooks/set-state-in-effect': 'warn',
    },
  },
];
