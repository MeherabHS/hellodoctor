'use client';
import './fonts.css';
import './web.css';
import { ErrorScreen } from '../components/ErrorScreen';

/** Last-resort 5xx screen when the root layout itself fails. Renders its own document. */
export default function GlobalError({ reset }: { error: Error & { digest?: string }; reset: () => void }) {
  return <html lang="en"><body>
    <ErrorScreen code="500" icon="alert" title="The portal is temporarily unavailable"
      message="We are having trouble right now. Please wait a moment and try again. If this continues, contact AfyaMkononi support."
      actions={<button className="am-btn primary lg" onClick={reset}>Try again</button>}/>
  </body></html>;
}
