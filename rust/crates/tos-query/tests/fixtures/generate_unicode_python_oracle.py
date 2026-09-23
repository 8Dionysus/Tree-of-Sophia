#!/usr/bin/env python3
"""Independent Python 3.14 / UCD 16.0 search normalization oracle."""

import json
import sys
import unicodedata


def main() -> None:
    assert sys.version_info[:2] == (3, 14), sys.version
    assert unicodedata.unidata_version == "16.0.0", unicodedata.unidata_version
    marks = "\u0301" * 2048
    mapped_scalars = "".join(
        chr(point)
        for point in range(0x110000)
        if not 0xD800 <= point <= 0xDFFF and chr(point).lower() != chr(point)
    )
    whitespace = "".join(chr(point) for point in range(0x110000) if chr(point).isspace())
    cases = [
        ("empty", ""),
        ("ascii_edges", " \tAlpha BETA\n"),
        ("sharp_s", "  Straße STRASSE  "),
        ("capital_sharp_s", "ẞ"),
        ("dotted_i", "İ I\u0307 i\u0307"),
        ("ligature", "ﬃ"),
        ("kelvin", "K"),
        ("sigma_final", "ΟΣ"),
        ("sigma_medial", "ΟΣΑ"),
        ("sigma_no_preceding_cased", "Σ"),
        ("sigma_case_ignorable", "A'Σ"),
        ("sigma_following_ignorable_cased", "AΣ\u0301B"),
        ("sigma_following_ignorable_uncased", "AΣ\u0301!"),
        ("sigma_long_following_uncased", "AΣ" + marks),
        ("sigma_long_following_cased", "AΣ" + marks + "B"),
        ("sigma_long_preceding", "A" + marks + "Σ"),
        ("greek_expansion", "\u0390 \u03b0"),
        ("non_bmp", "𐐀 𐐨"),
        ("strip_ascii_separators", "\u001c\u001dA\u001e\u001f"),
        ("strip_unicode_edges", "\u0085\u00a0\u1680\u2000\u2028\u202f\u205f\u3000A\u3000"),
        ("zero_width_nonspace", "\u200bA\u200b"),
        ("composed_decomposed", "É E\u0301"),
        ("lower_expands_bytes", "İ"),
        ("all_lower_mapped_scalars", mapped_scalars),
        ("all_python_whitespace_edges", whitespace + "X" + whitespace),
    ]
    rows = []
    for name, value in cases:
        stripped = value.strip()
        lowered = stripped.lower()
        rows.append({
            "name": name,
            "input": value,
            "stripped": stripped,
            "lowered": lowered,
            "input_code_points": len(value),
            "lowered_code_points": len(lowered),
            "lowered_utf8_bytes": len(lowered.encode("utf-8")),
        })
    print(json.dumps({"python": "3.14", "unicode": "16.0.0", "profile": "tos-python-native-unicode-v1", "cases": rows}, ensure_ascii=True, separators=(",", ":")))


if __name__ == "__main__":
    main()
