# Antonovsky 1907 RNL-Reported RSL Holding Research

Date of evidence refresh: 2026-08-10
Scope: identifier-first interpretation of the Russian National Library (RNL)
RUSMARC control field and cross-agency holding statement for the exact Saint
Petersburg 1907 third edition of *Thus Spoke Zarathustra* translated by Yu. M.
Antonovsky; current state of the predicted Russian State Library (RSL) public
route and the former RuNEB route
Authority posture: model-made bibliographic and access research over official
RUSMARC documentation, exact current institutional metadata, and official MARC
organization-code evidence; no source book, physical inspection, accepted
transcription, human bibliography, rights clearance, semantics, or canon

## Result first

The value `V 106/216` in the current RNL record is no longer
"uninterpreted nonconventional catalogue data." It is a cross-agency holding
coordinate reported by RNL:

- RUSMARC field `899` is the deprecated Russian communication-format field for
  the location of copies; its `$a` identifies the holding organization and its
  `$j` carries the call number;
- the exact record contains `899 |a RuMoRGB |j V 106/216`;
- official Library of Congress MARC documentation uses `RuMoRGB` for the
  Russian State Library (RSL), including an RSL-authored proposal whose examples
  attach `$5 RuMoRGB` to RSL data;
- therefore the verified metadata assertion is: **RNL reports an RSL holding
  coordinate `V 106/216` for this bibliographic record**.

This does not verify a currently serviceable physical RSL copy. The predicted
RSL public route `01003693382` still returns HTTP `404`, no current RSL record
identifier has been independently resolved, and no RSL item was physically
inspected. ToS can preserve `V 106/216` as an external request coordinate with
its reporting institution encoded in the identifier scheme; it cannot create
an Item, custody claim, availability claim, digital-object claim, or rights
conclusion from it.

The same current RNL response also exposes control field `001`
`v19\rc\1717109`. This is admitted as `RNL RUSMARC 001`, not as an RSL public
record, RSL MARC `001`, RuNEB object identifier, or proof that distinct
identifier wrappers are interchangeable.

## 1. Classical and official format documentation

The official
[RUSMARC international-use block](https://nlr.ru/rusmarc-texts/rusmarc/mf_det8.php)
defines field `899` as `Данные о местонахождении (устаревшее)`. It says that
the field contains location data for copies of the catalogued document, is
repeatable by copy, and was used before the UNIMARC holdings format. The
document defines `$a` as the code or name of the organization holding the copy
and `$j` as the storage call number. The field is historical, but its semantics
remain explicit; "deprecated" does not mean "meaningless."

The current Library of Congress
[MARC Code List for Organizations](https://www.loc.gov/marc/organizations/)
explains that organization codes identify libraries and other bibliographic
organizations, including agencies holding copies. Its official 2002 proposal
[Defining field 065](https://www.loc.gov/marc/marbi/2002/2002-15.html) names
the Russian State Library as the source and repeatedly uses `RuMoRGB` for the
institution to which RSL data applies. Together these sources provide the
format semantics and the organization-code interpretation; neither says that
the particular 1907 copy is presently available.

## 2. Exact current RNL record

The official tagged RUSMARC route remains:

`https://webservices.nlr.ru/util/?method=recordFormat&vid=07NLR_VU1&sysid=004843723&format=001&base=NLR01`

The 9,371-byte response returned HTTP `200` on 2026-08-10 and had SHA-256
`b80e9ac98527f520e9b6ea4ca726fff06a0dc218e68f4b22b8d73b1cf3b8b4ce`.
Its 145-byte response headers had SHA-256
`9baa29da45e085b6520c94089a6d345c6a88494f3c713516cc23c289660b1254`.
The response exposes these distinct fields:

| Field | Literal value | Bounded interpretation |
| --- | --- | --- |
| `001` | `v19\rc\1717109` | RNL/RUSMARC record-control value; admitted only under that scheme |
| `005` | `20150915144811.2` | record transaction timestamp carried by the response; not an access or holdings timestamp |
| `801` | `|a RU |b NLR |g psbo` | RNL source/agency statement |
| `801` | `|a RU |b RuMoRGB |2 psbo` | RSL organization code occurs in record-source metadata |
| `852` | `|a NLR |j 17.145.5.1` | one RNL location and shelfmark |
| `899` | `|a RuMoRGB |j V 106/216` | RNL-reported RSL holding coordinate |
| `899` | `|a БАН` | reported organization without a call number; insufficient as an actionable copy coordinate |
| `899-M` | three RNL shelfmarks | current RNL catalogue holdings already reconciled in the v3 pass |

The HTML response also injected a current `801 2` modification statement with
date `20260810`; it is not used to rewrite the older `005` value or to claim
that RSL independently refreshed its holding on that date.

The `БАН` row is preserved as incomplete institutional metadata. Because it
has no `$j`, ToS does not count it as a resolved remote holding, create an
identifier from it, or open another request solely from this row.

## 3. RSL and RuNEB service-state controls

The predicted RSL route
`https://search.rsl.ru/ru/record/01003693382` returned HTTP `404` on
2026-08-10. The 44,048-byte response body had SHA-256
`fb211bce5038f2d9d31636d01043581b40b6ce8944361af3d1db3724455f37be`;
the 1,184-byte headers had SHA-256
`f417947455187a1a007aa08a9c0fbbfa139530865e8147580e6d5cfd445540ce`.
The number remains a rejected predicted route, not a verified external
identifier.

The exact former RuNEB card
`000199_000009_003693382` timed out during this refresh before any response
bytes arrived. That transient transport result does not supersede the earlier
observed removed-or-replaced state and is not evidence that the bibliographic
record or a physical copy ceased to exist.

No deterministic crosswalk from RNL `001` `v19\rc\1717109` to RSL public
record `01003693382` or to the RuNEB code suffix `003693382` is asserted. The
nearby 1900 and 1903 identifier sequences are useful search leads, but sequence
shape is not identity authority.

## 4. Established and freshest work remain controls

This pass follows, rather than replaces, the already ordered research:

1. Antonovsky's exact 1911 preface remains the source-visible evidence for his
   reported edition sequence.
2. The established 1991 Blok study remains independent evidence for the 1907
   edition and historical holding context.
3. Ermakova's 2025 direct translation-comparison article remains the freshest
   retained methodological control for keeping exact witnesses separate.
4. General web remains last and produced no exact source object or current RSL
   record that outranks the institutional RNL metadata.

Nothing in the new holding interpretation supplies text for collation,
translation judgment, etymology, semantics, or an accepted bibliography.

## 5. Identifier admission decision

| Candidate | Decision | Reason |
| --- | --- | --- |
| `RNL RUSMARC 001 = v19\rc\1717109` | admit as verified metadata | literal current official RNL field `001` |
| `RSL shelfmark reported by RNL = V 106/216` | admit as verified metadata statement | official field semantics plus official `RuMoRGB` interpretation; scheme preserves the reporting boundary |
| `RSL public record = 01003693382` | reject | predicted route remains HTTP `404`; no current RSL record resolved |
| `RSL physical Item under V 106/216` | withhold | no physical inspection, current RSL item row, completeness, condition, or service confirmation |
| `БАН` holding coordinate | withhold | field names an institution but supplies no call number |
| `003693382` as an RSL MARC `001` | withhold | occurs inside the prior RuNEB identifier; no current RSL record proves that scheme assignment |

`verified` in the two admitted corpus identifiers means the literal metadata
statement and its bounded interpretation were verified. It does not mean that
ToS has verified a physical copy or gained custody.

## 6. Access-request consequence

The existing RSL/RuNEB request stays `draft-not-sent`, but its first question
changes. It no longer asks the institution to discover an entirely unknown
call number. It asks RSL to confirm whether the RNL-reported `V 106/216` is a
current RSL coordinate, which department or collection owns it, whether it
maps to the former RuNEB object `000199_000009_003693382`, and what lawful
viewing or copying route applies. Only after that confirmation does the tiered
request ask for key bibliographic pages and, if allowed, a local research copy.

No message, form, order, payment, digitization recommendation, permission
acceptance, or external upload was made.

## 7. Exact boundary after this pass

Materialized:

- one additional RNL record-control identifier;
- one explicitly attributed RNL-reported RSL shelfmark;
- one versioned discovery record and provenance event;
- one refocused public-safe, unsent access-request record.

Still absent:

- a current RSL public record identifier;
- an independently confirmed RSL item or current service state;
- a resolved БАН call number;
- source pages, book bytes, OCR, transcription, collation, or accepted text;
- redistribution, derivative-publication, or server-processing permission;
- translation, etymological, semantic, sign, concept, graph, canon, or human
  review claim.

The durable conclusion is narrow: **RNL reports that RSL holds the 1907
edition under `V 106/216`; ToS can address that report, but only RSL or direct
inspection can close the physical-copy claim.**
