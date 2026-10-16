#!/bin/sh
set -eu

: "${TOS_ACCESS_BIN:?TOS_ACCESS_BIN must point to the installed native tos executable}"

tos_site_root=${TOS_SITE_ROOT:-}
source_text_search_selection=${TOS_SOURCE_TEXT_SEARCH_SELECTION:-}
tos_port=${TOS_SITE_PORT:-5439}

case "${TOS_ACCESS_BIN}" in
  /*) ;;
  *) printf '%s\n' "TOS_ACCESS_BIN must be an absolute installed path" >&2; exit 2 ;;
esac

test -x "${TOS_ACCESS_BIN}"

set --
if [ -n "${tos_site_root}" ]; then
  case "${tos_site_root}" in
    /*) ;;
    *) printf '%s\n' "TOS_SITE_ROOT must be an absolute path" >&2; exit 2 ;;
  esac
  test -f "${tos_site_root}/AGENTS.md"
  test -f "${tos_site_root}/ToS/source_home.manifest.json"
  test -f "${tos_site_root}/ToS/derived-exports/tos_corpus_index.min.json"
  test -f "${tos_site_root}/ToS/derived-exports/philosophy_graph_projection.min.json"
  set -- "$@" --root "${tos_site_root}"
fi

if [ -n "${source_text_search_selection}" ]; then
  case "${source_text_search_selection}" in
    /*) ;;
    *) printf '%s\n' "TOS_SOURCE_TEXT_SEARCH_SELECTION must be an absolute path" >&2; exit 2 ;;
  esac
  set -- "$@" --source-text-search-selection "${source_text_search_selection}"
fi

if [ "$#" -eq 0 ]; then
  printf '%s\n' "set TOS_SITE_ROOT or TOS_SOURCE_TEXT_SEARCH_SELECTION to select origin data" >&2
  exit 2
fi

exec "${TOS_ACCESS_BIN}" "$@" serve "127.0.0.1:${tos_port}"
