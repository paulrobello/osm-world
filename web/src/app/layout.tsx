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
