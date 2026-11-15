import type { Metadata } from 'next';
import './fonts.css';
import './web.css';
import { ConsoleGuard } from '../components/ConsoleGuard';

export const metadata: Metadata = { title: 'AfyaMkononi Doctor Portal', robots: { index: false, follow: false } };
export const dynamic = 'force-dynamic';
export default function RootLayout({ children }: { children: React.ReactNode }) {
  return <html lang="en"><body><ConsoleGuard />{children}</body></html>;
}
