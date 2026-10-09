import { DoctorApplication } from '../components/DoctorApplication';
import { headers } from 'next/headers';
import { redirect } from 'next/navigation';
import { canonicalLoopbackRedirect } from '../lib/origin';

export default async function Page() {
  const configured = process.env.DOCTOR_WEB_ORIGIN;
  const requestHeaders = await headers();
  // Page redirects preserve the absolute origin; Next middleware rewrites
  // loopback redirects against its internal server hostname.
  const canonical = canonicalLoopbackRedirect(configured || 'http://localhost/', configured, requestHeaders.get('host'));
  if (canonical) redirect(canonical);
  return <DoctorApplication />;
}
