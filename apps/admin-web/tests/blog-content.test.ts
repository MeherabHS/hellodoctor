import test from 'node:test';
import assert from 'node:assert/strict';
import {articleErrors,blankArticle,articleLanguage,safeArticleLink,articleActions,nairobiSchedule} from '../src/lib/blogContent.ts';
const valid=()=>({...blankArticle(),title_en:'Title',body_en:'## Body',category_id:'category'});
test('article validation accepts English fallback and paired bilingual content',()=>{
  assert.deepEqual(articleErrors(valid()),[]);
  assert.deepEqual(articleErrors({...valid(),title_sw:'Kichwa',body_sw:'Maelezo'}),[]);
  assert.ok(articleErrors({...valid(),title_sw:'Kichwa'}).length);
});
test('article validation bounds required content, tags, category and nulls',()=>{
  for(const change of [{title_en:''},{body_en:''},{title_en:'x'.repeat(161)},{body_en:'x'.repeat(30001)},{category_id:''},{tags:Array(11).fill('tag')},{tags:['x'.repeat(41)]},{body_en:'bad\0body'}])assert.ok(articleErrors({...valid(),...change}).length);
});
test('preview language selects whole translation and English fallback consistently',()=>{
  const en=valid();assert.equal(articleLanguage(en,'sw').title,'Title');assert.equal(articleLanguage(en,'sw').fallback,true);
  const bilingual={...en,title_sw:'Kichwa',body_sw:'Maelezo'};assert.deepEqual(articleLanguage(bilingual,'sw'),{title:'Kichwa',body:'Maelezo',fallback:false});
});
test('article links allow explicit HTTP(S) and reject active, local and credential URLs',()=>{
  assert.equal(safeArticleLink('https://example.org/path'),'https://example.org/path');
  for(const url of ['javascript:alert(1)','JaVaScRiPt:alert(1)','data:text/html,hi','file:///etc/passwd','//example.org','https://user:password@example.org','https://example.org\n','https:\\evil.example','mailto:a@example.org'])assert.equal(safeArticleLink(url),'');
});
test('status actions require review before publication and archive is terminal',()=>{
  assert.ok(!articleActions('DRAFT').includes('publish'));assert.ok(articleActions('PENDING_REVIEW').includes('return'));assert.ok(articleActions('APPROVED').includes('publish'));assert.ok(!articleActions('SCHEDULED').includes('approve'));assert.deepEqual(articleActions('ARCHIVED'),[]);
});
test('Nairobi schedule produces UTC independent of browser timezone',()=>{
  assert.equal(nairobiSchedule('2099-01-01T12:30'),'2099-01-01T09:30:00.000Z');assert.throws(()=>nairobiSchedule('bad'));assert.throws(()=>nairobiSchedule('2020-01-01T12:30'));
});
