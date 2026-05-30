# Agent-ready Template Refresh

This repo can generate the OpenSSF tracking sheet used by
`Anionix/agent-ready-template`.

```sh
cargo run -- fill \
  --project-id 12855 \
  --project-json-url https://www.bestpractices.dev/projects/12855.json \
  --repo-url https://github.com/Anionix/agent-ready-template \
  --homepage-url https://github.com/Anionix/agent-ready-template \
  --badge-level silver \
  --old-ref https://github.com/Anionix/agent-ready-repo-template-spec \
  --old-ref Anionix/agent-ready-repo-template-spec \
  --old-ref agent-ready-repo-template-spec \
  --old-ref agent-ready-repo-template \
  --old-ref agent-ready-repoーtemplate \
  --output docs/openssf-checksheet-12855.yml
```

Copy the generated YAML into the target repository when it is intended to be a
committed project tracking artifact. Keep CSV, HTML, proposal markdown, and
readback JSON under `.cache/` unless they are intentionally added as examples.

Readback rows may reuse a criterion-like key such as `homepage_url`; tooling
treats `track + criterion_key` as the stable row identity.
