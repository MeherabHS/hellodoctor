import test from 'node:test';
import assert from 'node:assert/strict';
import { consultationContext, compareMessageInstants } from '../src/lib/contracts.ts';
import type { Appointment } from '../src/lib/contracts.ts';

test('historical chat retains authoritative encounter identity beyond the first operational page', () => {
  const loaded=Array.from({length:100},(_,i)=>({id:`current-${i}`} as Appointment));
  const selected={id:'older-encounter',patient_display_name:'Historical Patient',status:'COMPLETED',modality:'CHAT'} as Appointment;
  assert.equal(consultationContext(loaded,'older-encounter',selected),selected);
  assert.equal(consultationContext(loaded,'older-encounter',selected)?.patient_display_name,'Historical Patient');
  assert.equal(consultationContext(loaded,'unrelated-encounter',selected),undefined);
  assert.equal(consultationContext(loaded,undefined,selected),undefined);
});

test('a refreshed authoritative row supersedes older selected encounter status', () => {
  const selected={id:'same',status:'CONFIRMED'} as Appointment;
  const refreshed={id:'same',status:'COMPLETED'} as Appointment;
  assert.equal(consultationContext([refreshed],'same',selected),refreshed);
  assert.equal(selected.status,'CONFIRMED');
});


test('message chronology handles whole-second instants, offsets and microsecond cursor boundaries', () => {
  const rows=[{id:'later',time:'2026-10-16T08:00:00.000002Z'},{id:'whole',time:'2026-10-16T08:00:00Z'},{id:'middle',time:'2026-10-16T11:00:00.000001+03:00'}];
  assert.deepEqual([...rows].sort((a,b)=>compareMessageInstants(a.time,b.time)).map(row=>row.id),['whole','middle','later']);
  assert.equal(compareMessageInstants('2026-10-16T08:00:00.123456Z','2026-10-16T11:00:00.123456+03:00'),0);
});
