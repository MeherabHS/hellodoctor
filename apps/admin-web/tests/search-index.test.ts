import test from 'node:test';
import assert from 'node:assert/strict';
import {buildSearchIndex,searchIndex} from '../src/lib/searchIndex.ts';

const people=[
  {name:'Neema Kamau',id:'#PT-DA9A6',county:'Nairobi',phone:'+254712345678',email:'neema@example.test'},
  {name:'Brian Otieno',id:'#PT-77B01',county:'Kisumu',phone:'+254798765432',email:'brian@example.test'},
  {name:'Wanjikú Mwangi',id:'#PT-0C1D2',county:'Nyeri',phone:null,email:null},
];
const index=buildSearchIndex(people,p=>[p.name,p.id,p.county,p.phone,p.email]);
const names=(q:string)=>searchIndex(index,q).map(p=>p.name);

test('every word must match, in any order, ignoring case',()=>{
  assert.deepEqual(names('kamau NAIROBI'),['Neema Kamau']);
  assert.deepEqual(names('nairobi brian'),[]);
  assert.deepEqual(names(''),people.map(p=>p.name));
});
test('accents are ignored',()=>{assert.deepEqual(names('wanjiku'),['Wanjikú Mwangi']);});
test('patient IDs match with or without punctuation',()=>{
  for(const q of ['#PT-DA9A6','pt-da9a6','PT DA9A6','ptda9a6','da9a6'])assert.deepEqual(names(q),['Neema Kamau'],q);
});
test('phone numbers match by digits, with or without +254',()=>{
  for(const q of ['0712345678','+254712345678','0712 345 678','712345678'])assert.deepEqual(names(q),['Neema Kamau'],q);
});
test('emails match',()=>{assert.deepEqual(names('brian@example'),['Brian Otieno']);});
test('regex and HTML characters are plain text',()=>{
  // "." is matched literally (both emails contain one), never as a pattern.
  assert.deepEqual(names('.*'),['Neema Kamau','Brian Otieno']);
  assert.deepEqual(names('(['),people.map(p=>p.name));
  assert.deepEqual(names('<script>'),[]);
});
