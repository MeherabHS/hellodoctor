const fs = require('fs');

const origHtml = fs.readFileSync('prototype/index.html', 'utf8');
const newAdminChunk = fs.readFileSync('scratch/generated_admin_shell.html', 'utf8');

const startMarker = '<div id="adminPortalShell"';
const endMarker = '</div><!-- /#adminPortalShell -->';

const startIdx = origHtml.indexOf(startMarker);
const endIdx = origHtml.indexOf(endMarker);

if (startIdx === -1 || endIdx === -1) {
  console.error('Could not find markers!');
  process.exit(1);
}

const fullEndIdx = endIdx + endMarker.length;
console.log(`Replacing from index ${startIdx} to ${fullEndIdx} (${fullEndIdx - startIdx} chars) with new chunk (${newAdminChunk.length} chars)`);

const updatedHtml = origHtml.substring(0, startIdx) + newAdminChunk + origHtml.substring(fullEndIdx);
fs.writeFileSync('prototype/index.html', updatedHtml, 'utf8');
console.log('Successfully updated prototype/index.html!');
