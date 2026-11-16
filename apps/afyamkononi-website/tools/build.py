"""Build the deployable AfyaMkononi website: only public files, nothing else.

    python apps/afyamkononi-website/tools/build.py

Copies the published pages, the CSS/JS/images they actually reference, the
Azure config and /.well-known/security.txt from apps/afyamkononi-website/
into apps/afyamkononi-website/dist/. Drafts (concept-*), old design versions, notes
(*.md), screenshots (previews/) and helper scripts (scratch/) never reach the
server. Point Azure Static Web Apps' app_location at the output folder.
"""
import json
import os
import re
import shutil
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
SRC = os.path.normpath(os.path.join(HERE, '..'))
OUT = os.path.join(SRC, 'dist')

DRAFT = re.compile(r'^concept-')
ALWAYS = ['favicon.ico', 'staticwebapp.config.json', 'web.config', '.well-known/security.txt']
REF = re.compile(r'(?:src|href|poster)="([^"#?:]+)')
SRCSET = re.compile(r'srcset="([^"]+)"')
CSS_URL = re.compile(r'url\(["\']?([^"\')?#]+)')
JS_IMPORT = re.compile(r'(?:from|import)\s*\(?\s*["\'](\.{1,2}/[^"\']+)["\']')


def main():
    pages = sorted(f for f in os.listdir(SRC) if f.endswith('.html') and not DRAFT.match(f))
    files = set(pages) | set(ALWAYS)
    queue = list(pages)
    while queue:
        name = queue.pop()
        path = os.path.join(SRC, name)
        if not os.path.isfile(path) or not name.endswith(('.html', '.css', '.js')):
            continue
        text = open(path, encoding='utf-8').read()
        if name.endswith('.html'):
            refs = REF.findall(text) + [c.strip().split(' ')[0] for v in SRCSET.findall(text) for c in v.split(',')]
        elif name.endswith('.js'):
            refs = JS_IMPORT.findall(text)
        else:
            refs = CSS_URL.findall(text)
        base = os.path.dirname(name)
        for ref in refs:
            if ref.startswith(('/', 'data')):
                continue  # routes such as /about, or inline data
            rel = os.path.normpath(os.path.join(base, ref)).replace(os.sep, '/')
            if rel.startswith('..') or rel in files:
                continue
            if os.path.isfile(os.path.join(SRC, rel)):
                files.add(rel)
                queue.append(rel)
    # Licence files ship with the code and fonts they cover.
    files |= {f'assets/fonts/{f}' for f in os.listdir(os.path.join(SRC, 'assets', 'fonts'))}
    files |= {'assets/vendor/zod/LICENSE', 'assets/vendor/zod/VERSION'}

    if os.path.isdir(OUT):
        shutil.rmtree(OUT)
    for rel in sorted(files):
        src = os.path.join(SRC, rel)
        if not os.path.isfile(src):
            sys.exit(f'missing: {rel}')
        dst = os.path.join(OUT, rel)
        os.makedirs(os.path.dirname(dst), exist_ok=True)
        shutil.copy2(src, dst)
    lock_to_front_door()
    size = sum(os.path.getsize(os.path.join(OUT, f)) for f in files)
    print(f'{len(files)} files, {size / 1e6:.1f} MB -> {OUT}')


def lock_to_front_door():
    """With AFD_ID set (the frontDoorId output of deploy/afyamkononi-website/
    frontdoor.bicep in deploy/afyamkononi-website), the Static Web App only answers requests that came
    through that Front Door, so its firewall and rate limits cannot be
    bypassed via the *.azurestaticapps.net address. Needs the SWA Standard
    plan. AFD_HOSTS lists the public hostnames (comma separated)."""
    fd_id = os.environ.get('AFD_ID', '').strip()
    if not fd_id:
        print('note: AFD_ID not set; the site is NOT locked to Front Door')
        return
    if not re.fullmatch(r'[0-9a-fA-F-]{36}', fd_id):
        sys.exit('AFD_ID must be the Front Door ID (a GUID)')
    hosts = [h.strip() for h in os.environ.get(
        'AFD_HOSTS', 'afyamkononi.app,www.afyamkononi.app').split(',') if h.strip()]
    path = os.path.join(OUT, 'staticwebapp.config.json')
    cfg = json.load(open(path, encoding='utf-8'))
    cfg['forwardingGateway'] = {
        'requiredHeaders': {'X-Azure-FDID': fd_id},
        'allowedForwardedHosts': hosts,
    }
    cfg['networking'] = {'allowedIpRanges': ['AzureFrontDoor.Backend']}
    json.dump(cfg, open(path, 'w', encoding='utf-8'), indent=2)
    print(f'locked to Front Door {fd_id} for {", ".join(hosts)}')


if __name__ == '__main__':
    main()
