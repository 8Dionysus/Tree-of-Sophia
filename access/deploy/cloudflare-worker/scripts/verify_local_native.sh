#!/bin/sh
set -eu
# npm may close inherited nonstdio FDs. Issue the real ticket after this boundary.
: "${TOS_NODE_EXE:?Set TOS_NODE_EXE to the exact supported Node executable}"
case "$TOS_NODE_EXE" in /*) ;; *) echo 'TOS_NODE_EXE must be absolute' >&2; exit 2;; esac
script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
exec "$TOS_NODE_EXE" "$script_dir/run_native_verifier.mjs" "$@"
