#!/usr/bin/env python3
"""Native-only Morph producer entry with a narrow Concept signature helper.

The helper remains executable compatibility for the Python Concept consumer.
Full historical producer bytes are retained separately as nonexecuted recipe
material; this entry never invokes a Python producer fallback.
"""
from __future__ import annotations
import os
from pathlib import Path
import shutil
import sys
import unicodedata

REPO = Path(__file__).resolve().parents[1]
COMMAND = "zarathustra-morphology-theme-candidates-v1"

DE_SUFFIXES = (
    ("ern", "nominal_inflection"), ("est", "verbal_inflection"),
    ("em", "nominal_inflection"), ("en", "inflection"),
    ("er", "nominal_inflection"), ("es", "nominal_inflection"),
    ("te", "verbal_inflection"), ("st", "verbal_inflection"),
    ("et", "verbal_inflection"), ("e", "inflection"),
    ("n", "inflection"), ("s", "inflection"), ("t", "verbal_inflection"),
)


RU_SUFFIXES = (
    ("иями", "nominal_inflection"), ("ями", "nominal_inflection"),
    ("ами", "nominal_inflection"), ("ого", "adjectival_inflection"),
    ("ему", "adjectival_inflection"), ("ому", "adjectival_inflection"),
    ("ими", "adjectival_inflection"), ("ыми", "adjectival_inflection"),
    ("аться", "verbal_inflection"), ("яться", "verbal_inflection"),
    ("ить", "verbal_inflection"), ("ать", "verbal_inflection"),
    ("ять", "verbal_inflection"), ("ешь", "verbal_inflection"),
    ("ишь", "verbal_inflection"), ("ете", "verbal_inflection"),
    ("ите", "verbal_inflection"), ("ут", "verbal_inflection"),
    ("ют", "verbal_inflection"), ("ат", "verbal_inflection"),
    ("ят", "verbal_inflection"), ("ла", "verbal_inflection"),
    ("ли", "verbal_inflection"), ("ло", "verbal_inflection"),
    ("ого", "nominal_inflection"), ("его", "nominal_inflection"),
    ("ому", "nominal_inflection"), ("ему", "nominal_inflection"),
    ("ой", "nominal_inflection"), ("ей", "nominal_inflection"),
    ("ий", "adjectival_inflection"), ("ый", "adjectival_inflection"),
    ("ая", "adjectival_inflection"), ("яя", "adjectival_inflection"),
    ("ую", "adjectival_inflection"), ("юю", "adjectival_inflection"),
    ("ов", "nominal_inflection"), ("ев", "nominal_inflection"),
    ("ам", "nominal_inflection"), ("ям", "nominal_inflection"),
    ("ах", "nominal_inflection"), ("ях", "nominal_inflection"),
    ("ом", "nominal_inflection"), ("ем", "nominal_inflection"),
    ("ою", "nominal_inflection"), ("ею", "nominal_inflection"),
    ("ы", "nominal_inflection"), ("и", "nominal_inflection"),
    ("а", "inflection"), ("я", "inflection"), ("у", "inflection"),
    ("ю", "inflection"), ("е", "inflection"), ("о", "inflection"),
    ("ь", "inflection"),
)


DE_FOLD = str.maketrans({"ä": "a", "ö": "o", "ü": "u", "ß": "ss"})


def fold(value: str, language: str) -> str:
    value = unicodedata.normalize("NFC", value).casefold()
    return value.translate(DE_FOLD) if language == "de" else value


def signatures(value: str, language: str) -> list[tuple[str, str, int]]:
    base = fold(value, language)
    rows = [(base, "orthographic_base", 3)]
    suffixes = DE_SUFFIXES if language == "de" else RU_SUFFIXES
    for suffix, method in suffixes:
        if base.endswith(suffix) and len(base) - len(suffix) >= 4:
            stem = base[:-len(suffix)]
            rows.append((stem, method, 2 if method != "inflection" else 1))
    # German umlaut alternation and the Russian historical fold are challenger
    # features, never lemma assignments.
    return list(dict.fromkeys(rows))



def main(argv: list[str] | None = None) -> int:
    args = list(sys.argv[1:] if argv is None else argv)
    # The dedicated native research parser accepts separate option/value tokens.
    args = [token for arg in args for token in (
        ["--source-root", arg.split("=", 1)[1]]
        if arg.startswith("--source-root=") else [arg]
    )]
    native = os.environ.get("TOS_NATIVE_PREPARED_CONSUMER_BIN") or shutil.which("tos")
    if not native or not Path(native).is_absolute():
        print("error: select installed tos through TOS_NATIVE_PREPARED_CONSUMER_BIN or PATH", file=sys.stderr)
        return 1
    command = [native, COMMAND]
    if args not in (["--help"], ["-h"]) and not any(
        arg == "--source-root" or arg.startswith("--source-root=") for arg in args
    ):
        command += ["--source-root", str(REPO)]
    command += args
    try:
        os.execv(native, command)
    except OSError as exc:
        print(f"error: cannot execute native tos: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
