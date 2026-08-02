# Future Server Import Boundary

No source work is uploaded by this route during the foundation task. This
directory defines how a future Tree of Sophia site may receive an item only
after manifest, fixity, rights, derivative, access, publication, and takedown
gates agree.

```text
server-import/
├── README.md
├── SERVER_IMPORT_PROTOCOL.md
└── plans/                    # tracked metadata-only status records
```

The contract is `ToS/contracts/server-import-contract.schema.json`. A server
receipt is downstream evidence. It never replaces ToS item/file IDs,
manifests, rights records, provenance, reviewed texts, or claims.

The *Ecce Homo* 1908 Commons/Getty plan exercises an active-term failure mode:
positive public-domain results for Nietzsche and Richter and scoped Commons
metadata licenses do not clear the mixed PDF while van de Velde's attributed
design remains in copyright in Germany through 2027-12-31. Its server policy
is `restricted`, `metadata-only`, `blocked-rights`, and operator-unapproved.
Term expiry is only a recheck trigger: any future candidate must be newly
acquired from a current upstream route, receive new fixity and provenance, and
pass layer, quality, human-review, and operator gates. The local payload is
never the upload source.
