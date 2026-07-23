# Rights-aware Access Requests

This route prepares lawful written requests for material that cannot be used
under a demonstrated public-domain, open-license, subscription, or existing
permission path. It never authorizes scraping, credential circumvention, or
other technical bypasses.

```text
access-requests/
├── README.md
├── templates/
│   ├── REQUEST_LETTER.md
│   └── REDACTED_PUBLIC_RECEIPT.md
├── public-ledger/             # safe status records, no correspondence/PII
└── private/                   # gitignored messages, identities, attachments
    └── README.md              # tracked boundary only
```

`ToS/contracts/access-request.schema.json` owns the public-safe status record.
Creating a draft does not authorize sending it. Sending, accepting terms,
paying, or uploading material requires a separate explicit human action.

## Workflow

1. Freeze a material card and the exact edition/item sought.
2. Reconcile the rights holder or institution and preserve the public evidence
   for that identification.
3. Select a public institutional contact route and refresh it before sending.
4. State Tree of Sophia, the research purpose, exact material and format,
   local storage conditions, non-redistribution promise, and each permission
   requested separately.
5. Obtain human approval of the exact message and terms before sending.
6. Keep outbound and inbound correspondence under `private/`.
7. Track only a redacted status, response class, permission scope, expiry, and
   safe evidence reference in Git.
8. Update the item's rights record; permission does not silently change work,
   edition, scan, translation, derivative, or publication rights outside its
   explicit scope.
9. Preserve denial, expiry, withdrawal, and non-response as real outcomes.

The status vocabulary is `public-domain`, `open-licensed`,
`permission-granted`, `research-only`, `restricted`, `rights-unknown`, or
`rejected`. These are access workflow states; the fuller rights record remains
the authority for scope and jurisdiction.
