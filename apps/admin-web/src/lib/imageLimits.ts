/** Admin-posted images (article covers, facility photos). The Admin may
 *  upload a photo of practically any size; the backend compresses it to at
 *  most 1 MB when it is saved. 25 MB only bounds server decoding memory and
 *  matches the backend's MAX_ADMIN_IMAGE_UPLOAD. */
export const MAX_ADMIN_IMAGE_UPLOAD = 25 * 1024 * 1024;
