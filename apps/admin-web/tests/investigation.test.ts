import test from 'node:test';
import assert from 'node:assert/strict';
import { investigationEvents } from '../src/lib/investigation.ts';

test('grievance timeline preserves every event, actor and correlation across both ledgers', () => {
  const trace = [{id:'same', event_type:'CHAT_OPENED', event_time_server:'2026-10-16T11:00:00+03:00', actor_user_id:'doctor-a', auth_session_id:'session-a', request_id:'request-a', result:'ACCEPTED'}, {id:'rejected',event_type:'CHAT_MESSAGE_REJECTED',event_time_server:'2026-10-16T08:01:00Z',result:'REJECTED',reason_code:'WINDOW_EXPIRED'}];
  const audit = [{id:'same',action:'ADMIN_GRIEVANCE_OPENED',created_at:'2026-10-16T07:59:00Z',actor_user_id:'admin-b',request_id:'request-b'}];
  const original = JSON.stringify({trace,audit});
  const rows = investigationEvents(trace,audit);
  assert.equal(rows.length,3);
  assert.deepEqual(rows.map(row=>row.event_type || row.action),['ADMIN_GRIEVANCE_OPENED','CHAT_OPENED','CHAT_MESSAGE_REJECTED']);
  assert.equal(rows[0].source,'Permanent audit');
  assert.equal(rows[0].actor_user_id,'admin-b');
  assert.equal(rows[1].source,'Temporary consultation trace');
  assert.equal(rows[1].actor_user_id,'doctor-a');
  assert.equal(rows[1].auth_session_id,'session-a');
  assert.equal(rows[1].request_id,'request-a');
  assert.equal(rows[2].reason_code,'WINDOW_EXPIRED');
  assert.equal(JSON.stringify({trace,audit}),original);
});

test('missing timestamp is preserved and visibly ordered after timestamped evidence', () => {
  const rows=investigationEvents([{id:'missing',event_type:'UNKNOWN_TIME'}],[{id:'known',action:'ADMIN_ACTION',created_at:'2026-10-16T08:00:00Z'}]);
  assert.deepEqual(rows.map(row=>row.id),['known','missing']);
  assert.equal(rows[1].event_time_server,undefined);
});


test('grievance ordering preserves database microseconds within the same millisecond', () => {
  const rows=investigationEvents([
    {id:'a-later',event_type:'CHAT_MESSAGE_SENT',event_time_server:'2026-10-16T08:00:00.123789Z'},
    {id:'z-first',event_type:'CHAT_OPENED',event_time_server:'2026-10-16T11:00:00.123123+03:00'},
  ],[{id:'m-middle',action:'ADMIN_ACTION',created_at:'2026-10-16T08:00:00.123456Z'}]);
  assert.deepEqual(rows.map(row=>row.id),['z-first','m-middle','a-later']);
});
