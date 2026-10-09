import fs from 'node:fs';
import path from 'node:path';
function files(root) { return fs.readdirSync(root,{withFileTypes:true}).flatMap(entry=>entry.isDirectory()?files(path.join(root,entry.name)):[path.join(root,entry.name)]); }
const failures=[];
for(const file of files('src').filter(file=>/\.(ts|tsx|css|json)$/.test(file))){
  const source=fs.readFileSync(file,'utf8');
  if(!source.endsWith('\n')&&!file.endsWith('templates.json'))failures.push(`${file}: missing final newline`);
  if(/\t|[ \t]+$/m.test(source)&&!file.endsWith('templates.json'))failures.push(`${file}: whitespace`);
  if(/dangerouslySetInnerHTML|\beval\s*\(|localStorage|sessionStorage|document\.cookie/.test(source))failures.push(`${file}: unsafe rendering/storage`);
  if(/bmdc|bangladesh|\+880/i.test(source))failures.push(`${file}: legacy professional-registration attribute or copy`);
  if(/\bBDT\b|৳|bKash|Nagad|Bangladesh|\bBMDC\b/.test(source))failures.push(`${file}: active Kenya conflict`);
}
if(failures.length){console.error(failures.join('\n'));process.exit(1);}
console.log('Source safety and project formatting checks passed.');
