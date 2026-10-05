# Frozen decision-record parity oracle

These exact pre-migration owner scripts are used only by the Rust
`decision_records_native` integration test, which copies them into its isolated
fixture to compare bytes and refusals with the native command. Maintained
commands continue to dispatch to native execution without a Python fallback.

Source revision: `39f0a051273cee6f707b5285d44a2304756d98a4` (parent of native migration join
`e999473564a452f725888dc71371ece08ae97297`). Source paths: `scripts/`.

| File | Bytes | SHA-256 |
| --- | ---: | --- |
| generate_decision_indexes.py | 17850 | `f2203d1219adda3144be7e65164f09ac8c62546e7283c15939dc7c28bbfd24bf` |
| validate_decision_records.py | 1039 | `60f5042745cd765af440b3105d0f4063f539c14a35944d5cc5db5fc3dd48c907` |

This frozen oracle proves historical software parity; it does not issue source,
rights, canon or semantic acceptance.
