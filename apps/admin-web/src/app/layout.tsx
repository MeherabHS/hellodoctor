import type { Metadata } from 'next';
import './globals.css';
import { ErrorBoundary } from '@/components/ErrorBoundary';
import { ConsoleGuard } from '@/components/ConsoleGuard';
import { cookies } from 'next/headers';
import { I18nProvider } from '@/lib/i18n';
import { tr } from '@/lib/i18n/core';
import { LOCALE_COOKIE, normalizeLocale } from '@/lib/i18n/locale';
// Every HTML response must use the nonce set by middleware, including login.
export const dynamic = 'force-dynamic';

export async function generateMetadata(): Promise<Metadata> {
  const locale = normalizeLocale((await cookies()).get(LOCALE_COOKIE)?.value);
  return {
    title: tr('ui.afyamkononiCentralHospitalOperationsComm', undefined, locale),
    description: tr('ui.enterpriseAdministrationGrievanceAdjudic', undefined, locale),
    // Browser-tab icon: the AfyaMkononi logo (/brand is public, so it shows on login too).
    icons: { icon: '/brand/afya_logo.png', apple: '/brand/afya_logo.png' },
  };
}

export default async function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  // English or Swahili, remembered per browser (EN | SW switch).
  const locale = normalizeLocale((await cookies()).get(LOCALE_COOKIE)?.value);
  return (
    <html lang={locale}>
      <body className="bg-slate-50 text-slate-900 min-h-screen antialiased flex flex-col">
        <ConsoleGuard />
        <I18nProvider initialLocale={locale}>
          <ErrorBoundary>{children}</ErrorBoundary>
        </I18nProvider>
      </body>
    </html>
  );
}
