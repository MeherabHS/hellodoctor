import test from 'node:test';
import assert from 'node:assert/strict';
import { articleActions, articleValidation } from '../src/lib/articles.ts';
import { safeBackendPath } from '../src/lib/contracts.ts';
test('articles support direct publication and archived content is read-only',()=>{
  assert.deepEqual(articleActions('DRAFT'),{editable:true,publishable:true,archivable:true});
  assert.deepEqual(articleActions('PUBLISHED'),{editable:true,publishable:false,archivable:true});
  assert.deepEqual(articleActions('ARCHIVED'),{editable:false,publishable:false,archivable:false});
});
test('structured article fields and reference bounds are validated',()=>{
  const content={title:'Title',summary:'Summary',body:'Educational article',references_list:[]};
  assert.equal(articleValidation(content),undefined);
  assert.ok(articleValidation({...content,title:' '}));
  assert.ok(articleValidation({...content,body:'a'.repeat(30001)}));
  assert.ok(articleValidation({...content,references_list:Array(11).fill('reference')}));
});
test('Doctor BFF exposes only own-article authoring without Admin review routes',()=>{
  const id='10000001-0000-0000-0000-000000000001';
  assert.equal(safeBackendPath('doctor/articles','POST'),true);
  assert.equal(safeBackendPath(`doctor/articles/${id}/publish`,'POST'),true);
  assert.equal(safeBackendPath(`doctor/articles/${id}/archive`,'POST'),true);
  assert.equal(safeBackendPath(`doctor/articles/${id}`,'PATCH'),true);
  assert.equal(safeBackendPath(`doctor/articles/${id}/publish`,'GET'),false);
  assert.equal(safeBackendPath('admin/articles','GET'),false);
});
