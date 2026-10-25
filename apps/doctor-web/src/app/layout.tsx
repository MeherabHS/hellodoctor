import type { Metadata } from 'next';
import './prototype.css';
import './fonts.css';
import './web.css';
import { ConsoleGuard } from '../components/ConsoleGuard';

export const metadata: Metadata = { title: 'AfyaMkononi — Doctor Web', robots: { index: false, follow: false } };
export const dynamic = 'force-dynamic';
export default function RootLayout({ children }: { children: React.ReactNode }) {
  return <html lang="en"><body data-theme="sage"><ConsoleGuard />{children}</body></html>;
}
