'use client';
// Mirrors apps/doctor-web/src/components/DoctorApplication.tsx's `Boundary`
// class + its window-level error/unhandledrejection hooks — the one
// client-crash-reporting pattern already established in this codebase,
// applied here since admin-web was the one app missing it.
import React, { useEffect } from 'react';
import { diagnostic } from '@/lib/diagnostics';

class Boundary extends React.Component<{ children: React.ReactNode }, { failed: boolean }> {
  state = { failed: false };
  static getDerivedStateFromError() { return { failed: true }; }
  componentDidCatch(error: Error) { diagnostic('render-error', error); }
  render() {
    if (!this.state.failed) return this.props.children;
    return (
      <main style={{ minHeight: '100vh', display: 'flex', alignItems: 'center', justifyContent: 'center', background: '#F8FAFC' }}>
        <div style={{ textAlign: 'center', padding: '32px', background: '#FFFFFF', border: '1px solid #E2E8F0', borderRadius: '12px', maxWidth: '360px' }}>
          <p style={{ color: '#334155', fontSize: '14px', marginBottom: '16px' }}>This screen could not be rendered. Reload to restore your session.</p>
          <button onClick={() => location.reload()} style={{ padding: '8px 20px', fontSize: '12px', background: '#0F172A', color: '#fff', borderRadius: '8px', border: 'none', cursor: 'pointer' }}>
            Reload
          </button>
        </div>
      </main>
    );
  }
}

export function ErrorBoundary({ children }: { children: React.ReactNode }) {
  useEffect(() => {
    const browserError = (event: ErrorEvent) => diagnostic('browser-error', event.error);
    const rejection = (event: PromiseRejectionEvent) => diagnostic('promise-rejection', event.reason);
    window.addEventListener('error', browserError);
    window.addEventListener('unhandledrejection', rejection);
    return () => {
      window.removeEventListener('error', browserError);
      window.removeEventListener('unhandledrejection', rejection);
    };
  }, []);
  return <Boundary>{children}</Boundary>;
}
