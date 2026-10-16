# Shared native fixtures

`knowledge-contract/` and `source-assembly/` retain bounded inputs consumed by
native compiler, command and CI tests. Their local READMEs record provenance.

Worker reference outcomes live in
[`test/fixtures/python-oracles/`](../../deploy/cloudflare-worker/test/fixtures/python-oracles/README.md).
The former Python factory inputs are available in Git history; Worker tests
read the captured outcomes directly.
