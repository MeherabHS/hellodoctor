"""Create the two blob containers the backend expects in the Azurite emulator.

Azurite keeps data in memory for these runs, so the containers are recreated
after every restore (Invoke-AssuranceScenario.ps1). Uses the emulator's
publicly documented development account and key, which only work against a
local Azurite instance. Standard library only.

    python -I azurite_create_containers.py [http://127.0.0.1:51000]
"""

import base64
import hashlib
import hmac
import sys
import time
import urllib.error
import urllib.request
from email.utils import formatdate

ACCOUNT = "devstoreaccount1"
# Azurite's well-known development key (public; not a secret).
KEY = "Eby8vdM02xNOcqFlqUwJPLlmEtlCDXJ1OUzFT50uSRZ6IFsuFq2UVErCz4I6tq/K1SZFPTOtr/KBHBeksoGMGw=="
VERSION = "2021-08-06"
CONTAINERS = ("clinical-private", "article-media")


def create(base, name):
    date = formatdate(usegmt=True)
    canon_headers = f"x-ms-date:{date}\nx-ms-version:{VERSION}\n"
    canon_resource = f"/{ACCOUNT}/{ACCOUNT}/{name}\nrestype:container"
    # urllib adds this Content-Type to any request with a body, so it is sent
    # explicitly and signed (Content-Length is signed as empty when zero).
    content_type = "application/x-www-form-urlencoded"
    fields = ["", "", "", "", content_type, "", "", "", "", "", ""]
    to_sign = "PUT\n" + "\n".join(fields) + "\n" + canon_headers + canon_resource
    sig = base64.b64encode(
        hmac.new(base64.b64decode(KEY), to_sign.encode(), hashlib.sha256).digest()
    ).decode()
    req = urllib.request.Request(
        f"{base}/{ACCOUNT}/{name}?restype=container",
        method="PUT",
        data=b"",
        headers={
            "x-ms-date": date,
            "x-ms-version": VERSION,
            "Authorization": f"SharedKey {ACCOUNT}:{sig}",
            "Content-Length": "0",
            "Content-Type": content_type,
        },
    )
    try:
        with urllib.request.urlopen(req, timeout=10) as r:
            return r.status
    except urllib.error.HTTPError as e:
        return e.code


def main():
    base = sys.argv[1] if len(sys.argv) > 1 else "http://127.0.0.1:51000"
    for attempt in range(30):
        try:
            results = {n: create(base, n) for n in CONTAINERS}
            break
        except OSError:
            time.sleep(1)
    else:
        sys.exit("Azurite not reachable")
    print(results)
    if any(code not in (201, 409) for code in results.values()):
        sys.exit("container creation failed")


if __name__ == "__main__":
    main()
