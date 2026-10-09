#!/bin/sh
set -eu

: "${TOS_SITE_ROOT:?TOS_SITE_ROOT must point to a Tree-of-Sophia checkout}"
: "${TOS_ACCESS_BIN:?TOS_ACCESS_BIN must point to the installed native tos executable}"

tos_port=${TOS_SITE_PORT:-5439}

case "${TOS_ACCESS_BIN}" in
  /*) ;;
  *) printf '%s\n' "TOS_ACCESS_BIN must be an absolute installed path" >&2; exit 2 ;;
esac

test -x "${TOS_ACCESS_BIN}"
test -f "${TOS_SITE_ROOT}/AGENTS.md"
test -f "${TOS_SITE_ROOT}/ToS/source_home.manifest.json"
test -f "${TOS_SITE_ROOT}/ToS/derived-exports/tos_corpus_index.min.json"
test -f "${TOS_SITE_ROOT}/ToS/derived-exports/philosophy_graph_projection.min.json"

exec "${TOS_ACCESS_BIN}" --root "${TOS_SITE_ROOT}" serve "127.0.0.1:${tos_port}"
