#!/usr/bin/env python3
"""Select bounded software checks; unknown inputs require the full release suite."""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path, PurePosixPath
import re
import subprocess
from urllib.parse import unquote, urlsplit

MODES = ('none', 'browser', 'reader', 'full')
DOC_ROOTS = ('docs/', 'access/')
LINK = re.compile(r'!?\[[^\]\n]*\]\(\s*(?:<([^>]+)>|([^\s)]+))(?:\s+[\'\"][^\n]*?[\'\"])?\s*\)')
REFERENCE = re.compile(r'^\s*\[[^\]\n]+\]:\s*<?([^\s>]+)>?', re.MULTILINE)


def git(root: Path, *args: str) -> bytes:
    return subprocess.check_output(['git', *args], cwd=root)


def changed_paths(root: Path, base: str) -> list[str]:
    # No rename detection: both removed and added paths influence selection.
    return sorted(set(p.decode('utf-8') for p in git(
        root, 'diff', '--name-only', '--no-renames', '-z', base, 'HEAD').split(b'\0') if p))


def select(paths: list[str], force_full: bool = False) -> dict:
    mode, worker, rust = 'none', False, False
    for path in paths:
        p = PurePosixPath(path)
        if p.is_absolute() or '..' in p.parts or '\n' in path:
            raise ValueError(f'invalid Git path: {path!r}')
        # Owner/source Markdown is not automatically treated as human-only docs.
        if (p.suffix == '.md' and p.name != 'AGENTS.md'
                and (len(p.parts) == 1 or path.startswith(DOC_ROOTS))):
            continue
        if path in ('Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml') or path.startswith(('rust/', 'tests/conformance/rust/')):
            rust = True
        elif path.startswith('access/deploy/cloudflare-worker/'):
            worker = True
        elif path.startswith(('access/web/', 'access/e2e/')):
            mode = max(mode, 'browser', key=MODES.index)
        elif path.startswith(('access/src/', 'access/tests/')):
            mode = max(mode, 'reader', key=MODES.index)
            # Worker fixtures exercise parity with the shared Python reader.
            worker = True
        else:
            mode, worker, rust = 'full', True, True
    if force_full or not paths:
        mode, worker, rust = 'full', True, True
    return {'schema_version': 2, 'software_mode': mode, 'worker': worker, 'rust': rust,
            'changed_paths': paths, 'forced_full': force_full or not paths}


def links(text: str) -> set[str]:
    # Literal examples inside fenced blocks are not document links.
    visible, fence = [], None
    for line in text.splitlines():
        match = re.match(r'^\s{0,3}(`{3,}|~{3,})', line)
        if match:
            marker = match.group(1)
            if fence is None:
                fence = marker
            elif marker[0] == fence[0] and len(marker) >= len(fence):
                fence = None
            continue
        if fence is None:
            visible.append(line)
    prose = '\n'.join(visible)
    return {a or b for a, b in LINK.findall(prose)} | set(REFERENCE.findall(prose))


def check_docs(root: Path, base: str, paths: list[str]) -> list[str]:
    errors = []
    for name in paths:
        target = root / name
        if not name.endswith('.md') or not target.is_file():
            continue
        content = target.read_text(encoding='utf-8')
        if re.search(r'^(<<<<<<< |>>>>>>> )', content, re.MULTILINE):
            errors.append(f'{name}: unresolved merge marker')
        previous = subprocess.run(['git', 'show', f'{base}:{name}'], cwd=root,
                                  stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
        old_links = links(previous.stdout.decode('utf-8')) if previous.returncode == 0 else set()
        for href in sorted(links(content) - old_links):
            url = urlsplit(href)
            if url.scheme or url.netloc or not url.path:
                continue
            # Root-relative URLs describe the hosted site, not repository files.
            if url.path.startswith('/'):
                continue
            destination = (target.parent / unquote(url.path)).resolve()
            if not destination.is_relative_to(root.resolve()) or not destination.exists():
                errors.append(f'{name}: missing repository link target {href}')
    return errors


def gate(needs: dict) -> None:
    if needs.get('plan', {}).get('result') != 'success':
        raise ValueError('check selection or documentation validation did not succeed')
    outputs = needs['plan'].get('outputs', {})
    mode, worker, rust = outputs.get('software_mode'), outputs.get('worker'), outputs.get('rust')
    if mode not in MODES or worker not in ('true', 'false') or rust not in ('true', 'false'):
        raise ValueError('missing or invalid check selection')
    for job, required in [('software', mode != 'none'), ('worker', worker == 'true'), ('rust', rust == 'true')]:
        expected = 'success' if required else 'skipped'
        if needs.get(job, {}).get('result') != expected:
            raise ValueError(f'{job}: expected {expected}, got {needs.get(job)}')


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest='command', required=True)
    plan = sub.add_parser('plan')
    plan.add_argument('--base', required=True)
    plan.add_argument('--full', action='store_true')
    sub.add_parser('gate')
    args = parser.parse_args()
    if args.command == 'gate':
        gate(json.loads(os.environ['CI_NEEDS']))
        print('All selected checks succeeded; unselected checks were skipped.')
        return 0
    root = Path(__file__).resolve().parents[1]
    paths = changed_paths(root, args.base)
    errors = check_docs(root, args.base, paths)
    if errors:
        raise ValueError('\n'.join(errors))
    result = select(paths, args.full)
    print(json.dumps(result, indent=2))
    if output := os.environ.get('GITHUB_OUTPUT'):
        with open(output, 'a', encoding='utf-8') as stream:
            stream.write(f"software_mode={result['software_mode']}\n")
            stream.write(f"worker={str(result['worker']).lower()}\n")
            stream.write(f"rust={str(result['rust']).lower()}\n")
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
