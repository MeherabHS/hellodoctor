import test from 'node:test';
import assert from 'node:assert/strict';
import {isPlainEmail} from '../src/lib/inviteEmail.ts';

test('plain addresses are accepted', () => {
  for (const ok of ['admin@afyamkononi.app', 'first.last+ops@example.co.ke', 'A_B-c%9@sub.example.org']) assert.equal(isPlainEmail(ok), true, ok);
});

test('markup, header injection and SQL-shaped input are rejected', () => {
  for (const bad of [
    '<script>alert(1)</script>@example.test', 'x"onmouseover=alert(1)@example.test', 'a\r\nBcc: v@example.test',
    'a,b@example.test', "' OR '1'='1@example.test", "robert'); DROP TABLE users;--@example.test",
    'a..b@example.test', '.a@example.test', 'a@-bad.example', 'a@example', 'a@@example.test', '', 'a@example.c0m',
  ]) assert.equal(isPlainEmail(bad), false, JSON.stringify(bad));
});
