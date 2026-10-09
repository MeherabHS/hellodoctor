from pathlib import Path
import re
root=Path(__file__).resolve().parents[3]/'apps/doctor-web/src'
p=root/'lib/server.ts';s=p.read_text(encoding='utf-8')
for name in ['clearSession','setTokens','invalidateCurrentScope']:
    s=s.replace('function '+name+'(', 'async function '+name+'(')
    s=re.sub(r'(?<!function )\b'+name+r'\(', 'await '+name+'(', s)
s=s.replace('cookies().', '(await cookies()).')
p.write_text(s,encoding='utf-8')
