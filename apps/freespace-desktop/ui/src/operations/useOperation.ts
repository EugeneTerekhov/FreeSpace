import { useCallback, useEffect, useRef, useState } from 'react';

import { api } from '../api/runtime';
import type {
  DeleteMode,
  OperationAccepted,
  OperationCompletedEvent,
  OperationPreview,
  OperationPreviewRequest,
  OperationProgress,
  UiError,
} from '../api/dto';
import { normalizeError } from '../app/errors';

export interface UseOperationState {
  preview: OperationPreview | null;
  progress: OperationProgress | null;
  accepted: OperationAccepted | null;
  busy: boolean;
  error: UiError | null;
  prepare: (request: OperationPreviewRequest) => Promise<void>;
  confirm: (mode: DeleteMode, extraConfirmation: boolean) => Promise<void>;
  cancel: () => Promise<void>;
  undo: () => Promise<void>;
  close: () => void;
}

const TERMINAL = new Set(['completed', 'partial', 'failed', 'cancelled']);

/** Opens the shared preview, confirms it and follows the operation to the end. */
export function useOperation(): UseOperationState {
  const [preview, setPreview] = useState<OperationPreview | null>(null);
  const [progress, setProgress] = useState<OperationProgress | null>(null);
  const [accepted, setAccepted] = useState<OperationAccepted | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<UiError | null>(null);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(
    () => () => {
      if (timer.current !== null) clearTimeout(timer.current);
    },
    [],
  );

  // The poll is the primary path, but the backend also announces the terminal
  // state. Reading it on the event closes the race where a very fast operation
  // finishes between two polls; the tree then removes the deleted row at once.
  useEffect(() => {
    if (accepted === null) return undefined;
    let active = true;
    let unlisten: (() => void) | undefined;
    const operationId = accepted.operation_id;

    void api.transport
      .listen<OperationCompletedEvent>('operation-completed', (envelope) => {
        if (!active || envelope.payload.operation_id !== operationId) return;
        void api
          .operationStatus(operationId)
          .then((current) => {
            if (active) setProgress(current);
          })
          .catch(() => undefined);
      })
      .then((off) => {
        if (active) unlisten = off;
        else off();
      });

    return () => {
      active = false;
      unlisten?.();
    };
  }, [accepted]);

  const prepare = useCallback(async (request: OperationPreviewRequest) => {
    setError(null);
    setBusy(true);
    setProgress(null);
    setAccepted(null);
    try {
      setPreview(await api.operationPreview(request));
    } catch (cause) {
      setError(normalizeError(cause));
      setPreview(null);
    } finally {
      setBusy(false);
    }
  }, []);

  const poll = useCallback((operationId: string) => {
    const tick = async () => {
      try {
        const current = await api.operationStatus(operationId);
        setProgress(current);
        if (!TERMINAL.has(current.state)) {
          timer.current = setTimeout(() => void tick(), 300);
        }
      } catch (cause) {
        setError(normalizeError(cause));
      }
    };
    void tick();
  }, []);

  const confirm = useCallback(
    async (mode: DeleteMode, extraConfirmation: boolean) => {
      if (preview === null) return;
      setBusy(true);
      setError(null);
      try {
        const acceptedOperation = await api.operationConfirm(
          preview.preview_id,
          preview.confirmation_token,
          mode,
          extraConfirmation,
        );
        setAccepted(acceptedOperation);
        setPreview(null);
        poll(acceptedOperation.operation_id);
      } catch (cause) {
        setError(normalizeError(cause));
      } finally {
        setBusy(false);
      }
    },
    [poll, preview],
  );

  const cancel = useCallback(async () => {
    if (accepted === null) return;
    try {
      await api.operationCancel(accepted.operation_id);
    } catch (cause) {
      setError(normalizeError(cause));
    }
  }, [accepted]);

  const undo = useCallback(async () => {
    if (accepted === null) return;
    try {
      await api.operationUndo(accepted.operation_id);
      setProgress((current) =>
        current === null ? current : { ...current, undo_available: false },
      );
    } catch (cause) {
      setError(normalizeError(cause));
    }
  }, [accepted]);

  const close = useCallback(() => {
    if (timer.current !== null) clearTimeout(timer.current);
    setPreview(null);
    setProgress(null);
    setAccepted(null);
    setError(null);
  }, []);

  return { preview, progress, accepted, busy, error, prepare, confirm, cancel, undo, close };
}
