/**
 * Root Next.js layout for the osm-world Web Explorer.
 *
 * Sets the document language, page metadata, and renders the global stylesheet
 * so every route shares the same HTML shell. Page-specific UI is rendered into
 * `<body>` by Next.js via `children`.
 */
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
