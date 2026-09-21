---
type: Note
title: Illustrative blocks, some of which are wrong
description: Five fenced blocks covering each arm of OkfSyntax's body check - broken, valid, untagged, unrecognised.
tags: [test]
generated: { by: process:fixture-author, at: 2026-09-21T00:00:00Z }
verified:
  - { by: human:mark@olliver.me.uk, at: 2026-09-21T00:00:00Z }
status: stable
---

# Illustrative blocks

A JSON block with a trailing comma. `serde_json` rejects it, and JSON needs no
feature, so this one is caught in **every** build that has `syntax` at all.

```json
{
  "runtime": "bigquery",
  "parameters": ["year"],
}
```

A Python block with no colon after the signature. This is caught only when
`syntax-grammars` is on, and is silently skipped otherwise - a language this
build cannot read is not the same as a language that passed, which is what
`conform_okf_syntax::is_checkable` exists to say.

```python
def check(receipt)
    return receipt["result"][0] > 0
```

A SQL block that is perfectly good, including the BigQuery backtick-quoted
identifiers that `tree-sitter-sequel` could not read and `sqlparser` can. It
must stay quiet, and it is the reason this fixture is not just a list of
errors.

```sql
SELECT COUNT(*) AS n FROM `acme.sales.orders` WHERE order_status IS NOT NULL;
```

An untagged block. `OKFL07` is the rule that reports an untagged block, and it
is a hygiene rule; nothing here says anything about it, because nothing here
knows what language to try.

```
SELCT this is not anything at all
```

A `mermaid` block. An unrecognised tag is not an error - a bundle may fence
`mermaid`, `text` or nothing at all, and refusing to classify is not the same
as refusing the document.

```mermaid
graph TD; A-->B;
```
