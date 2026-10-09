"""Read-only SCRAM credential consistency check; no passwords/hashes logged."""
import base64,hashlib,hmac,json,subprocess
from pathlib import Path
ROOT=Path(__file__).resolve().parents[3]
config={}
for line in (ROOT/'services/backend/.env').read_text().splitlines():
    if '=' in line and not line.startswith('#'):
        name,value=line.split('=',1);config[name]=value.strip().strip('\"\'')
def matches(password,encoded):
    if not encoded.startswith('SCRAM-SHA-256$'):return False
    algorithm,params,keys=encoded.split('$');iterations,salt=params.split(':');stored,server=keys.split(':')
    salted=hashlib.pbkdf2_hmac('sha256',password.encode(),base64.b64decode(salt),int(iterations))
    return hmac.compare_digest(hashlib.sha256(hmac.new(salted,b'Client Key','sha256').digest()).digest(),base64.b64decode(stored))
names=subprocess.check_output(['docker','exec','hellodoctor-postgres','printenv','POSTGRES_USER','POSTGRES_DB'],text=True).splitlines()
data=subprocess.check_output(['docker','exec','hellodoctor-postgres','psql','-U',names[0],'-d',names[1],'-At','-c',"SELECT rolname,rolpassword FROM pg_authid WHERE rolname IN ('hellodoctor_api','hellodoctor_auth_worker','hellodoctor_payment_worker','hellodoctor_system_worker','hellodoctor_finance_worker')"],text=True)
role_keys={'hellodoctor_api':'DB_API_ROLE_PASSWORD','hellodoctor_auth_worker':'DB_AUTH_WORKER_PASSWORD','hellodoctor_payment_worker':'DB_PAYMENT_WORKER_PASSWORD','hellodoctor_system_worker':'DB_SYSTEM_WORKER_PASSWORD','hellodoctor_finance_worker':'DB_FINANCE_WORKER_PASSWORD'}
result=[]
for line in data.splitlines():
    role,encoded=line.split('|',1)
    result.append({'role':role,'matches_current_project_configuration':matches(config[role_keys[role]],encoded),'matches_test_harness_password':matches('test_role_password_dev_only',encoded)})
print(json.dumps(result))
