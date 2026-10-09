import type { Metadata } from 'next';
import './prototype.css';
import './fonts.css';
import './web.css';

export const metadata: Metadata = { title: 'HelloDoctor — Doctor Web', robots: { index: false, follow: false } };
export const dynamic = 'force-dynamic';
export default function RootLayout({ children }: { children: React.ReactNode }) {
  return <html lang="en"><body data-theme="sage">{children}</body></html>;
}
