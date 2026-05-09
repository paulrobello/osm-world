'use client';

import { errorHintForMessage } from '@/lib/errorHints';
import type { CommandVariant } from '@/lib/commandVariants';
import type { PrepareAreaResponse } from '@/lib/api';

function sourceStatusLabel(status: string): string {
  return status.replaceAll('_', ' ');
}

type CopyStatus = 'idle' | 'copied' | 'failed';
type LaunchStatus = 'idle' | 'launching' | 'launched' | 'failed';

interface PreparedOutputSectionProps {
  preparedArea: PrepareAreaResponse;
  commandVariants: CommandVariant[];
  copiedCommandVariant: string | null;
  copyStatus: CopyStatus;
  launchStatus: LaunchStatus;
  launchMessage: string | null;
  onCopyCommand: (variantId: string, command: string) => Promise<void>;
  onLaunchRenderer: () => Promise<void>;
}

export function PreparedOutputSection({
  preparedArea,
  commandVariants,
  copiedCommandVariant,
  copyStatus,
  launchStatus,
  launchMessage,
  onCopyCommand,
  onLaunchRenderer,
}: PreparedOutputSectionProps) {
  return (
    <section className="result-card" aria-labelledby="result-title">
      <div className="section-heading">
        <h2 id="result-title">Prepared output</h2>
        <span>{preparedArea.cache_status}</span>
      </div>
      <dl className="result-list">
        <div>
          <dt>OSM path</dt>
          <dd>{preparedArea.osm_path}</dd>
        </div>
        {preparedArea.srtm_dir ? (
          <div>
            <dt>SRTM dir</dt>
            <dd>{preparedArea.srtm_dir}</dd>
          </div>
        ) : null}
        <div>
          <dt>Cache key</dt>
          <dd>{preparedArea.cache_key}</dd>
        </div>
        <div>
          <dt>Source status</dt>
          <dd>{sourceStatusLabel(preparedArea.source_status)}</dd>
        </div>
        {preparedArea.spawn_lat !== null && preparedArea.spawn_lon !== null ? (
          <div>
            <dt>Spawn point</dt>
            <dd>{preparedArea.spawn_lat.toFixed(6)}, {preparedArea.spawn_lon.toFixed(6)}</dd>
          </div>
        ) : null}
      </dl>
      {preparedArea.warnings.length > 0 ? (
        <div className="warning-stack" role="status">
          {preparedArea.warnings.map((warning) => (
            <p className="status-line pending" key={warning}>{warning}</p>
          ))}
        </div>
      ) : null}
      <div className="command-variant-stack" aria-label="Launch command variants">
        {commandVariants.map((variant) => (
          <article className="command-variant" key={variant.id}>
            <label className="command-box">
              <span>{variant.label} command</span>
              <textarea readOnly value={variant.command} rows={3} />
            </label>
            <p className="microcopy">{variant.description}</p>
            <button className="ghost-button copy-button" type="button" onClick={() => void onCopyCommand(variant.id, variant.command)}>
              {copiedCommandVariant === variant.id ? `Copied ${variant.label}` : `Copy ${variant.label}`}
            </button>
          </article>
        ))}
      </div>
      <div className="button-row result-actions">
        <button className="ghost-button copy-button" type="button" onClick={() => void onLaunchRenderer()} disabled={launchStatus === 'launching'}>
          {launchStatus === 'launching' ? 'Launching…' : 'Launch renderer'}
        </button>
      </div>
      {copyStatus === 'failed' ? <p className="status-line error">Clipboard permission denied. Select the command manually.</p> : null}
      {launchMessage ? (
        <div className="status-stack">
          <p className={`status-line ${launchStatus === 'failed' ? 'error' : 'success'}`}>{launchMessage}</p>
          {launchStatus === 'failed' && errorHintForMessage(launchMessage) ? (
            <p className="status-line hint">{errorHintForMessage(launchMessage)}</p>
          ) : null}
        </div>
      ) : null}
    </section>
  );
}
