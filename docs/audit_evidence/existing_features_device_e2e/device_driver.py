"""Drive the connected Android phone using its existing ADB, without an emulator.

No application data is cleared. Screenshots/UI trees are synthetic-account test
evidence. Text input is deliberately not echoed to the action log.
"""
import json
import os
import re
import shlex
import subprocess
import sys
import time
import xml.etree.ElementTree as ET
from datetime import datetime, timezone
from pathlib import Path

sys.stdout.reconfigure(encoding='utf-8')

OUT = Path(__file__).resolve().parent
ADB = str(Path(os.environ['LOCALAPPDATA']) / 'Android/Sdk/platform-tools/adb.exe')
APPS = {'patient': 'com.hellodoctor.patient', 'doctor': 'com.example.hellodoctor_mobile'}


def adb(*args, binary=False):
    data = subprocess.run([ADB, *args], check=True, capture_output=True,
                          timeout=30).stdout
    return data if binary else data.decode('utf-8', errors='replace').strip()


def record(action, **details):
    with (OUT / 'actions.jsonl').open('a', encoding='utf-8') as file:
        file.write(json.dumps({'utc': datetime.now(timezone.utc).isoformat(),
                               'action': action, **details}) + '\n')


def nodes():
    adb('shell', 'uiautomator', 'dump', '/data/local/tmp/hellodoctor-e2e.xml')
    raw = adb('shell', 'cat', '/data/local/tmp/hellodoctor-e2e.xml')
    return list(ET.fromstring(raw).iter('node'))


def label(node):
    if node.get('password') == 'true':
        return '[password field]'
    return node.get('content-desc') or node.get('text') or node.get('hint') or ''


def bounds(node):
    return list(map(int, re.findall(r'\d+', node.get('bounds', ''))))


def click(node):
    x1, y1, x2, y2 = bounds(node)
    assert x2 > x1 and y2 > y1, 'Node has no visible bounds'
    adb('shell', 'input', 'tap', str((x1+x2)//2), str((y1+y2)//2))
    time.sleep(0.7)


def tap(text, contains=False, index=0):
    deadline = time.monotonic() + 15
    while True:
        matches = [n for n in nodes() if n.get('enabled') == 'true'
                   and (text in label(n) if contains else label(n) == text)]
        if len(matches) > index or time.monotonic() >= deadline:
            break
        time.sleep(.5)
    assert len(matches) > index, f'Visible enabled control not found: {text!r}'
    click(matches[index])
    record('tap', label=text, index=index)


def type_text(text):
    # ADB invokes the Android shell: quote the complete argument there.
    adb('shell', 'input text ' + shlex.quote(text.replace(' ', '%s')))
    record('type', character_count=len(text))


def wait_for(text, timeout=20):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if any(text in label(n) for n in nodes()):
            record('assert_ui', text=text)
            return
        time.sleep(.5)
    raise AssertionError('Expected UI text absent: '+text)


def field(index, text):
    edits = [n for n in nodes() if n.get('class') == 'android.widget.EditText']
    assert len(edits) > index, 'Editable field not found'
    click(edits[index])
    type_text(text)


def capture(name):
    assert re.fullmatch(r'[a-zA-Z0-9_-]+', name)
    (OUT / (name+'.png')).write_bytes(adb('exec-out', 'screencap', '-p', binary=True))
    record('capture', file=name+'.png')
    print('Screenshot:', name+'.png', flush=True)


def launch(app, cold=False):
    package = APPS[app]
    if cold:
        adb('shell', 'am', 'force-stop', package)
    adb('shell', 'am', 'start', '-W', '-n', package+'/.MainActivity')
    time.sleep(1.5)
    record('launch', app=app, cold=cold)


def swipe(direction):
    match = re.findall(r'(\d+)x(\d+)', adb('shell', 'wm', 'size'))[-1]
    width, height = map(int, match)
    x = width//2
    high, low = int(height*.28), int(height*.76)
    y1, y2 = (low, high) if direction == 'up' else (high, low)
    adb('shell', 'input', 'swipe', str(x), str(y1), str(x), str(y2), '450')
    time.sleep(.5)
    record('swipe', direction=direction)


if __name__ == '__main__':
    command, *args = sys.argv[1:]
    if command == 'dump':
        for index, node in enumerate(nodes()):
            if label(node) or node.get('class') == 'android.widget.EditText':
                print(index, node.get('class'), bounds(node), repr(label(node)))
    elif command == 'tap':
        tap(args[0], '--contains' in args,
            index=int(args[args.index('--index')+1]) if '--index' in args else 0)
    elif command == 'type':
        type_text(args[0])
    elif command == 'field':
        field(int(args[0]), args[1])
    elif command == 'capture':
        capture(args[0])
    elif command == 'launch':
        launch(args[0], cold='--cold' in args)
    elif command == 'swipe':
        swipe(args[0])
    elif command == 'back':
        adb('shell', 'input', 'keyevent', '4')
        record('back')
    elif command == 'assert':
        assert any(args[0] in label(n) for n in nodes()), 'Expected UI text absent'
        print('UI assertion PASS:', args[0])
    else:
        raise SystemExit('Unknown command')
