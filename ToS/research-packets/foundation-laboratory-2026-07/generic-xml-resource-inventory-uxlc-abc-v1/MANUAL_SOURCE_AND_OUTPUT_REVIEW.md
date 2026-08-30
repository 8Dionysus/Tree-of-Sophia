# Manual source and output review

Date: 2026-08-30  
Reviewer: `model:codex` acting as Tree of Sophia Master  
Human participant: none  
Language competence claimed: none  
State: completed before `evaluate_lab.py` was executed

## Direct source reopen

I reopened both operator-local XML files directly with `xmllint`; I did not
infer their shape from candidate or evaluator output.

| Source | SHA-256 | Bytes | Elements | Attributes | TEI-namespace nodes | TEI `text` nodes | Verses | Words |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| selected exact capture | `98fdc24c…303c` | 8,227 | 133 | 27 | 0 | 0 | 3 | 23 |
| same-selector replay | `d970996c…a30e` | 8,227 | 133 | 27 | 0 | 0 | 3 | 23 |

The provider word grouping is 10 / 7 / 6 in both files. The replay therefore
confirms that the source is well-formed non-namespaced provider XML and still
is not TEI. I did not read or judge the Hebrew strings.

## Representative B return

I resolved these source paths independently with `xmllint`; every expression
returned exactly one element:

```text
/Tanach[1]
/Tanach[1]/teiHeader[1]
/Tanach[1]/tanach[1]
/Tanach[1]/tanach[1]/book[1]/c[1]/v[1]
/Tanach[1]/tanach[1]/book[1]/c[1]/v[3]
/Tanach[1]/tanach[1]/book[1]/c[1]/v[1]/w[1]
/Tanach[1]/tanach[1]/book[1]/c[1]/v[3]/w[6]
```

I then opened the private B owner rather than only its summary:

- `xml-element-000001` is the null-namespace `Tanach` root;
- `xml-element-000002` is the null-namespace `teiHeader`, not a TEI element;
- `xml-element-000050` is the content `tanach` subtree;
- `xml-element-000059` is the first verse, with ten element children and
  one structural attribute name `n`, but no attribute value;
- `xml-element-000078` is the third verse, with six element children;
- `xml-element-000060` is the first provider word path;
- `xml-element-000084` is the sixth word of the third provider verse path.

The structured B locators agreed with the direct XPath returns. Parent refs,
preorder, all-child positions and same-name sibling positions were internally
consistent on these representatives. The separate consumer extended that
check to 133/133 elements in each real file with zero path or metadata
mismatch.

## C and B+C inspection

The primary C output contains 28 provider resources: one book, one chapter,
three verses and 23 word positions. C explicitly states that it is not a
generic XML owner and does not claim intrinsic/cross-corpus word IDs or
accepted ToS structure.

The composed B+C output also contains 28 projection records. I inspected its
first book, first word and last word records:

- the first provider word points to `xml-element-000060` and the exact first
  word path;
- the last provider word points to `xml-element-000084` and the exact sixth
  word path under the third verse;
- all 28 records have non-null generic refs;
- all 28 refs exist in B;
- no projection ref was missing or redirected to another B path.

The consumer independently returned 28/28 provider records through B for each
real capture. C remains a versioned provider navigation view, not a source-
neutral owner or a linguistic judgment.

## Tracked-output source-value inspection

I derived 62 private comparison controls from non-empty source text/tail or
attribute values that either contained a Hebrew code point or had at least 16
characters. I scanned:

- 100 public-synthetic candidate JSON files;
- `run-observations.json`;
- `source-run-receipt.json`;
- `independent-consumer-receipt.json`.

Across 103 tracked generated files there were zero controlled source-value
matches and zero Hebrew code points.

The private B owner also had zero matches. Private C and B+C each had one long
match, but it was exactly the separately declared provider identity already
frozen in `input-manifest.json`, not Hebrew or extracted passage content. Those
outputs are mode `0600` and Git-ignored. No B or B+C element resource contains
`text`, `tail`, attribute values, label/content fingerprint or text/content
digest fields.

This confirms the intended distinction:

```text
public text-free mechanics and receipts
  ≠ private provider provenance metadata
  ≠ source content
```

## Review-correction controls

The post-review rerun binds every measured output path and digest in
`run-observations.json`; the evaluator reopens those paths and rejects any
changed, missing, duplicate, or unexpected output before accepting the
determinism gate. The independent consumer checks the exact source-file
binding for A, B, C, and the B owner inside B+C. Provider chapter and verse
`n` attributes, plus each word's one-based sibling position, are checked
against the resolved XML element rather than only its tag. The evaluator's
freeze-order gate uses the durable UTC timestamp and digest of a pre-output
method freeze, never filesystem checkout mtimes.

## Same-selector correction replay

The two real HTTP responses differ exactly at File level:

- selected File: `98fdc24c…303c`;
- replay File: `d970996c…a30e`.

Their B ordered-topology digest is the same (`c689dcf2…fb1c`) and their
unordered element-shape digest is the same (`94f40897…b676`). C retains the
same 28 resources, three verses, 23 words and 10 / 7 / 6 grouping.

An independent private C14N comparison found four changed element subtrees at
preorders 1, 2, 47 and 49. The selected provider content subtree itself was
equal. No per-element content digest was written to a tracked file.

Therefore the replay supports three separate statements only:

1. the exact capture changed;
2. the element topology remained the same;
3. the provider passage projection shape remained the same.

It does not prove permanent passage identity, textual correctness or
equivalence of Works/Expressions.

## Negative and failure review

The revised run recorded 133 measured processes:

- 116 successful builds;
- 16 DTD/entity/malformed controls failed closed across A/B/C/B+C;
- one C-on-generic-P1 control failed because the source was not UXLC shape;
- zero unexpected exits;
- zero negative outputs.

The first run did fail before source candidate execution: GNU `time` output
parsing and lxml proxy `id()` mapping were wrong. That failure is preserved in
`RUNNER_AND_BC_PROXY_PREFLIGHT_FAILURE_V1.md`; the changed executables were
revision-frozen before the complete rerun. I do not treat the discarded 49
partial v1 outputs as evidence.

## Quality, cost and speed observation

These are process observations, not benchmark claims:

| Shape | Successful processes | Wall range | Peak RSS range | Output range |
|---|---:|---:|---:|---:|
| A | 52 | 0.05–0.07 s | 25,396–25,700 KiB | 1,208–1,223 bytes |
| B | 52 | 0.05–0.07 s | 25,408–26,244 KiB | 3,386–190,972 bytes |
| C | 6 | 0.06–0.07 s | 25,448–25,744 KiB | 9,546–37,670 bytes |
| B+C | 6 | 0.06–0.07 s | 25,472–26,500 KiB | 30,609–246,154 bytes |

Direct monetary cost was zero; electricity and fully loaded cost were not
measured. Human time was zero because no human participant was asked to work.
Manual burden is recorded instead as two exact-source reopens, seven explicit
source XPath returns, representative owner/projection inspection, 103 tracked
file leak checks and one correction comparison. A future human review of
Hebrew content is outside this laboratory and remains unperformed.

## Manual verdict before evaluator

Before running the sealed evaluator, my source-visible verdict is:

- A is retained as the exact capture view but rejected as the only resource
  owner because it cannot return an element;
- B is preferred as the generic exact-file XML element owner;
- C is rejected as the generic owner because it recognizes only UXLC shape;
- C is retained as a useful derived provider projection over B;
- per-element content fingerprints remain excluded;
- the public contract and canonical builder must not be changed merely because
  this private/source-visible laboratory passes.

This verdict concerns owner/projection mechanics only. I have not evaluated
Hebrew orthography, grammar, word segmentation, morphology, translation,
Amenemope/Proverbs relation, philosophical meaning, signs, semantic edges,
canon status or publication rights.
