# Material Discovery

This route owns reproducible searches for works, expressions, editions,
translations, critical resources, and research material before acquisition.
It stores public-safe query and result evidence, not downloaded payloads or a
rights conclusion inferred from availability.

Use `DISCOVERY_PROTOCOL.md` for the ordered method and
`ToS/contracts/material-discovery-record.schema.json` for a run receipt.

```text
discovery/
├── README.md
├── DISCOVERY_PROTOCOL.md
└── runs/                    # public-safe query/result receipts only
```

Acquired bytes route to the exact item `payload/`; restricted correspondence
routes to `../access-requests/private/`; scholarly interpretation routes to a
research packet or claim, never into the discovery record.
