// Controlled development verification. No credentials, OTPs or tokens are logged.
import { readFileSync } from 'node:fs';
import { request } from 'node:http';
import { fileURLToPath } from 'node:url';
const source = readFileSync(fileURLToPath(new URL('../../../services/backend/src/seed.rs', import.meta.url)), 'utf8');
const constant = name => source.match(new RegExp(`pub const ${name}: &str = "([^"]+)"`))[1];
async function call(path, body, token) {
  const serialized = body ? JSON.stringify(body) : '';
  return new Promise((resolve, reject) => {
    const req = request({hostname:'127.0.0.1',port:8080,localAddress:'127.0.0.2',path:'/api/v1/'+path,method:body?'POST':'GET',headers:{'Content-Type':'application/json',...(body?{'Content-Length':Buffer.byteLength(serialized)}:{}),...(token?{Authorization:'Bearer '+token}:{})}}, res => {
      const chunks=[];res.on('data',data=>chunks.push(data));res.on('end',()=>{try{resolve({status:res.statusCode,body:JSON.parse(Buffer.concat(chunks).toString())});}catch(error){reject(error);}});
    });
    req.on('error',reject);req.end(serialized);
  });
}
const login=await call('auth/doctor/login',{email:constant('SEED_DOCTOR2_EMAIL'),password:constant('SEED_DOCTOR1_PASSWORD')});
if(login.status!==200){process.stdout.write(JSON.stringify({login:login.status,error:login.body.error?.code}));process.exit(1);}
const verified=await call('auth/doctor/verify-otp',{session_token:login.body.data.session_token,otp_code:login.body.data.debug_otp_code});
if(verified.status!==200){process.stdout.write(JSON.stringify({login:200,mfa:verified.status,error:verified.body.error?.code}));process.exit(1);}
// Stdout is consumed in memory by the browser test; it is never a saved artifact.
process.stdout.write(JSON.stringify({login:200,mfa:200,access:verified.body.data.access_token,refresh:verified.body.data.refresh_token}));
