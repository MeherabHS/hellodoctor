// Match the current server's single-page budget and resizing policy. The server
// still validates/sanitizes the image; browser compression only reduces transfer.
export const RX_PAGE_MAX_BYTES = 40 * 1024;
export const RX_SOURCE_MAX_BYTES = 10 * 1024 * 1024;
const qualities = [0.8, 0.65, 0.5, 0.35, 0.2, 0.05, 0.01];

export async function encodeWithinRxBudget(width: number, height: number, mime: string,
  encode: (width: number, height: number, quality: number) => Promise<Blob>,
  isCurrent: () => boolean = () => true): Promise<Blob> {
  if (!Number.isInteger(width) || !Number.isInteger(height) || width < 1 || height < 1 ||
    width > 8192 || height > 8192 || width * height > 32 * 1024 * 1024) {
    throw new Error('This photo is too large to process. Choose a smaller image.');
  }
  let edge = Math.min(2000, Math.max(width, height));
  for (;;) {
    const scale = edge / Math.max(width, height);
    for (const quality of mime === 'image/png' ? [1] : qualities) {
      if (!isCurrent()) throw new DOMException('Session or consultation changed', 'AbortError');
      const blob = await encode(Math.max(1, Math.round(width * scale)), Math.max(1, Math.round(height * scale)), quality);
      if (!isCurrent()) throw new DOMException('Session or consultation changed', 'AbortError');
      if (blob.size > 0 && blob.size <= RX_PAGE_MAX_BYTES && blob.type === mime) return blob;
      if (!blob.size || blob.type !== mime) throw new Error('Unable to compress this photo. Choose another JPEG or PNG.');
    }
    if (edge <= 8) throw new Error('Unable to fit this photo within 40 KB. Choose another photo.');
    edge = Math.max(8, Math.floor(edge / 2));
  }
}

export async function compressRxPhoto(file: File, isCurrent: () => boolean): Promise<File> {
  if (!['image/jpeg', 'image/png', 'image/webp'].includes(file.type) || !file.size || file.size > RX_SOURCE_MAX_BYTES) {
    throw new Error('Choose a JPEG, PNG or WebP photo, up to 10 MB before compression.');
  }
  let image: ImageBitmap;
  try { image = await createImageBitmap(file, { imageOrientation: 'from-image' }); }
  catch { throw new Error('This photo could not be opened. Choose a valid JPEG, PNG or WebP image.'); }
  const canvas = document.createElement('canvas');
  try {
    const mime = file.type === 'image/png' ? 'image/png' : 'image/jpeg';
    const blob = await encodeWithinRxBudget(image.width, image.height, mime, async (width, height, quality) => {
      canvas.width = width; canvas.height = height;
      const context = canvas.getContext('2d');
      if (!context) throw new Error('Photo compression is unavailable in this browser.');
      if (mime === 'image/jpeg') { context.fillStyle = '#ffffff'; context.fillRect(0, 0, width, height); }
      context.drawImage(image, 0, 0, width, height);
      return new Promise<Blob>((resolve, reject) => canvas.toBlob(result => result ? resolve(result) : reject(new Error('Unable to compress this photo.')), mime, quality));
    }, isCurrent);
    // Canvas re-encoding strips metadata. WebP is converted to the JPEG format
    // accepted by the current Doctor prescription endpoint; PNG remains PNG.
    return new File([blob], file.name.replace(/\.[^.]*$/, '') + (mime === 'image/png' ? '.png' : '.jpg'), { type: mime });
  } finally { image.close(); canvas.width = 0; canvas.height = 0; }
}
