#!/bin/sh
set -eu
: "${TOS_ACCESS_BIN:?Set TOS_ACCESS_BIN to the exact installed native access product}"
case "$TOS_ACCESS_BIN" in /*) ;; *) echo 'TOS_ACCESS_BIN must be absolute' >&2; exit 2;; esac
exec "$TOS_ACCESS_BIN" verify-edge-local "$@"
