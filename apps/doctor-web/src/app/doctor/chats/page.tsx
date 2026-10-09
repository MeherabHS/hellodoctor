import { DoctorApplication } from '../../../components/DoctorApplication';
import { headers } from 'next/headers';
import { redirect } from 'next/navigation';
import { canonicalLoopbackRedirect } from '../../../lib/origin';

export default async function PatientChatsPage() {
  const configured=process.env.DOCTOR_WEB_ORIGIN;
  const requestHeaders=await headers();
  const canonical=canonicalLoopbackRedirect(new URL('/doctor/chats',configured||'http://localhost/').href,configured,requestHeaders.get('host'));
  if(canonical)redirect(canonical);
  return <DoctorApplication initialPage="chat"/>;
}
