'use client';

import { useCallback, useEffect, useRef, useState } from 'react';

/**
 * A lightweight modal dialog that replaces window.prompt and window.confirm.
 *
 * Usage:
 *   const [open, setOpen] = useState(false);
 *   <PromptDialog open={open} title="Name" defaultValue="" onConfirm={handleConfirm} onCancel={() => setOpen(false)} />
 */
export function PromptDialog({
  open,
  title,
  label,
  defaultValue = '',
  confirmLabel = 'OK',
  cancelLabel = 'Cancel',
  onConfirm,
  onCancel,
}: {
  open: boolean;
  title: string;
  label?: string;
  defaultValue?: string;
  confirmLabel?: string;
  cancelLabel?: string;
  onConfirm: (value: string) => void;
  onCancel: () => void;
}) {
  const [value, setValue] = useState(defaultValue);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (open) {
      setValue(defaultValue);
      // Focus the input after the dialog renders
      requestAnimationFrame(() => inputRef.current?.focus());
    }
  }, [open, defaultValue]);

  const handleKeyDown = useCallback(
    (event: React.KeyboardEvent) => {
      if (event.key === 'Enter') {
        event.preventDefault();
        onConfirm(value);
      } else if (event.key === 'Escape') {
        event.preventDefault();
        onCancel();
      }
    },
    [onConfirm, onCancel, value],
  );

  if (!open) {
    return null;
  }

  return (
    <div className="dialog-overlay" role="dialog" aria-modal="true" aria-labelledby="dialog-title" onKeyDown={handleKeyDown}>
      <div className="dialog-card">
        <h3 id="dialog-title">{title}</h3>
        {label ? <label className="dialog-label">{label}</label> : null}
        <input
          ref={inputRef}
          className="dialog-input"
          type="text"
          value={value}
          onChange={(event) => setValue(event.target.value)}
        />
        <div className="dialog-actions">
          <button className="ghost-button" type="button" onClick={onCancel}>
            {cancelLabel}
          </button>
          <button className="primary-action" type="button" onClick={() => onConfirm(value)}>
            {confirmLabel}
          </button>
        </div>
      </div>
    </div>
  );
}

/**
 * A confirmation dialog that replaces window.confirm.
 */
export function ConfirmDialog({
  open,
  title,
  message,
  confirmLabel = 'Delete',
  cancelLabel = 'Cancel',
  onConfirm,
  onCancel,
  dangerous = false,
}: {
  open: boolean;
  title: string;
  message: string;
  confirmLabel?: string;
  cancelLabel?: string;
  onConfirm: () => void;
  onCancel: () => void;
  dangerous?: boolean;
}) {
  const cancelRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    if (open) {
      requestAnimationFrame(() => cancelRef.current?.focus());
    }
  }, [open]);

  const handleKeyDown = useCallback(
    (event: React.KeyboardEvent) => {
      if (event.key === 'Escape') {
        event.preventDefault();
        onCancel();
      }
    },
    [onCancel],
  );

  if (!open) {
    return null;
  }

  return (
    <div className="dialog-overlay" role="dialog" aria-modal="true" aria-labelledby="dialog-title" onKeyDown={handleKeyDown}>
      <div className="dialog-card">
        <h3 id="dialog-title">{title}</h3>
        <p className="dialog-message">{message}</p>
        <div className="dialog-actions">
          <button className="ghost-button" type="button" ref={cancelRef} onClick={onCancel}>
            {cancelLabel}
          </button>
          <button className={dangerous ? 'ghost-button danger-button' : 'primary-action'} type="button" onClick={onConfirm}>
            {confirmLabel}
          </button>
        </div>
      </div>
    </div>
  );
}
