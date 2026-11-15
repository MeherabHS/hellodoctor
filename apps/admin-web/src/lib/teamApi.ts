import {request} from './api';

/** Admin Team (backend admin_team_handlers.rs). */
export interface TeamAdmin { id: string; email: string | null; phone: string; is_active: boolean; created_at: string; is_you: boolean; /** Email of the Admin whose invitation this account accepted; null for founding Admins. */ invited_by: string | null; invited_at: string | null }
export type InvitationStatus = 'PENDING' | 'ACCEPTED' | 'REVOKED' | 'EXPIRED';
export interface TeamInvitation { id: string; email: string; invited_by: string | null; created_at: string; expires_at: string; accepted_at: string | null; revoked_at: string | null; status: InvitationStatus }
export interface CreatedInvitation { invitation: {id: string; email: string; expires_at: string}; email_sent: boolean; /** Development only. */ dev_invite_url: string | null }

export const getTeam = () => request<{admins: TeamAdmin[]; invitations: TeamInvitation[]}>('/admin/team');
/** Inviting and resending need the signed-in Admin's current password (sent only in the POST body). */
export const inviteAdmin = (email: string, currentPassword: string) => request<CreatedInvitation>('/admin/team/invitations', {method: 'POST', body: JSON.stringify({email, current_password: currentPassword})});
export const resendInvitation = (id: string, currentPassword: string) => request<CreatedInvitation>(`/admin/team/invitations/${encodeURIComponent(id)}/resend`, {method: 'POST', body: JSON.stringify({current_password: currentPassword})});
export const revokeInvitation = (id: string) => request<unknown>(`/admin/team/invitations/${encodeURIComponent(id)}/revoke`, {method: 'POST', body: '{}'});

export {isPlainEmail} from './inviteEmail';
