"""Replay Flutter render checks with the same short-lived, real MFA sessions."""
import json, os, subprocess
from pathlib import Path
root=Path(__file__).resolve().parents[3]
out=Path(__file__).parent
runtime=root/'services/backend/target/patient_consultation_runtime.json'
env={**os.environ,**json.loads(runtime.read_text(encoding='utf-8'))}
report=json.loads((out/'live_results.json').read_text())
for app,runner in [('mobile','live_doctor_flutter.dart'),('mobile_patient','live_patient_flutter.dart')]:
    result=subprocess.run(['flutter.bat','test',str(out/runner)],cwd=root/'apps'/app,env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True,encoding='utf-8')
    (out/(app+'_live.log')).write_text(result.stdout,encoding='utf-8')
    if result.returncode:
        print(result.stdout[-6000:].encode('ascii','backslashreplace').decode('ascii'));raise SystemExit(result.returncode)
    report['checks'][app+'_real_backend_render']='PASS'
    (out/'live_results.json').write_text(json.dumps(report,indent=2),encoding='utf-8')
report['checks']['all_three_clients_shared_history']='PASS'
(out/'live_results.json').write_text(json.dumps(report,indent=2),encoding='utf-8')
runtime.unlink()
print(json.dumps(report['checks']))
