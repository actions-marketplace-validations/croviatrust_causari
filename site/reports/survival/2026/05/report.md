# Survival Report #5 — 2026-10-12

Counts of surviving lines from AI-tagged commits in 59 open-source repositories, measured with causari 0.4.1, method v4. Counts, not grades: no rank, no verdict; rows are alphabetical.

Page: https://causari.dev/reports/survival/2026/05/  
Data: https://causari.dev/reports/survival/2026/05/report.json  
Feed: https://causari.dev/reports/survival/feed.xml  
Licence: CC-BY-4.0  
DOI: https://doi.org/10.5281/zenodo.23323312  

## Aggregate

18,866,088 of 34,800,950 lines introduced by 73,083 AI-tagged commits in 59 open-source repositories are still at HEAD (54.2 %). 95 % interval over the sampled repositories: 38.3 % to 75.3 %.

- Repositories aggregated: 59
- Commits in those repositories (no merges): 921,545
- AI-tagged (VERIFIED) commits: 73,083
- Lines introduced by them: 34,800,950
- Still attributed to them at HEAD: 18,866,088
- Line-weighted ratio: 54.2 %
- 95 % bootstrap interval over the sampled repositories: 38.3 % to 75.3 %
- Median of per-repository capped ratios: 75.6 %
- 95 % bootstrap interval on that median: 69.0 % to 76.4 %

95 % percentile interval from 2000 bootstrap resamples of the 59 aggregated repositories (with replacement, seed 5). It describes the sampled repositories, not all AI-assisted code, and not the repositories not in this sample.


## Baseline: the same repositories' untagged lines, in the matched age windows

gap = AI-tagged line-weighted survival minus untagged survival re-weighted to the age mix of the AI-tagged lines of the same repository, over age windows where both cohorts hold at least 5 commits, computed inside each repository; untagged = commits with no machine-readable AI signal (human-written, inline-completed and untagged-agent code alike); age = commit date to HEAD date. A negative gap means AI-tagged lines survive less than untagged lines in the matched age windows of the same repository.

- Repositories with an age-matched gap: 58 of 59 with a baseline
- Median gap across them: +4.9 pts
- 95 % bootstrap interval on that median: -0.9 pts to +6.7 pts
- Gaps below zero: 22 · above zero: 36
- Cleared or rewritten (more than 50% of the repository's commits predate the oldest line still at HEAD): OpenHands/OpenHands. Nothing from before that date survives in them, tagged or not; their rows measure the rewrite as much as the code.

Age windows, counts summed across the repositories with a baseline, one row per age window; one large repository can dominate a window, so no gap is computed from these rows: the gap is computed inside each repository and only its median crosses repositories.

| Line age | AI-tagged commits | AI-tagged lines | Still at HEAD | AI-tagged | Untagged commits | Untagged lines | Still at HEAD | Untagged |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| 0–30 d | 16,227 | 6,937,334 | 5,278,312 | 76.1 % | 22,021 | 12,815,911 | 10,425,956 | 81.4 % |
| 30–90 d | 13,242 | 9,542,043 | 4,182,210 | 43.8 % | 41,149 | 28,949,280 | 15,765,949 | 54.5 % |
| 90–180 d | 15,916 | 10,133,481 | 4,430,237 | 43.7 % | 48,624 | 46,727,672 | 38,467,601 | 82.3 % |
| 180–365 d | 14,198 | 6,084,161 | 3,597,014 | 59.1 % | 93,017 | 24,082,238 | 12,475,555 | 51.8 % |
| 365–730 d | 10,408 | 2,033,857 | 1,345,196 | 66.1 % | 132,801 | 20,846,314 | 9,882,004 | 47.4 % |
| 730+ d | 3,092 | 70,074 | 33,119 | 47.3 % | 510,740 | 83,959,165 | 19,364,715 | 23.1 % |

## Repositories (alphabetical)

| Repository | Commits | AI-tagged | Introduced | Still at HEAD | Line-weighted | Capped | Median per commit | Largest commit | Untagged, matched windows | Gap | Reproduce |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|
| Aider-AI/aider | 12,461 | 11,156 | 374,687 | 235,782 | 62.9 % | 41.9 % | 33.3 % | 27 % | 52.7 % | +10.3 pts | `re audit Aider-AI/aider --json` |
| airbytehq/airbyte | 54,058 | 2,527 | 1,127,769 | 964,500 | 85.5 % | 76.4 % | 87.5 % | 44 % | 63.1 % | +22.4 pts | `re audit airbytehq/airbyte --json` |
| anthropics/anthropic-sdk-python | 1,501 | 47 | 5,371 | 4,413 | 82.2 % | 77.8 % | 83.3 % | 30 % | 89.3 % | -6.7 pts | `re audit anthropics/anthropic-sdk-python --json` |
| anthropics/anthropic-sdk-typescript | 1,432 | 61 | 14,883 | 12,391 | 83.3 % | 85.0 % | 95.8 % | 37 % | 88.9 % | -1.0 pts | `re audit anthropics/anthropic-sdk-typescript --json` |
| anthropics/claude-code | 776 | 78 | 31,739 | 29,673 | 93.5 % | 75.6 % | 58.1 % | 69 % | 81.1 % | +12.4 pts | `re audit anthropics/claude-code --json` |
| apache/fluss | 2,232 | 40 | 13,047 | 10,463 | 80.2 % | 79.7 % | 94.5 % | 12 % | 82.5 % | -2.9 pts | `re audit apache/fluss --json` |
| apache/spark | 48,842 | 19 | 1,379 | 1,218 | 88.3 % | 88.3 % | 95.5 % | 19 % | 84.7 % | +7.4 pts | `re audit apache/spark --json` |
| apache/texera | 7,278 | 541 | 208,278 | 165,939 | 79.7 % | 82.9 % | 91.1 % | 8 % | 83.3 % | -3.6 pts | `re audit apache/texera --json` |
| BerriAI/litellm | 44,554 | 5,575 | 2,217,640 | 1,390,900 | 62.7 % | 68.4 % | 67.3 % | 4 % | 53.6 % | +9.2 pts | `re audit BerriAI/litellm --json` |
| browser-use/browser-use | 7,039 | 263 | 19,774 | 4,278 | 21.6 % | 28.9 % | 14.3 % | 19 % | 44.7 % | -23.1 pts | `re audit browser-use/browser-use --json` |
| cfug/flutter.cn | 10,588 | 149 | 68,543 | 59,214 | 86.4 % | 75.1 % | 87.9 % | 48 % | 66.8 % | +19.6 pts | `re audit cfug/flutter.cn --json` |
| cline/cline | 7,487 | 156 | 97,936 | 65,245 | 66.6 % | 68.2 % | 70.6 % | 12 % | 56.1 % | +10.6 pts | `re audit cline/cline --json` |
| cloudflare/workers-sdk | 8,603 | 205 | 90,928 | 72,273 | 79.5 % | 78.0 % | 72.1 % | 8 % | 69.1 % | +10.3 pts | `re audit cloudflare/workers-sdk --json` |
| continuedev/continue | 16,266 | 228 | 27,756 | 18,063 | 65.1 % | 63.5 % | 68.1 % | 11 % | 58.4 % | +6.7 pts | `re audit continuedev/continue --json` |
| cozystack/cozystack | 5,531 | 2,763 | 617,439 | 479,687 | 77.7 % | 77.8 % | 84.6 % | 6 % | 82.2 % | -4.6 pts | `re audit cozystack/cozystack --json` |
| crewAIInc/crewAI | 2,863 | 205 | 3,349,470 | 154,574 | 4.6 % | 76.4 % | 89.3 % | 97 % | 63.3 % | -58.7 pts | `re audit crewAIInc/crewAI --json` |
| croviatrust/causari | 274 | 14 | 6,267 | 5,142 | 82.0 % | 82.0 % | 92.5 % | 25 % | 83.4 % | +1.2 pts | `re audit croviatrust/causari --json` |
| danny-avila/LibreChat | 6,016 | 90 | 84,535 | 78,512 | 92.9 % | 90.1 % | 86.3 % | 24 % | 87.0 % | +5.9 pts | `re audit danny-avila/LibreChat --json` |
| fastrepl/anarlog | 9,831 | 1,531 | 853,469 | 546,950 | 64.1 % | 49.2 % | 14.3 % | 14 % | 55.3 % | +8.8 pts | `re audit fastrepl/anarlog --json` |
| flutter/flutter | 89,443 | 156 | 86,373 | 18,153 | 21.0 % | 65.7 % | 84.3 % | 38 % | 39.7 % | -18.7 pts | `re audit flutter/flutter --json` |
| flutter/website | 8,562 | 152 | 50,155 | 47,624 | 95.0 % | 81.7 % | 93.9 % | 65 % | 68.0 % | +27.1 pts | `re audit flutter/website --json` |
| ghostty-org/ghostty | 13,625 | 84 | 4,351 | 3,134 | 72.0 % | 74.5 % | 88.0 % | 11 % | 49.9 % | +22.1 pts | `re audit ghostty-org/ghostty --json` |
| github/gh-aw-mcpg | 5,733 | 5,274 | 453,721 | 250,832 | 55.3 % | 62.7 % | 70.5 % | 3 % | 41.3 % | +14.0 pts | `re audit github/gh-aw-mcpg --json` |
| google-gemini/gemini-cli | 6,482 | 350 | 74,449 | 47,567 | 63.9 % | 62.5 % | 64.5 % | 4 % | 75.2 % | -12.7 pts | `re audit google-gemini/gemini-cli --json` |
| google/sam | 1,669 | 36 | 248 | 90 | 36.3 % | 39.0 % | 0.0 % | 71 % | 36.4 % | -3.2 pts | `re audit google/sam --json` |
| GoogleCloudPlatform/scion | 5,854 | 86 | 24,153 | 20,058 | 83.0 % | 75.8 % | 80.9 % | 35 % | 82.3 % | +0.8 pts | `re audit GoogleCloudPlatform/scion --json` |
| jdubois/boot-ui | 1,581 | 1,169 | 571,711 | 458,683 | 80.2 % | 79.9 % | 81.5 % | 9 % | 73.7 % | +6.6 pts | `re audit jdubois/boot-ui --json` |
| langchain-ai/langchain | 16,852 | 43 | 7,289 | 5,388 | 73.9 % | 69.8 % | 87.4 % | 24 % | 59.0 % | +14.4 pts | `re audit langchain-ai/langchain --json` |
| langgenius/dify | 14,075 | 597 | 649,272 | 342,362 | 52.7 % | 53.2 % | 56.9 % | 16 % | 44.8 % | +7.9 pts | `re audit langgenius/dify --json` |
| lidge-ai/cli-jaw | 5,721 | 1,361 | 445,819 | 196,468 | 44.1 % | 75.0 % | 68.0 % | 47 % | 56.0 % | -12.0 pts | `re audit lidge-ai/cli-jaw --json` |
| lidge-ai/ima2-gen | 2,059 | 455 | 127,661 | 46,829 | 36.7 % | 53.0 % | 60.0 % | 19 % | 57.4 % | -20.7 pts | `re audit lidge-ai/ima2-gen --json` |
| lobehub/lobe-chat | 14,435 | 2,161 | 2,644,253 | 2,319,734 | 87.7 % | 80.5 % | 82.3 % | 2 % | 82.9 % | +4.8 pts | `re audit lobehub/lobe-chat --json` |
| managarm/managarm | 6,534 | 752 | 58,846 | 45,638 | 77.6 % | 77.8 % | 89.0 % | 5 % | 75.5 % | +2.3 pts | `re audit managarm/managarm --json` |
| mem0ai/mem0 | 2,646 | 95 | 37,062 | 23,896 | 64.5 % | 64.6 % | 83.0 % | 9 % | 65.4 % | -1.7 pts | `re audit mem0ai/mem0 --json` |
| microsoft/BCApps | 4,311 | 728 | 391,690 | 340,596 | 87.0 % | 81.4 % | 85.7 % | 28 % | 92.1 % | -5.3 pts | `re audit microsoft/BCApps --json` |
| microsoft/vscode | 149,593 | 7,387 | 2,853,416 | 2,267,295 | 79.5 % | 72.6 % | 76.2 % | 31 % | 63.4 % | +16.0 pts | `re audit microsoft/vscode --json` |
| microsoft/vscode-copilot-chat | 3,713 | 336 | 995,871 | 939,159 | 94.3 % | 69.0 % | 76.3 % | 88 % | 55.7 % | +38.6 pts | `re audit microsoft/vscode-copilot-chat --json` |
| NovaSky-AI/SkyRL | 1,369 | 419 | 143,612 | 115,318 | 80.3 % | 77.6 % | 82.5 % | 4 % | 64.0 % | +16.3 pts | `re audit NovaSky-AI/SkyRL --json` |
| NVIDIA-NeMo/Switchyard | 541 | 26 | 15,177 | 10,057 | 66.3 % | 67.0 % | 77.1 % | 21 % | 74.7 % | -8.4 pts | `re audit NVIDIA-NeMo/Switchyard --json` |
| openai/codex | 12,122 | 364 | 183,908 | 108,534 | 59.0 % | 58.4 % | 58.0 % | 6 % | 53.2 % | +5.9 pts | `re audit openai/codex --json` |
| openai/openai-agents-python | 2,368 | 6 | 97,483 | 97,002 | 99.5 % | 96.4 % | 89.4 % | 97 % | no shared window | — | `re audit openai/openai-agents-python --json` |
| OpenHands/extensions | 401 | 321 | 92,214 | 73,616 | 79.8 % | 79.3 % | 86.4 % | 3 % | 74.8 % | +5.0 pts | `re audit OpenHands/extensions --json` |
| OpenHands/OpenHands · rewritten | 8,535 | 2,723 | 1,000,629 | 333,748 | 33.4 % | 30.9 % | 0.0 % | 13 % | 39.6 % | -6.2 pts | `re audit OpenHands/OpenHands --json` |
| OpenHands/software-agent-sdk | 2,552 | 1,769 | 502,006 | 398,993 | 79.5 % | 76.4 % | 77.4 % | 11 % | 77.9 % | +1.6 pts | `re audit OpenHands/software-agent-sdk --json` |
| PrefectHQ/prefect | 18,481 | 1,688 | 463,501 | 377,861 | 81.5 % | 80.3 % | 89.2 % | 3 % | 69.4 % | +12.1 pts | `re audit PrefectHQ/prefect --json` |
| pydantic/pydantic-ai | 4,061 | 353 | 688,758 | 584,568 | 84.9 % | 76.4 % | 75.7 % | 63 % | 82.6 % | +2.2 pts | `re audit pydantic/pydantic-ai --json` |
| QwenLM/qwen-code | 9,886 | 567 | 661,978 | 544,888 | 82.3 % | 76.7 % | 52.6 % | 21 % | 72.4 % | +9.9 pts | `re audit QwenLM/qwen-code --json` |
| ray-project/ray | 31,962 | 920 | 565,848 | 356,538 | 63.0 % | 76.3 % | 89.6 % | 25 % | 66.1 % | -3.1 pts | `re audit ray-project/ray --json` |
| RooCodeInc/Roo-Code | 6,210 | 43 | 88,016 | 39,395 | 44.8 % | 57.0 % | 62.6 % | 67 % | 45.6 % | -0.9 pts | `re audit RooCodeInc/Roo-Code --json` |
| run-llama/llama_index | 7,956 | 13 | 8,542 | 7,872 | 92.2 % | 92.2 % | 90.4 % | 73 % | 81.3 % | +10.9 pts | `re audit run-llama/llama_index --json` |
| secdev/scapy | 5,735 | 66 | 8,575 | 8,199 | 95.6 % | 91.6 % | 100.0 % | 29 % | 91.1 % | -2.6 pts | `re audit secdev/scapy --json` |
| smithersai/smithers | 19,573 | 13,583 | 8,702,149 | 2,114,510 | 24.3 % | 46.3 % | 56.8 % | 39 % | 20.4 % | +3.9 pts | `re audit smithersai/smithers --json` |
| sst/opencode | 20,217 | 53 | 26,242 | 3,901 | 14.9 % | 16.3 % | 0.0 % | 42 % | 18.8 % | -5.9 pts | `re audit sst/opencode --json` |
| TracecatHQ/tracecat | 5,952 | 582 | 406,229 | 281,474 | 69.3 % | 67.3 % | 66.7 % | 4 % | 70.4 % | -1.1 pts | `re audit TracecatHQ/tracecat --json` |
| vercel/next.js | 35,967 | 257 | 67,391 | 52,325 | 77.6 % | 75.4 % | 82.1 % | 7 % | 64.6 % | +13.1 pts | `re audit vercel/next.js --json` |
| vllm-project/llm-compressor | 3,252 | 160 | 21,533 | 18,061 | 83.9 % | 75.9 % | 85.7 % | 24 % | 78.0 % | +5.9 pts | `re audit vllm-project/llm-compressor --json` |
| xing-shuyin/pi-web-ui | 1,040 | 31 | 2,459 | 2,078 | 84.5 % | 85.4 % | 93.3 % | 8 % | 79.5 % | +5.0 pts | `re audit xing-shuyin/pi-web-ui --json` |
| youknowone/pyre | 85,626 | 2,143 | 2,318,903 | 1,639,055 | 70.7 % | 68.3 % | 71.3 % | 14 % | 64.3 % | +6.3 pts | `re audit youknowone/pyre --json` |
| zed-industries/zed | 37,219 | 126 | 48,547 | 25,372 | 52.3 % | 58.6 % | 71.3 % | 16 % | 66.3 % | -14.4 pts | `re audit zed-industries/zed --json` |

## By agent, across aggregated repositories (alphabetical)

| Agent | Repositories | Commits | Introduced | Still at HEAD | Line-weighted |
|---|---:|---:|---:|---:|---:|
| ai | 2 | 59 | 2,878 | 2,540 | 88.3 % |
| aider | 4 | 11,167 | 375,563 | 235,811 | 62.8 % |
| claude-code | 51 | 24,430 | 17,208,971 | 8,760,959 | 50.9 % |
| copilot | 4 | 4,037 | 253,701 | 149,235 | 58.8 % |
| cursor | 37 | 1,304 | 4,364,014 | 817,581 | 18.7 % |
| devin | 14 | 8,850 | 3,238,756 | 2,379,154 | 73.5 % |
| gemini | 14 | 1,565 | 477,198 | 290,984 | 61.0 % |
| github-copilot | 39 | 11,588 | 5,715,881 | 4,333,179 | 75.8 % |
| grok | 1 | 258 | 354,071 | 271,785 | 76.8 % |
| jules | 8 | 38 | 3,534 | 2,071 | 58.6 % |
| llm | 1 | 947 | 189,786 | 150,262 | 79.2 % |
| openai-codex | 23 | 4,247 | 1,151,354 | 756,273 | 65.7 % |
| opencode | 2 | 6 | 3,249 | 2,880 | 88.6 % |
| openhands | 5 | 4,554 | 1,459,646 | 711,397 | 48.7 % |
| pi | 2 | 33 | 2,348 | 1,977 | 84.2 % |

## Measured but not aggregated (fewer than 5 AI-tagged commits)

| Repository | Commits | AI-tagged | Introduced | Still at HEAD | Reproduce |
|---|---:|---:|---:|---:|---|
| openai/openai-python | 1,718 | 3 | 63 | 61 | `re audit openai/openai-python --json` |
| stackblitz/bolt.new | 101 | 0 | 0 | 0 | `re audit stackblitz/bolt.new --json` |

## Excluded from this report

- Shallow clones (history truncated; method v4 refuses them): none
- Audits that failed in this run: fern-api/fern, llvm/llvm-project, richlander/dotnet-inspect
- Opted out by their maintainers (https://github.com/croviatrust/causari/blob/main/.github/survival-optout.txt): 0

## Where this measurement connected

The witness recorded, in 20 of 20 uploaded shard logs, 1 destination(s) over 128 connection(s): 1 allowed, 0 blocked, 0 failed, under an allowlist of 1 rule(s) in enforce mode. Every destination the witness saw is one the policy allows.

| Destination | Outcome | Connections | Bytes out | Bytes in |
|---|---|---:|---:|---:|
| github.com:443 | allowed | 128 | 521,443 | 30,788,299,161 |

Signed run sheet: https://causari.dev/reports/survival/2026/05/reach.sheet.json (profile crovia.pnx.v1, witness `urn:causari:survival-report:witness`, capture `proxy-connect`, disclosure `clear`).  
Policy: https://causari.dev/reports/survival/2026/05/egress-policy.json (source https://github.com/croviatrust/causari/blob/main/.github/egress-policy.json), bound in the sheet as `sha256:7a501cbe66356070f9c6c6a5372a8dd425be059bb2315a71bffcccff64692e21`.  
Verify: `tacet-pnx verify reach.sheet.json --policy egress-policy.json` or `re pnx verify reach.sheet.json --policy egress-policy.json`, or paste the sheet and the policy at https://croviatrust.com/registry/seal/verify/.  

Covered: the connection attempts in the shard logs that were uploaded and concatenated into this sheet. Not covered: any shard that uploaded no log; what the runner does outside the measurement step (checking out this repository, installing the tool, uploading the shard); and any connection that did not go through the witness. The record says where the measurement connected through the witness and how many bytes crossed, nothing about their content.

## Method

Method v4, causari 0.4.1. Detection from commit metadata only; survival from `git blame -w -M -C` at HEAD. Per-commit cap: a commit weighs at most the 95th percentile of per-commit introduced line counts in its repository, and never more than 10,000 lines. Sample floor: 5 VERIFIED commits. VERIFIED only; PROBABLE is listed but never summed. Full clones only. Baseline (method v3): the untagged lines of the same repository, by age; the gap is defined in the Baseline section. Details, limits and how to contest a number: https://causari.dev/method.

## What this report is, and is not

This report counts lines. For each repository it states how many lines were introduced by commits that carry machine-readable AI authorship metadata (trailers such as Co-Authored-By naming an agent, bot author identities, aider markers, and, from method v4, git-ai notes that name a non-empty tool; method v3 counted any git-ai note), and how many of those lines git blame still attributes to those commits at HEAD, under the method version stated on the page (blame with -w -M -C, a per-commit weight cap, a sample floor, full clones only; from method v3 the untagged lines of the same repository, in the matched age windows, stand next to the AI-tagged ones). Every row is reproducible with one command.

It is not a quality judgement, a productivity measure, total AI usage, proof of authorship, a security finding, a correctness finding, or a causal effect of AI. Deleted lines include removed features and rewritten prototypes; surviving lines include dead code. It is not a sample of all AI-assisted code: inline completions leave no trace in git, untagged agent commits are invisible, and the repositories were selected, not drawn at random: 30 hand-picked and 34 found by GitHub commit search as public repositories with at least 5 commits carrying the same AI authorship metadata and at least 100 stars, most-starred first (discovered 2026-09-27); the selection rule and the counts behind it are public. The intervals describe the sampled repositories only.

Prior measurement work asks related questions with different instruments. GitClear publishes churn reports built from code-change patterns across the repositories it analyses; arXiv 2601.16809 ("Will It Survive?") follows the modification of agent-authored code in 201 projects with its own detector and finds that such code is modified less often than human-written code. This report does not reproduce either method and does not adjudicate between them: it publishes counts from git metadata alone, with the method version, the tool version and the exact bytes behind every number, so that the three can be read side by side.

## Cite

Crovia Trust. Survival Report #5 (2026-10-12). https://causari.dev/reports/survival/2026/05/ DOI 10.5281/zenodo.23323312
