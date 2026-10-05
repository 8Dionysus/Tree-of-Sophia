#!/usr/bin/env python3
"""Compatibility transport for the native reviewed-fragment producer."""
import json
import os
import subprocess
import sys

SOURCE_IDS = ['work', 'chapter-p3.r2', 'moment', 'chapter-p3.r13', 'all-things', 'same-life', 'dossier']

def binary():
    return os.environ.get('TOS_CONSTRUCTOR_FRAGMENTS_BIN', 'tos-constructor-fragments')

def assemble(source, passages, bindings):
    completed = subprocess.run([binary(), '--assemble'], input=json.dumps({'source': source, 'passages': passages, 'bindings': bindings}, ensure_ascii=False).encode(), stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    if completed.returncode:
        raise ValueError(completed.stderr.decode().strip())
    result = json.loads(completed.stdout)
    return result['catalog'], result['catalogText'].encode(), result['library']

def main():
    os.execvp(binary(), [binary(), *sys.argv[1:]])

if __name__ == '__main__':
    main()
