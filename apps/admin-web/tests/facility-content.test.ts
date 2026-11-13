import test from 'node:test';
import assert from 'node:assert/strict';
import {blankFacility,copyMondayToWeekdays,facilityErrors,isGoogleMapsLink,normalizeKenyanPhone,openingHoursErrors,publishProblems} from '../src/lib/facilityContent.ts';

test('Kenyan phone numbers normalise to E.164; others are refused',()=>{
  assert.equal(normalizeKenyanPhone('0712 345 678'),'+254712345678');
  assert.equal(normalizeKenyanPhone('+254 712-345-678'),'+254712345678');
  assert.equal(normalizeKenyanPhone('(020) 123 4567'),'+254201234567');
  for(const bad of ['12345','+1 202 555 0100','07123456789','0012345678','07x2345678','71+2345678'])assert.equal(normalizeKenyanPhone(bad),null,bad);
});

test('only HTTPS links on official Google Maps hosts are accepted',()=>{
  for(const ok of ['https://www.google.com/maps/place/Siaya+County+Hospital','https://maps.app.goo.gl/AbCdEf123','https://maps.google.com/?q=0.06,34.28','https://www.google.co.ke/maps/dir/?api=1&destination=Siaya'])assert.ok(isGoogleMapsLink(ok),ok);
  for(const bad of ['http://www.google.com/maps/place/x','javascript:alert(1)','https://user:pw@www.google.com/maps','https://www.google.com:8443/maps','https://www.google.com.evil.example/maps','https://evil.example/www.google.com/maps','https://www.google.com/search?q=maps','https://goo.gl/maps/abc','https://maps.app.goo.gl/'])assert.ok(!isGoogleMapsLink(bad),bad);
});

test('opening hours: overlaps, reversed times and overnight rules',()=>{
  assert.deepEqual(openingHoursErrors(null),[]);
  assert.deepEqual(openingHoursErrors({MON:{periods:[{open:'08:00',close:'12:00'},{open:'14:00',close:'17:00'}]}}),[]);
  assert.ok(openingHoursErrors({MON:{periods:[{open:'08:00',close:'13:00'},{open:'12:00',close:'17:00'}]}}).length);
  assert.ok(openingHoursErrors({MON:{periods:[{open:'17:00',close:'08:00'}]}}).length);
  assert.deepEqual(openingHoursErrors({FRI:{periods:[{open:'20:00',close:'06:00',overnight:true}]},SAT:{periods:[{open:'08:00',close:'12:00'}]}}),[]);
  assert.ok(openingHoursErrors({FRI:{periods:[{open:'20:00',close:'06:00',overnight:true}]},SAT:{periods:[{open:'05:00',close:'12:00'}]}}).length);
  assert.ok(openingHoursErrors({SUN:{periods:[{open:'22:00',close:'07:00',overnight:true}]},MON:{periods:[{open:'06:00',close:'12:00'}]}}).length);
  assert.ok(openingHoursErrors({MON:{all_day:true,periods:[{open:'08:00',close:'09:00'}]}}).length);
});

test('copy Monday hours to Tuesday–Friday only',()=>{
  const h=copyMondayToWeekdays({MON:{periods:[{open:'08:00',close:'17:00'}]},SAT:{all_day:true}});
  for(const d of ['TUE','WED','THU','FRI'] as const)assert.deepEqual(h[d],{periods:[{open:'08:00',close:'17:00'}]});
  assert.deepEqual(h.SAT,{all_day:true});assert.equal(h.SUN,undefined);
});

test('a draft needs only a name; publishing needs every required field and a photo',()=>{
  const draft={...blankFacility(),name:'Siaya County Hospital'};
  assert.deepEqual(facilityErrors(draft),[]);
  assert.equal(publishProblems(draft,false).length,6);
  const complete={...draft,county:'Siaya',location_text:'Siaya town',contact_phone:'0712345678',google_maps_url:'https://maps.app.goo.gl/x1',services:['PREP']};
  assert.deepEqual(publishProblems(complete,true),[]);
  assert.deepEqual(publishProblems(complete,false),['A facility photo is required.']);
  assert.ok(facilityErrors({...draft,google_maps_url:'https://evil.example/maps'}).length);
});
