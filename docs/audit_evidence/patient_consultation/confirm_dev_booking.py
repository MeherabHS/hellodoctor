"""Confirm one synthetic Patient booking through the existing dev webhook.
Usage: python confirm_dev_booking.py APT-YYYYMMDD-XXXXXXXX
Never updates financial database state directly; never confirms real payments.
"""
import hashlib,hmac,json,re,subprocess,sys,uuid
from datetime import datetime,timezone
from pathlib import Path
from urllib.request import Request,urlopen

root=Path(__file__).resolve().parents[3]
if len(sys.argv)!=2:raise SystemExit('Supply the appointment reference shown in the Patient app.')
reference=sys.argv[1]
try: reference=str(uuid.UUID(reference))
except ValueError:
    if not re.fullmatch(r'APT-\d{8}-[A-F0-9]{8}',reference):raise SystemExit('Invalid appointment reference.')
env={}
for line in (root/'services/backend/.env').read_text().splitlines():
    if '=' in line and not line.startswith('#'):
        key,value=line.split('=',1);env[key]=value.strip('"\'')
if env.get('ENVIRONMENT')!='development':raise SystemExit('Development environment required. No payment was changed.')
names=subprocess.check_output(['docker','exec','hellodoctor-postgres','printenv','POSTGRES_USER','POSTGRES_DB'],text=True).splitlines()
# Reference is validated above; the account restriction cannot be supplied by a caller.
statement=f"""SELECT json_build_object('id',a.id,'reference',a.appointment_number,
 'status',a.status,'session',t.payment_session_id,'gateway',t.gateway,'amount',a.consultation_fee)
 FROM appointments a JOIN transactions t ON t.appointment_id=a.id
 JOIN patient_profiles p ON p.id=a.patient_id
 LEFT JOIN patient_profiles guardian ON guardian.id=p.guardian_patient_id
 JOIN users u ON u.id=COALESCE(p.user_id,guardian.user_id)
 WHERE (a.id::text='{reference}' OR a.appointment_number='{reference}')
 AND u.email='patient.test3@hellodoctor.local'"""
raw=subprocess.check_output(['docker','exec','hellodoctor-postgres','psql','-U',names[0],'-d',names[1],'-At','-c',statement],text=True).strip()
if not raw:raise SystemExit('No booking for the synthetic Patient test account. No payment was changed.')
booking=json.loads(raw)
if booking['status']!='PENDING_PAYMENT':
    print(f"{booking['reference']}: server status is {booking['status']}. No new callback sent.")
    raise SystemExit(0)
if booking['gateway']!='MPESA':raise SystemExit('This helper supports only the configured MPESA development contract.')
body=json.dumps({'payment_session_id':booking['session'],'transaction_reference':'DEV-MOBILE-'+str(uuid.uuid4()),
  'status':'SUCCESS','event_timestamp':datetime.now(timezone.utc).isoformat(),'provider_event_id':str(uuid.uuid4())},separators=(',',':')).encode()
signature=hmac.new(env['WEBHOOK_HMAC_SECRET'].encode(),body,hashlib.sha256).hexdigest()
request=Request('http://127.0.0.1:8080/api/v1/webhooks/payment/mpesa',data=body,headers={'Content-Type':'application/json','X-Signature':signature})
with urlopen(request,timeout=30) as response:
    result=json.load(response)
if not result.get('success'):raise SystemExit('Development callback was not accepted. Inspect backend logs.')
print(f"Signed development callback accepted for {booking['reference']} (KES {booking['amount']}). No real payment occurred. Refresh the app to read server confirmation.")
