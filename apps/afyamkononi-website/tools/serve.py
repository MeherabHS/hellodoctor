"""Local preview of the AfyaMkononi website, behaving like Azure Static Web Apps.

    python apps/afyamkononi-website/tools/serve.py [port] [--dist]

Clean URLs (/about serves about.html, /about.html redirects to /about), the
security headers and the blocked paths from staticwebapp.config.json. With
--dist it serves the built folder (tools/build.py) instead of the
source folder, which is exactly what gets deployed.
"""
import fnmatch
import http.server
import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
DIST = '--dist' in sys.argv
SITE = os.path.normpath(os.path.join(HERE, '..'))
ROOT = os.path.join(SITE, 'dist') if DIST else SITE
CONFIG = json.load(open(os.path.join(ROOT, 'staticwebapp.config.json'), encoding='utf-8'))
HEADERS = CONFIG.get('globalHeaders', {})
# Local http, so HTTPS-only directives would break the preview.
HEADERS = {k: v.replace('; upgrade-insecure-requests', '') for k, v in HEADERS.items()
           if k != 'Strict-Transport-Security'}
REDIRECTS = {r['route']: r['redirect'] for r in CONFIG.get('routes', []) if 'redirect' in r}
BLOCKED = [r['route'] for r in CONFIG.get('routes', [])
           if r.get('statusCode') == 404 and 'methods' not in r]


class Handler(http.server.SimpleHTTPRequestHandler):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, directory=ROOT, **kwargs)

    def end_headers(self):
        for k, v in HEADERS.items():
            self.send_header(k, v)
        super().end_headers()

    def _blocked(self, path):
        return any(fnmatch.fnmatchcase(path, pat) for pat in BLOCKED)

    def do_GET(self):
        path, sep, query = self.path.partition('?')
        if self._blocked(path) or path == '/staticwebapp.config.json':
            self.send_error(404)
            return
        if path in REDIRECTS:
            self.send_response(301)
            self.send_header('Location', REDIRECTS[path])
            self.end_headers()
            return
        if path.endswith('.html'):
            clean = '/' if path == '/index.html' else path[:-5]
            self.send_response(301)
            self.send_header('Location', clean + sep + query)
            self.end_headers()
            return
        local = self.translate_path(path)
        if os.path.isdir(local) and path != '/':
            self.send_error(404)  # no directory listings
            return
        if not os.path.exists(local) and os.path.isfile(local + '.html'):
            self.path = path + '.html' + sep + query
        super().do_GET()

    def do_HEAD(self):
        self.do_GET()

    def list_directory(self, path):
        self.send_error(404)
        return None


if __name__ == '__main__':
    port = next((int(a) for a in sys.argv[1:] if a.isdigit()), 4173)
    print(f'Serving {ROOT} on http://127.0.0.1:{port}/', flush=True)
    http.server.ThreadingHTTPServer(('127.0.0.1', port), Handler).serve_forever()
