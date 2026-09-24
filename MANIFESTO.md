# ∵ causari

**AI-written code has no author. It has causes. Causari records them.**

Every number about AI in code today is a self-report: a vendor's percentage,
a dashboard's churn rate, a survey's estimate. Nobody can re-run it, nobody
can sign it, nobody can contest it with the data. The industry asks *who
wrote this line*. That question has a market, a consortium and, soon, a
native answer inside GitHub.

Causari asks a different question: **is this claim about AI code verifiable
by someone who trusts neither you nor the tool?**

## What Causari is

Causari is the proof layer above every provenance tracker.

- It **reads everyone's evidence**: git trailers (`Co-Authored-By`,
  `Assisted-by`), git-ai notes, Agent Trace records, its own agent hooks.
- It **measures with a public method**: which lines came from an AI-tagged
  change, and how many of them are still alive, hunk by hunk, with the
  method version and its limits written into the result.
- It **signs the result** as a `crovia.seal.v1` receipt, bound to the exact
  commit and to the exact bytes it measured. Anyone re-runs it and gets the
  same bytes. Anyone verifies it offline with a public key.
- It **witnesses the agent's egress**: what left the machine during a coding
  session, and what did not. A signed proof that the secrets, the customer
  data and the protected sources never went to the model (PNX, a TACET
  profile).

Neutral by construction. Causari does not compete with the trackers; it
verifies them, including its own.

## What Causari refuses to do

- **It does not grade.** No rank, no red/amber/green, no "healthy", no
  "waste". Counts, method, confidence, limits. Judgement belongs to the
  reader.
- **It does not publish verdicts about third parties.** Measurements about a
  repository are published as a dataset with a methodology page, a
  confidence interval and an opt-out, never as a leaderboard.
- **It does not claim what it cannot show.** Inline completions leave no git
  trace and are invisible. Paraphrase is invisible. A formatter can move a
  line. Every output says what it covered and what it did not.
- **It does not ask for trust.** Every proof verifies without Causari, its
  servers, or an account.

## Where it belongs

Causari is part of the Crovia family, one grammar in three tenses:

| | proves | that |
|---|---|---|
| **TACET** | silence | a model's public card carried no training-data disclosure in the observed hours |
| **PNX** | non-exfiltration | an agent's egress never carried the protected bytes |
| **Causari** | cause and survival | why a line of code exists, and whether it is still here |

Same rules everywhere: reproducible numbers, no verdicts, verification
without our servers, limits stated first.

## The mark

`∵`: *because*. The mathematical sign for "since, for the reason that".
Three points: the cause, the effect, the proof. It is the whole product in
one glyph, it types on any keyboard, it survives a 16-pixel favicon and a
monochrome terminal, and nobody owns it.

`⊢`: *proves*. It appears next to a number only when a signature verified.
Nowhere else.

No gradients. No traffic lights. Numbers do not have colours.
