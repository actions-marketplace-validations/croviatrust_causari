# Causari review, 2026-09-20

A read-only study of the whole project at `0fa1874` (v0.1.5), done before any
change of direction. Five areas, five reports, one synthesis. Every finding
marked **[R]** in the synthesis was reproduced against the built binary, not
inferred from reading.

| File | Area |
|---|---|
| [`00-synthesis.md`](00-synthesis.md) | What Causari is today, what is good, the hard truths ranked, the market, the thesis |
| [`01-storage.md`](01-storage.md) | Object model, on-disk layout, atomicity, performance, relationship with git |
| [`02-capture.md`](02-capture.md) | Proxy, watch, hooks, MCP, skills; attribution model; integration reality |
| [`03-proofs-audit.md`](03-proofs-audit.md) | Survival metric, Seal conformance, Proof, guard/churn, the leaderboard |
| [`04-cli.md`](04-cli.md) | Every command as a product: maturity, UX, coherence, release pipeline |
| [`05-surface-market.md`](05-surface-market.md) | Public claims vs reality, live status, competitors, channels |

The decisions taken from this review are in [`../../ROADMAP.md`](../../ROADMAP.md).
The thesis is in [`../../MANIFESTO.md`](../../MANIFESTO.md).

These documents describe the code *as it was* on 2026-09-20. Line numbers
refer to that commit and will drift; the failure modes are what matter.
