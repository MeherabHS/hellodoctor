// Test process only. The same EdDSA claims as backend api_tests::make_token.
// Never imported by Doctor Web. Stdout is captured in memory by the fixture runner.
import { readFileSync } from 'node:fs';
import { createPrivateKey, sign } from 'node:crypto';
import { fileURLToPath } from 'node:url';
const env = Object.fromEntries(readFileSync(fileURLToPath(new URL('../../../services/backend/.env', import.meta.url)), 'utf8').split(/\r?\n/).filter(line => line.includes('=') && !line.startsWith('#')).map(line => [line.slice(0,line.indexOf('=')),line.slice(line.indexOf('=')+1).replace(/^["']|["']$/g,'')]));
if (env.ENVIRONMENT !== 'development') throw new Error('Development fixtures only');
const user = JSON.parse(readFileSync(0, 'utf8')).user_id;
if (!/^[a-f0-9-]{36}$/.test(user)) throw new Error('Invalid fixture identity');
const now = Math.floor(Date.now()/1000);
const encode = value => Buffer.from(JSON.stringify(value)).toString('base64url');
const input = encode({typ:'JWT',alg:'EdDSA'})+'.'+encode({sub:user,user_id:user,role:'DOCTOR',permissions:['doctor:self','consultation:conduct'],iss:'hellodoctor-backend',aud:'hellodoctor-mobile',iat:now,exp:now+900});
const key = createPrivateKey({key:Buffer.from(env.JWT_ED25519_PRIVATE_KEY_B64,'base64'),format:'der',type:'pkcs8'});
process.stdout.write(input+'.'+sign(null,Buffer.from(input),key).toString('base64url'));
