import type { Profile } from './contracts';

export function isProfessionallyVerified(profile: Pick<Profile, 'professional_verification_status'> | null | undefined): boolean {
  return profile?.professional_verification_status === 'VERIFIED';
}

export function professionalVerificationLabel(profile: Pick<Profile, 'professional_verification_status'>): string {
  switch (profile.professional_verification_status) {
    case 'VERIFIED': return 'Professionally verified';
    case 'REJECTED': return 'Professional registration rejected';
    case 'PENDING': return 'Pending review';
    default: return 'Professional verification unavailable';
  }
}
