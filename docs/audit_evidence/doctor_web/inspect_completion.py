import json
from pathlib import Path
from workflow_extra import sql
fixtures=json.loads((Path(__file__).parent/'workflow/results.json').read_text())
for appointment in fixtures['fixture_ids']['appointments']:
    print(sql(f"SELECT id,status,clinical_outcome,completed_at FROM appointments WHERE id='{appointment}'"))
