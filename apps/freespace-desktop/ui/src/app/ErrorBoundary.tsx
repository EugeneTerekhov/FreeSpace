import { Component, type ErrorInfo, type ReactNode } from 'react';

import { Button } from '../components/Button';
import { ErrorState } from '../components/Feedback';
import { logLocal, normalizeError } from './errors';

interface ErrorBoundaryProps {
  children: ReactNode;
}

interface ErrorBoundaryState {
  details: string | null;
  code: string | null;
}

/** Catches render failures, logs them locally and offers a reload. */
export class ErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
  override state: ErrorBoundaryState = { details: null, code: null };

  static getDerivedStateFromError(error: unknown): ErrorBoundaryState {
    const normalized = normalizeError(error);

    return {
      code: normalized.code,
      details: normalized.details,
    };
  }

  override componentDidCatch(error: unknown, info: ErrorInfo): void {
    logLocal('error', 'interface crashed', {
      error: normalizeError(error).details,
      componentStack: info.componentStack,
    });
  }

  override render(): ReactNode {
    if (this.state.code !== null) {
      return (
        <ErrorState
          title="Something went wrong"
          code={this.state.code}
          description="The screen could not be rendered. Your data was not changed."
          details={this.state.details ?? undefined}
          actions={
            <Button
              variant="primary"
              onClick={() => {
                window.location.reload();
              }}
            >
              Reload
            </Button>
          }
        />
      );
    }

    return this.props.children;
  }
}
