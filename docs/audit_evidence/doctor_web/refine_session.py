from pathlib import Path
root = Path(__file__).resolve().parents[3]
p = root/'apps/doctor-web/src/components/DoctorApplication.tsx'
s = p.read_text(encoding='utf-8')
s = s.replace('errorText, request }', 'errorText, request, runtimeGeneration }')
if "const [search,setSearch]" not in s:
    s = s.replace("const [page,setPage]=useState<Page>('queue');", "const [page,setPage]=useState<Page>('queue');const [search,setSearch]=useState('');")
s = s.replace("const [search,setSearch]=useState('');const [search,setSearch]=useState('');", "const [search,setSearch]=useState('');")
s = s.replace("setPage('queue');Object.values", "setPage('queue');setSearch('');Object.values")
if 'const renderGeneration=' not in s:
    s = s.replace('const chosen=active??', 'const renderGeneration=runtimeGeneration();\n  const chosen=active??')
s = s.replace('if(chosen)setRx(previous=>', 'if(chosen&&renderGeneration===runtimeGeneration())setRx(previous=>')
s = s.replace('<DesktopAppointments queue={queue} action={action} addSlot=', '<DesktopAppointments queue={queue} action={action} search={search} setSearch={setSearch} addSlot=')
s = s.replace("placeholder:'Search owned appointments…',onChange:()=>go('appointments')", "placeholder:'Search owned appointments…',value:search,onChange:(event:React.ChangeEvent<HTMLInputElement>)=>{setSearch(event.target.value);go('appointments');}")
s = s.replace("e.status===401){wipe();}", "e.status===401){wipe();setRestoring(false);}")
p.write_text(s, encoding='utf-8')
