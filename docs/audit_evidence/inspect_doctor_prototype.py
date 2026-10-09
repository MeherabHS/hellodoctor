"""Read-only canonical prototype inventory, using the standard library."""
import json
import re
from html.parser import HTMLParser
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / 'prototype/index.html'
VOID = {'area', 'base', 'br', 'col', 'embed', 'hr', 'img', 'input', 'link', 'meta', 'param', 'source', 'track', 'wbr'}

class Parser(HTMLParser):
    def __init__(self):
        super().__init__(convert_charrefs=True)
        self.root = {'tag': 'root', 'attrs': {}, 'children': []}
        self.stack = [self.root]
    def handle_starttag(self, tag, attrs):
        node = {'tag': tag, 'attrs': dict(attrs), 'children': [], 'line': self.getpos()[0]}
        self.stack[-1]['children'].append(node)
        if tag not in VOID:
            self.stack.append(node)
    def handle_startendtag(self, tag, attrs):
        self.handle_starttag(tag, attrs)
        if tag not in VOID:
            self.stack.pop()
    def handle_endtag(self, tag):
        for i in range(len(self.stack)-1, 0, -1):
            if self.stack[i]['tag'] == tag:
                del self.stack[i:]
                break
    def handle_data(self, text):
        self.stack[-1]['children'].append(text)

def walk(n):
    if isinstance(n, dict):
        yield n
        for c in n['children']:
            yield from walk(c)

def label(n):
    return re.sub(r'\s+', ' ', ''.join(c if isinstance(c, str) else label(c) for c in n['children'])).strip()

def main():
    raw = SOURCE.read_text(encoding='utf-8')
    parser = Parser()
    parser.feed(raw)
    nodes = list(walk(parser.root))
    byid = {n['attrs']['id']: n for n in nodes if n['attrs'].get('id')}
    selected = [byid['doctorAppShell'], byid['doctorWebShell']]
    handlers = set()
    rows = []
    def collect(root, screen):
        for n in walk(root):
            a = n['attrs']
            if a.get('id', '').startswith(('doc-view-', 'dwPage_')):
                screen = a['id']
            h = '; '.join(v or '' for k, v in a.items() if k.startswith('on'))
            handlers.update(re.findall(r'\b([A-Za-z_$][\w$]*)\(', h))
            if h or a.get('id') or n['tag'] in ('button', 'input', 'select', 'textarea'):
                rows.append((a.get('id') or '.' + a.get('class', n['tag']).replace(' ', '.'), screen, label(n)[:110], h, n['line']))
    for root in selected:
        collect(root, root['attrs']['id'])
    # Shared Doctor dialogs are outside both shells. Preserve their full inventory.
    modal_ids = [i for i in byid if re.search(r'(doctor|doc|rx|patientHistory|labReport|consultationFee|callOverlay|incomingCall)', i, re.I) and re.search(r'(modal|overlay|sheet|dialog|auth)', i, re.I) and not i.startswith('admin')]
    for i in modal_ids:
        if not any(byid[i] is n for r in selected for n in walk(r)):
            collect(byid[i], i)
    out = ROOT / 'docs/audit_evidence/doctor_web'
    out.mkdir(parents=True, exist_ok=True)
    (out / 'prototype_dom.json').write_text(json.dumps({'roots':selected, 'dialogs':{i:byid[i] for i in modal_ids}, 'handlers':sorted(handlers)}, ensure_ascii=False, indent=2), encoding='utf-8')
    md = ['# Doctor Web canonical prototype inventory', '', 'Source: `prototype/index.html`; root `index.html` was not used.', '', 'Read both the six Doctor views and the distinct desktop Doctor workstation. Prototype text is historical; backend financial/security rules override it. No screen is accepted from source inspection.', '', 'Initial implementation status: NOT_COMPLETE for all rows. Mapping is refined against current source during implementation.', '', '| Prototype ID/class (source line) | Screen | Purpose | Interaction | Data required | Backend source | Mobile reference | Web implementation status |', '|---|---|---|---|---|---|---|---|']
    for ident, screen, text, h, line in rows:
        fields = [f'`{ident}` ({line})',screen,text,h or 'display / field','Current Doctor/session or authorized clinical DTO','See DOCTOR_WEB_BACKEND_MAPPING.md','doctor_pages.dart / doctor_repository.dart','NOT_COMPLETE']
        md.append('| ' + ' | '.join(str(x).replace('|','/').replace('\n',' ') for x in fields) + ' |')
    md += ['', '## Screen roots', '', *[f'- `{i}`' for i in byid if i.startswith(('doc-view-', 'dwPage_'))], '', '## Shared dialog/sheet roots', '', *[f'- `{i}`' for i in modal_ids], '', '## Handler names read', '', ', '.join(f'`{h}`' for h in sorted(handlers))]
    (ROOT / 'docs/DOCTOR_WEB_PROTOTYPE_INVENTORY.md').write_text('\n'.join(md)+'\n', encoding='utf-8')
    print(json.dumps({'rows':len(rows),'screens':[i for i in byid if i.startswith(('doc-view-', 'dwPage_'))], 'dialogs':modal_ids,'handlers':sorted(handlers)}, indent=2))

if __name__ == '__main__':
    main()
