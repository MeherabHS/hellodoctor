import type { Slot } from './contracts';

// Counts need the complete authorized listing, not just 100 historical slots.
export async function loadDoctorSlots(load: (path: string) => Promise<Slot[]>, isCurrent: () => boolean): Promise<Slot[]> {
  const rows: Slot[] = [];
  for (let offset = 0; offset <= 9900; offset += 100) {
    const next = await load(`doctor/slots?limit=100&offset=${offset}`);
    if (!isCurrent()) throw new DOMException('Session changed', 'AbortError');
    rows.push(...next);
    if (next.length < 100) return rows;
  }
  throw new Error('Schedule exceeds display capacity.');
}
