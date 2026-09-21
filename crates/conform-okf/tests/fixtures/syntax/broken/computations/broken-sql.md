---
type: Attested Computation
title: Orders delivered in a fiscal year
description: Sanctioned SQL with a deliberate syntax error, so that the `# Computation` arm of OkfSyntax has something to catch.
tags: [test, attested]
runtime: bigquery
parameters:
  - { name: year, type: integer, required: true }
executor:
  resource: skills/run-on-bq.md
  receipt: [job_id, executed_sql, result]
attester:
  resource: attesters/sql_equality.py
generated: { by: process:fixture-author, at: 2026-09-21T00:00:00Z }
verified:
  - { by: human:mark@olliver.me.uk, at: 2026-09-21T00:00:00Z }
status: stable
---

# Computation

```sql
SELECT
  COUNT(*) AS delivered
FROM `acme.sales.orders` AS o
WHERE o.order_status = 'delivered'
  AND EXTRACT(YEAR FROM o.order_ts) = @year
GROUP BY
```

The `GROUP BY` names no expression, so this is not a statement any SQL dialect
accepts. It is the one block in an OKF document that something is expected to
*execute*, which is why `OKF310` is a warning where `OKF007` is information.

# Freshness

Nothing here is time-dependent; `stale_after` is deliberately absent so this
fixture produces the same findings on every day it is run.
