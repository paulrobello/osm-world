'use client';

import type { PreparedAreaEntry } from '@/lib/api';

function sourceStatusLabel(status: string): string {
  return status.replaceAll('_', ' ');
}

function formatBbox(bbox: number[] | null): string {
  if (!bbox) {
    return 'No bbox selected';
  }

  return bbox.map((value) => value.toFixed(5)).join(', ');
}

interface PreparedHistorySectionProps {
  entries: PreparedAreaEntry[];
  disabled: boolean;
  onLoadEntry: (entry: PreparedAreaEntry) => void;
  onToggleFavorite: (entry: PreparedAreaEntry) => void;
  onRename: (entry: PreparedAreaEntry) => void;
  onDelete: (entry: PreparedAreaEntry) => void;
}

export function PreparedHistorySection({
  entries,
  disabled,
  onLoadEntry,
  onToggleFavorite,
  onRename,
  onDelete,
}: PreparedHistorySectionProps) {
  return (
    <section className="control-group" aria-labelledby="history-title">
      <div className="section-heading">
        <h2 id="history-title">Prepared history</h2>
        <span>{entries.length} cached</span>
      </div>
      {entries.length === 0 ? (
        <p className="microcopy">Prepared areas will appear here after the first successful prepare request.</p>
      ) : (
        <div className="history-list">
          {entries.map((entry) => (
            <article className="history-entry" key={entry.cache_key}>
              <button className="history-main" type="button" onClick={() => onLoadEntry(entry)} disabled={disabled}>
                <strong>{entry.display_name || entry.cache_key.slice(0, 10)}</strong>
                <span>{sourceStatusLabel(entry.source_status)} · {formatBbox(entry.bbox)}</span>
              </button>
              <div className="history-actions">
                <button className="mini-button" type="button" onClick={() => void onToggleFavorite(entry)}>
                  {entry.favorite ? '★' : '☆'}
                </button>
                <button className="mini-button" type="button" onClick={() => onRename(entry)}>
                  Name
                </button>
                <button className="mini-button danger-mini-button" type="button" onClick={() => onDelete(entry)}>
                  Delete
                </button>
              </div>
            </article>
          ))}
        </div>
      )}
    </section>
  );
}
