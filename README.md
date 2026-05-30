# ossf-badge-workbench

`ossf-badge-workbench` is an unofficial helper for OpenSSF Best Practices badge
work. It keeps badge evidence, conservative autofill decisions, proposal URLs,
and readback checks in reviewable files.

It does not submit the OpenSSF UI for you. Generated proposal URLs should be
opened, reviewed, saved by a maintainer, and then confirmed with JSON readback.

## Install

```sh
cargo install --path .
```

## Workflow

Validate the generic template, catalog, and examples:

```sh
cargo run -- validate
```

Generate a filled checksheet from a project JSON readback:

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
  --signed-release-version v0.1.2 \
  --output docs/openssf-checksheet-12855.yml
```

Generate reviewable proposal URLs:

```sh
cargo run -- proposals \
  --input docs/openssf-checksheet-12855.yml \
  --mode criterion \
  --out .cache/openssf/12855/proposals.md
```

By default, proposal URLs omit rows whose target status is `?`. The upstream
automation proposal docs describe query-string `?` as a reset-to-unknown value,
but current controller behavior rejects parsed unknown URL proposals. This tool
therefore uses a conservative no-reset default; pass `--include-unknown` only
when you intentionally want to inspect explicit unknown proposals.

Export repository automation JSON for checked or accepted rows:

```sh
cargo run -- export bestpractices-json \
  --input docs/openssf-checksheet-12855.yml \
  --out .cache/openssf/12855/.bestpractices.json
```

The export uses proposal-style/internal field names such as
`osps_ac_03_01_status` and omits unknown `?` rows.

Generate CSV, HTML, and readback JSON:

```sh
cargo run -- render \
  --input docs/openssf-checksheet-12855.yml \
  --out-dir .cache/openssf/12855
```

Compare a checksheet against current project JSON readback:

```sh
cargo run -- compare-readback \
  --input docs/openssf-checksheet-12855.yml \
  --project-json-file tests/fixtures/bestpractices-12855.json
```

Refresh OSPS baseline criteria foundations from upstream:

```sh
cargo run -- sync-upstream \
  --catalog data/openssf/criteria-catalog.yml \
  --output data/openssf/criteria-catalog.yml
```

## Conservative Autofill

The tool carries forward current readback values unless a row needs review.
Old repository references become `review_decision: revise`. Private or
admin-only evidence becomes `save_state: deferred`. Gold social/process claims
must remain unclaimed unless the project really satisfies them.

OSPS readback/display keys such as `OSPS-AC-03.01_status` are normalized to
proposal and repository-automation keys such as `osps_ac_03_01_status` at export
and proposal boundaries.

## Repository Layout

- `templates/openssf-checksheet.template.yml`: empty reusable source template
- `data/openssf/criteria-catalog.yml`: criterion metadata used by autofill
- `schemas/`: JSON Schemas for the checksheet and catalog shapes
- `examples/`: small examples for humans and tests

## Status

This is a small v1 workbench. It is intentionally file-first and maintainer
review-first.
