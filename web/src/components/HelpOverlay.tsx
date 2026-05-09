'use client';

import { useEffect, useState } from 'react';

const HELP_OVERLAY_STORAGE_KEY = 'osm-world-web-help-seen';

export function HelpOverlay() {
  const [visible, setVisible] = useState(false);

  useEffect(() => {
    if (window.localStorage.getItem(HELP_OVERLAY_STORAGE_KEY) !== 'true') {
      setVisible(true);
    }
  }, []);

  if (!visible) {
    return null;
  }

  const dismiss = () => {
    window.localStorage.setItem(HELP_OVERLAY_STORAGE_KEY, 'true');
    setVisible(false);
  };

  return (
    <section className="help-overlay" role="dialog" aria-modal="true" aria-labelledby="help-title">
      <div className="help-card">
        <p className="eyebrow">first run checklist</p>
        <h2 id="help-title">Fly the city without guessing</h2>
        <ul>
          <li><strong>Flycam:</strong> WASD moves, mouse looks, Shift accelerates, Space/Ctrl changes altitude.</li>
          <li><strong>Minimap:</strong> press M in the renderer to toggle it; rotation follows the camera heading.</li>
          <li><strong>Settings:</strong> F1 opens lighting, labels, shadows, minimap, and performance controls.</li>
          <li><strong>Screenshots:</strong> use the screenshot command variant for repeatable PNG captures.</li>
          <li><strong>Labels:</strong> POI, address, and street-sign labels can be tuned from renderer settings.</li>
        </ul>
        <button className="primary-action" type="button" onClick={dismiss}>
          Got it
        </button>
      </div>
    </section>
  );
}
