"""Freeze CPython's legacy whole-form-set byte observations, not Rust output."""

from __future__ import annotations

import hashlib
import json
import sys
from pathlib import Path


HERE = Path(__file__).resolve().parent
REPO = HERE.parents[3]
SOURCE = Path(
    "ToS/source-witnesses/works/friedrich-nietzsche/"
    "jenseits-von-gut-und-boese/work.human-forms.json"
)
SOURCE_SHA256 = "7abcf8fe8b90667ac5ca8e2eb5affe51cc56283fb3ca4f7f26a2e03750f4e25b"
MAX_SET_BYTES = 2_097_152
PROFILE = "python_json_indent2_insertion_order_utf8_lf_v1"


def encode(value: object) -> bytes:
    return (
        json.dumps(value, ensure_ascii=False, allow_nan=False, indent=2) + "\n"
    ).encode("utf-8")


def observation(case_id: str, value: object) -> dict:
    raw = encode(value)
    assert len(raw) <= MAX_SET_BYTES
    return {
        "case_id": case_id,
        "profile": PROFILE,
        "input_kind": "inline_json",
        "input_json": json.dumps(value, ensure_ascii=True, allow_nan=False, separators=(",", ":")),
        "expected": {
            "utf8": raw.decode("utf-8"),
            "size_bytes": len(raw),
            "sha256": hashlib.sha256(raw).hexdigest(),
        },
    }


def main() -> None:
    cases = [
        observation("empty-form-set", {
            "forms": [], "prior_forms": [], "growth_history": [],
        }),
        observation("fixed-receipt-context", {
            "forms": [{"form_id": "tos.form.synthetic", "form_version": 2,
                       "language": "ru", "content": {"kind": "source-copy", "slot": "wording"}}],
            "prior_forms": [],
            "growth_history": [{
                "command_id": "ass-fixed-command", "request_digest": "sha256:" + "1" * 64,
                "principal_id": "ass-synthetic", "authority_ref": "ass:fixture",
                "owner_configuration": "sha256:" + "2" * 64,
                "recorded_at": "2026-09-23T00:00:00+00:00",
                "source": {"id": "tos.work.synthetic", "version": 7,
                           "digest": "sha256:" + "3" * 64},
                "previous_revision": None,
                "results": [{"id": "tos.form.synthetic", "version": 2,
                             "digest": "sha256:" + "4" * 64}],
            }],
        }),
        observation("unicode-and-controls", {
            "Bö̀se": "По ту сторону добра и зла",
            "escaped": "quote \" slash \\ newline\n tab\t",
            "nested": {"Ω": "漢字", "emoji": "🜁"},
        }),
        observation("number-kinds", {
            "big_int": 18_446_744_073_709_551_617,
            "unsafe_js_int": 9_007_199_254_740_993,
            "negative_zero_float": -0.0,
            "tiny_float": 1e-6,
            "carry_float": 1.9999999999999998,
            "subnormal": 5e-324,
            "maximum": 1.7976931348623157e308,
            "zero_int": 0, "enabled": True, "absent": None,
        }),
        observation("nested-order-and-empty", {
            "z": [{"later": 2, "first": 1}, [], {}],
            "a": {"third": [], "second": {"b": False, "a": None}, "first": ""},
        }),
        observation("order-A-then-B", {"a": 1, "b": 2}),
        observation("order-B-then-A", {"b": 2, "a": 1}),
    ]

    source = (REPO / SOURCE).read_bytes()
    assert hashlib.sha256(source).hexdigest() == SOURCE_SHA256
    loaded = json.loads(source)
    actual = encode(loaded)
    cases.append({
        "case_id": "pinned-public-work-form-set",
        "profile": PROFILE,
        "input_kind": "repo_file",
        "input_path": SOURCE.as_posix(),
        "input_sha256": SOURCE_SHA256,
        "expected": {"size_bytes": len(actual), "sha256": hashlib.sha256(actual).hexdigest()},
    })

    base_size = len(encode({"payload": ""}))
    for case_id, count in [
        ("at-2mib-cap", MAX_SET_BYTES - base_size),
        ("one-over-2mib-cap", MAX_SET_BYTES - base_size + 1),
    ]:
        raw = encode({"payload": "x" * count})
        row = {
            "case_id": case_id, "profile": PROFILE,
            "input_kind": "repeat_string", "key": "payload", "unit": "x", "count": count,
            "expected_size_bytes": len(raw),
        }
        if len(raw) <= MAX_SET_BYTES:
            row["expected"] = {"size_bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest()}
        else:
            row["expected_error"] = "budget_exceeded"
        cases.append(row)

    cases.extend([
        {"case_id": "nonfinite-overflow", "profile": PROFILE,
         "input_kind": "inline_json", "input_json": '{"n":1e5000}',
         "expected_python_error": "ValueError"},
        {"case_id": "unpaired-surrogate", "profile": PROFILE,
         "input_kind": "inline_json", "input_json": '{"s":"\\ud800"}',
         "expected_python_error": "UnicodeEncodeError"},
    ])
    for row in cases:
        if "expected_python_error" not in row:
            continue
        try:
            encode(json.loads(row["input_json"]))
        except (ValueError, UnicodeEncodeError) as error:
            assert type(error).__name__ == row["expected_python_error"]
        else:
            raise AssertionError(f"{row['case_id']} unexpectedly encoded")
    output = HERE / "legacy-whole-form-set.jsonl"
    rendered = "".join(json.dumps(row, ensure_ascii=True, separators=(",", ":")) + "\n"
                       for row in cases)
    if output.exists() and "--update" not in sys.argv:
        assert output.read_text(encoding="utf-8") == rendered, "Python oracle drift"
    else:
        output.write_text(rendered, encoding="utf-8")
    print(f"{len(cases)} cases, {output.name}, source_sha256={SOURCE_SHA256}")


if __name__ == "__main__":
    main()
