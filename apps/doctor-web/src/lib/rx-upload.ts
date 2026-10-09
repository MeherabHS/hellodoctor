import type { RecordDto, RxPage } from './contracts';

// Upload orchestration only; persistence, page cap, ownership and completion stay server-side.
export async function uploadPrescriptionPages(pages: RxPage[], notes: string, only: string | undefined,
  upload: (page: RxPage, progress: (fraction: number | null) => void) => Promise<RecordDto>, update: (pages: RxPage[]) => void,
  isCurrent: () => boolean, failure: (error: unknown) => string) {
  const current = [...pages];
  for (let index = 0; index < current.length; index++) {
    if (!isCurrent()) throw new DOMException('Session changed', 'AbortError');
    const page = current[index];
    if (page.state === 'UPLOADED' || (only && page.key !== only)) continue;
    current[index] = { ...page, state: 'UPLOADING', error: undefined, uploadProgress: null, uploadNotes: page.uploadNotes ?? notes };
    update([...current]);
    try {
      const result = await upload(current[index], fraction => {
        if (!isCurrent() || current[index].state !== 'UPLOADING') return;
        current[index] = { ...current[index], uploadProgress: fraction === null || !Number.isFinite(fraction) ? null : Math.min(1, Math.max(0, fraction)) };
        update([...current]);
      });
      if (!isCurrent()) throw new DOMException('Session changed', 'AbortError');
      current[index] = { ...current[index], state: 'UPLOADED', uploadProgress: 1, documentId: String(result.id) };
    } catch (error) {
      if (!isCurrent() || (error instanceof DOMException && error.name === 'AbortError')) throw error;
      current[index] = { ...current[index], state: 'FAILED', error: failure(error) };
    }
    update([...current]);
  }
  return current;
}
