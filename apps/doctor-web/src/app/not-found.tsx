import { ErrorScreen } from '../components/ErrorScreen';

export default function NotFound() {
  return <ErrorScreen code="404" icon="search" title="We couldn't find that page"
    message="The page may have moved, or the link may be incomplete. Your consultations and patients are safe on your dashboard."
    actions={<a className="am-btn primary lg" href="/"><span>Go to dashboard</span></a>}/>;
}
