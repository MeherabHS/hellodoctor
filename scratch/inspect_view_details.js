const fs = require('fs');
const html = fs.readFileSync('index.html', 'utf8');

function getChunk(startStr, endStr) {
  const s = html.indexOf(startStr);
  if (s === -1) return '';
  const e = endStr ? html.indexOf(endStr, s) : s + 2000;
  return html.substring(s, e !== -1 ? e : s + 2000);
}

console.log('--- VIEWS BREAKDOWN ---');
for (let i = 0; i <= 14; i++) {
  const id = `id="view-${i}"`;
  const nextId = i < 14 ? `id="view-${i+1}"` : `id="doc-view-0"`;
  const chunk = getChunk(id, nextId);
  const comments = [...chunk.matchAll(/<!--\s*SCREEN\s*(\d+)?:?\s*(.*?)\s*-->/gi)].map(m => m[0]);
  const buttons = [...chunk.matchAll(/onclick="([^"]+)"/g)].map(m => m[1]).slice(0, 5);
  console.log(`view-${i}: Comments: ${comments.slice(0, 2).join(' | ')}`);
  console.log(`         Sample onClicks: ${buttons.join(', ')}`);
}

console.log('\n--- DOCTOR VIEWS BREAKDOWN ---');
for (let i = 0; i <= 5; i++) {
  const id = `id="doc-view-${i}"`;
  const nextId = i < 5 ? `id="doc-view-${i+1}"` : `id="doctorWebShell"`;
  const chunk = getChunk(id, nextId);
  const comments = [...chunk.matchAll(/<!--\s*(SCREEN|Doctor|DOC)\s*(\d+)?:?\s*(.*?)\s*-->/gi)].map(m => m[0]);
  const buttons = [...chunk.matchAll(/onclick="([^"]+)"/g)].map(m => m[1]).slice(0, 5);
  console.log(`doc-view-${i}: Comments: ${comments.slice(0, 2).join(' | ')}`);
  console.log(`             Sample onClicks: ${buttons.join(', ')}`);
}
