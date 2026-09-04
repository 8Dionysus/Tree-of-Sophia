#!/bin/sh
set -eu

: "${TOS_SITE_ROOT:?TOS_SITE_ROOT must point to a Tree-of-Sophia checkout}"

tos_python=${TOS_PYTHON:-python3}
tos_port=${TOS_SITE_PORT:-5439}

test -f "${TOS_SITE_ROOT}/access/src/tos_access/__main__.py"
test -f "${TOS_SITE_ROOT}/ToS/derived-exports/tos_corpus_index.min.json"
test -f "${TOS_SITE_ROOT}/ToS/derived-exports/philosophy_graph_projection.min.json"
test -f "${TOS_SITE_ROOT}/access/web/dist/index.html"

cd "${TOS_SITE_ROOT}"
exec env \
  PYTHONDONTWRITEBYTECODE=1 \
  PYTHONPATH="${TOS_SITE_ROOT}/access/src" \
  "${tos_python}" -m tos_access \
  --root "${TOS_SITE_ROOT}" \
  serve \
  --host 127.0.0.1 \
  --port "${tos_port}"
