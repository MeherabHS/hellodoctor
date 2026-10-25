import type { Metadata } from 'next';
import './globals.css';
import { ErrorBoundary } from '@/components/ErrorBoundary';
import { ConsoleGuard } from '@/components/ConsoleGuard';
// Every HTML response must use the nonce set by middleware, including login.
export const dynamic = 'force-dynamic';

export const metadata: Metadata = {
  title: 'AfyaMkononi — Central Hospital & Operations Command Portal',
  description: 'Enterprise administration, grievance adjudication, and monthly finance disbursements.',
};

export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html lang="en">
      <body className="bg-slate-50 text-slate-900 min-h-screen antialiased flex flex-col">
        <ConsoleGuard />
        <ErrorBoundary>{children}</ErrorBoundary>
      </body>
    </html>
  );
}
