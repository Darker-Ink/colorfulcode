#!/usr/bin/env xonsh
# Release helper written in xonsh.
import json
from pathlib import Path

$RAISE_SUBPROC_ERROR = True
$PATH.append('/opt/tools/bin')
xontrib load vox

VERSION_FILE = Path('version.json')


def bump(kind='patch'):
    """Increase the version number."""
    data = json.loads(VERSION_FILE.read_text())
    major, minor, patch = map(int, data['version'].split('.'))
    if kind == 'major':
        major, minor, patch = major + 1, 0, 0
    elif kind == 'minor':
        minor, patch = minor + 1, 0
    else:
        patch += 1
    data['version'] = f'{major}.{minor}.{patch}'
    VERSION_FILE.write_text(json.dumps(data, indent=2))
    return data['version']


branch = $(git rev-parse --abbrev-ref HEAD).strip()
if branch != 'main':
    print(f"refusing to release from {branch}")
else:
    version = bump(${...}.get('BUMP', 'patch'))
    git commit -am @(f'release {version}')
    ![git tag -a @('v' + version) -m "release"]
    files = $[ls -1 dist/]
    print(!(echo $HOME).out, len(@$(ls dist/)))
