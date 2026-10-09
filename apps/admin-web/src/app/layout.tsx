import type { Metadata } from 'next';
import './globals.css';
// Every HTML response must use the nonce set by middleware, including login.
export const dynamic = 'force-dynamic';

export const metadata: Metadata = {
  title: 'HelloDoctor — Central Hospital & Operations Command Portal',
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
        {children}
      </body>
    </html>
  );
}
