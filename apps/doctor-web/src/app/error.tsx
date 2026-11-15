'use client';
import { useEffect } from 'react';
import { diagnostic } from '../lib/client';
import { ErrorScreen } from '../components/ErrorScreen';

/** Route-level 5xx screen. The error is reported through the sanitised diagnostics
 * channel only (category and digest), never rendered. */
export default function RouteError({ error, reset }: { error: Error & { digest?: string }; reset: () => void }) {
  useEffect(() => { diagnostic('render-error', error); }, [error]);
  return <ErrorScreen code="500" icon="alert" title="Something went wrong on our side"
    message="We could not load this screen. Nothing you entered has been shared. Please try again in a moment."
    actions={<><button className="am-btn primary lg" onClick={reset}>Try again</button><a className="am-btn ghost lg" href="/">Go to dashboard</a></>}/>;
}
