import {request} from './api';
import {parseApiError} from './i18n/apiErrors';

/** The signed-in Admin's own profile (backend admin_profile_handlers.rs). */
export interface AdminMe { email: string | null; /** Last three digits only. */ phone: string; created_at: string; has_photo: boolean; photo_updated_at: string | null }

export const getMe = () => request<AdminMe>('/admin/me');

/** The photo as a data: URL, or null when none is set. */
export async function getMyPhoto(): Promise<string | null> {
  try {
    const photo = await request<{mime_type: string; base64_content: string}>('/admin/me/photo');
    return `data:${photo.mime_type};base64,${photo.base64_content}`;
  } catch (e) {
    if (parseApiError(e).status === 404) return null;
    throw e;
  }
}

export const uploadMyPhoto = (photo: {mime_type: string; base64_content: string}) =>
  request<unknown>('/admin/me/photo', {method: 'POST', body: JSON.stringify(photo)});
export const removeMyPhoto = () => request<unknown>('/admin/me/photo', {method: 'DELETE'});

/** Shared endpoint; revokes every other session of this account, keeps this one. */
export const changePassword = (currentPassword: string, newPassword: string) =>
  request<unknown>('/auth/change-password', {method: 'POST', body: JSON.stringify({current_password: currentPassword, new_password: newPassword})});

/** Resize to at most 512 px and re-encode as JPEG in the browser (canvas only, nothing stored). */
export async function preparePhoto(file: File, messages: {type: string; tooLarge: string}): Promise<{mime_type: string; base64_content: string}> {
  if (!['image/jpeg', 'image/png', 'image/webp'].includes(file.type)) throw new Error(messages.type);
  if (file.size > 10 * 1024 * 1024) throw new Error(messages.tooLarge);
  const bitmap = await createImageBitmap(file);
  const scale = Math.min(1, 512 / Math.max(bitmap.width, bitmap.height));
  const canvas = document.createElement('canvas');
  canvas.width = Math.round(bitmap.width * scale);
  canvas.height = Math.round(bitmap.height * scale);
  canvas.getContext('2d')!.drawImage(bitmap, 0, 0, canvas.width, canvas.height);
  bitmap.close();
  return {mime_type: 'image/jpeg', base64_content: canvas.toDataURL('image/jpeg', 0.86).split(',')[1]};
}
