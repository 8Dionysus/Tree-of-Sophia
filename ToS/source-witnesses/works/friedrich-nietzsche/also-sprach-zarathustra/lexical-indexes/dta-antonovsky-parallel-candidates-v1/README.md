# DTA–Antonovsky parallel lexical candidates v1

This route is a whole-work, source-bound observation layer over the four DTA
first-edition witnesses, the Antonovsky 1911 witness, and paragraph alignment
v1. It materializes three deliberately non-authoritative surfaces:

- exact-form recurrence and part/reading dispersion for the Russian witness;
- recurrence-prominence and repeated two-/three-token sequence candidates for
  German and Russian;
- opaque DE↔RU surface-association candidates supported by proposed paragraph
  groups and challenged against strict one-to-one groups and the preserved
  ambiguous/deferred groups.

Exact source strings, token order, occurrence selectors, and human-readable
candidate labels stay in ignored mode-`0600` local artifacts. Tracked outputs
contain hashes, counts, dispersion, method scores, source-returnable alignment
IDs, and explicit `proposed`, `ambiguous`, or `deferred` posture.

`keyword_candidate` means recurrence prominence inside this bounded work. It
is not corpus keyness and does not establish philosophical importance.
`translation_surface_association_candidate` means two mechanically normalized
forms repeatedly co-occur in technically aligned paragraph groups. It is not
lexical equivalence, a translation judgment, a lexeme, a sign, or a concept.

## Rebuild and verify

- first identity issue and build: `python scripts/build_zarathustra_parallel_lexical_candidates_v1.py --build --issue-identities`
- rebuild: `python scripts/build_zarathustra_parallel_lexical_candidates_v1.py --build`
- parity check: `python scripts/build_zarathustra_parallel_lexical_candidates_v1.py --check`
- focused tests: `python -m unittest tests.test_zarathustra_parallel_lexical_candidates_v1`

Identity issuance is a one-time operation. A normal rebuild refuses to remint
opaque association-candidate IDs.

## Authority boundary

The route creates transparent lexical observations and model-free candidate
rankings only. It does not accept either source text, normalize an edition,
establish morphology, lemma, lexeme, sense, sign, concept, translation
equivalence, graph relation, canon, publication, or human review. Promotion
must return to exact occurrences and pass the separate semantic annotation and
review route.
