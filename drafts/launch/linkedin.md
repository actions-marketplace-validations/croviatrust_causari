# LinkedIn (≤ 180 words)

Every number about AI-written code I have seen this year is a self-report: a vendor's percentage, a dashboard, a survey. None of them can be re-run by the person reading it.

So I built one that can. `re audit` reads plain git history, finds the commits that carry machine-readable AI authorship metadata (Co-Authored-By trailers naming an agent, bot authors, AI-* trailers), and asks git blame how many of the lines they introduced are still at HEAD. One binary, any repository, no account, no upload. Anyone re-runs it and gets the same bytes.

The first weekly Survival Report measured 10 open-source repositories: 462,838 of 754,476 lines from 12,349 AI-tagged commits are still at HEAD. That is 61.3 %, with a 95 % interval over that sample of 53.0 % to 64.9 %. It describes those repositories, not AI code in general.

What it cannot see: inline completions leave no git trace. Untagged commits are never counted. It is a count, not a grade.

The method, its limits and how to contest a number: causari.dev/method. Apache-2.0.
