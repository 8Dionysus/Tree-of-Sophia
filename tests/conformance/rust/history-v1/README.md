# Legacy whole HumanForm-set byte oracle

`legacy-whole-form-set.jsonl` is derived by `generate_oracle.py` with CPython
3.14.7 from the exact owner writer expression in
[historical source writer](https://github.com/8Dionysus/Tree-of-Sophia/blob/60e96eceb0903d74e287217fe5e8c1a891f4ab74/mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_commands.py) (line 1709):
`(json.dumps(value, ensure_ascii=False, allow_nan=False, indent=2) + "\n").encode("utf-8")`.
It preserves insertion order, default separators, Python float spelling and
the final LF. This is a **whole published form-set/history** byte profile,
distinct from sorted compact command-input and source-record digest profiles.

Seven small positive rows include exact UTF-8 output and SHA-256. The fixed
receipt row uses synthetic identifiers and `recorded_at=2026-09-23T00:00:00+00:00`
to exercise shape and key order; it is not a valid owner authorization or an
`apply` receipt. The existing public Work's adjacent form set is referenced
by repository path, pinned input SHA-256, output size and output SHA-256; no
source file was copied into this package. The two repeat-string rows construct
the exact 2,097,152-byte cap and one byte over without storing megabytes of
expected text. Two negatives record Python's nonfinite and unpaired-surrogate
failure classes. They are byte-serializer cases, not request decoding rules.

Run `PYTHONDONTWRITEBYTECODE=1 python3
tests/conformance/rust/history-v1/generate_oracle.py` from the repository
root to verify the checked-in oracle. It fails on drift and writes nothing;
`--update` is an intentional regeneration operation. FND can compare the
positive output bytes/digests and error cases with a named Rust profile after
its public API exists. CMD must separately compare a complete isolated
`public-source-forms apply` and replay under the same root/configuration bytes
and injected clock; this package alone does not prove writer or receipt parity.
