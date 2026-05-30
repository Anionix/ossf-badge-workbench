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
  --old-ref agent-ready-repo-template-spec \
  --output docs/openssf-checksheet-12855.yml
```

Generate reviewable proposal URLs:

```sh
cargo run -- proposals \
  --input docs/openssf-checksheet-12855.yml \
  --mode criterion \
  --out .cache/openssf/12855/proposals.md
```

Generate CSV, HTML, and readback JSON:

```sh
cargo run -- render \
  --input docs/openssf-checksheet-12855.yml \
  --out-dir .cache/openssf/12855
```

## Conservative Autofill

The tool carries forward current readback values unless a row needs review.
Old repository references become `review_decision: revise`. Private or
admin-only evidence becomes `save_state: deferred`. Gold social/process claims
must remain unclaimed unless the project really satisfies them.

## Repository Layout

- `templates/openssf-checksheet.template.yml`: empty reusable source template
- `data/openssf/criteria-catalog.yml`: criterion metadata used by autofill
- `schemas/`: JSON Schemas for the checksheet and catalog shapes
- `examples/`: small examples for humans and tests

## Status

This is a small v1 workbench. It is intentionally file-first and maintainer
review-first.
