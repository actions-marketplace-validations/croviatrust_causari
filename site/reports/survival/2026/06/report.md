# Survival Report #6 — 2026-10-19

Counts of surviving lines from AI-tagged commits in 59 open-source repositories, measured with causari 0.4.1, method v4. Counts, not grades: no rank, no verdict; rows are alphabetical.

Page: https://causari.dev/reports/survival/2026/06/  
Data: https://causari.dev/reports/survival/2026/06/report.json  
Feed: https://causari.dev/reports/survival/feed.xml  
Licence: CC-BY-4.0  

## Aggregate

20,346,193 of 36,935,340 lines introduced by 76,205 AI-tagged commits in 59 open-source repositories are still at HEAD (55.1 %). 95 % interval over the sampled repositories: 40.4 % to 75.2 %.

- Repositories aggregated: 59
- Commits in those repositories (no merges): 929,380
- AI-tagged (VERIFIED) commits: 76,205
- Lines introduced by them: 36,935,340
- Still attributed to them at HEAD: 20,346,193
- Line-weighted ratio: 55.1 %
- 95 % bootstrap interval over the sampled repositories: 40.4 % to 75.2 %
- Median of per-repository capped ratios: 75.6 %
- 95 % bootstrap interval on that median: 69.8 % to 76.8 %

95 % percentile interval from 2000 bootstrap resamples of the 59 aggregated repositories (with replacement, seed 6). It describes the sampled repositories, not all AI-assisted code, and not the repositories not in this sample.


## Baseline: the same repositories' untagged lines, in the matched age windows

gap = AI-tagged line-weighted survival minus untagged survival re-weighted to the age mix of the AI-tagged lines of the same repository, over age windows where both cohorts hold at least 5 commits, computed inside each repository; untagged = commits with no machine-readable AI signal (human-written, inline-completed and untagged-agent code alike); age = commit date to HEAD date. A negative gap means AI-tagged lines survive less than untagged lines in the matched age windows of the same repository.

- Repositories with an age-matched gap: 58 of 59 with a baseline
- Median gap across them: +4.3 pts
- 95 % bootstrap interval on that median: -0.1 pts to +7.0 pts
- Gaps below zero: 22 · above zero: 36
- Cleared or rewritten (more than 50% of the repository's commits predate the oldest line still at HEAD): OpenHands/OpenHands. Nothing from before that date survives in them, tagged or not; their rows measure the rewrite as much as the code.

Age windows, counts summed across the repositories with a baseline, one row per age window; one large repository can dominate a window, so no gap is computed from these rows: the gap is computed inside each repository and only its median crosses repositories.

| Line age | AI-tagged commits | AI-tagged lines | Still at HEAD | AI-tagged | Untagged commits | Untagged lines | Still at HEAD | Untagged |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| 0–30 d | 16,487 | 8,130,839 | 6,224,414 | 76.6 % | 21,750 | 13,234,861 | 10,767,787 | 81.4 % |
| 30–90 d | 14,839 | 9,862,532 | 4,500,812 | 45.6 % | 42,594 | 21,993,197 | 15,902,874 | 72.3 % |
| 90–180 d | 16,350 | 10,307,946 | 4,392,961 | 42.6 % | 49,047 | 55,003,716 | 39,438,889 | 71.7 % |
| 180–365 d | 14,894 | 6,431,828 | 3,786,945 | 58.9 % | 93,324 | 24,519,548 | 12,419,156 | 50.7 % |
| 365–730 d | 10,540 | 2,132,024 | 1,407,942 | 66.0 % | 133,968 | 21,086,274 | 9,976,387 | 47.3 % |
| 730+ d | 3,095 | 70,171 | 33,119 | 47.2 % | 512,377 | 84,165,417 | 19,421,732 | 23.1 % |

## Repositories (alphabetical)

| Repository | Commits | AI-tagged | Introduced | Still at HEAD | Line-weighted | Capped | Median per commit | Largest commit | Untagged, matched windows | Gap | Reproduce |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|
| Aider-AI/aider | 12,461 | 11,156 | 374,687 | 235,782 | 62.9 % | 41.9 % | 33.3 % | 27 % | 52.7 % | +10.3 pts | `re audit Aider-AI/aider --json` |
| airbytehq/airbyte | 54,587 | 2,637 | 1,162,874 | 990,464 | 85.2 % | 76.7 % | 86.7 % | 43 % | 64.2 % | +21.0 pts | `re audit airbytehq/airbyte --json` |
| anthropics/anthropic-sdk-python | 1,529 | 51 | 20,141 | 18,645 | 92.6 % | 80.1 % | 83.3 % | 69 % | 90.9 % | +2.0 pts | `re audit anthropics/anthropic-sdk-python --json` |
| anthropics/anthropic-sdk-typescript | 1,472 | 70 | 30,926 | 28,224 | 91.3 % | 86.1 % | 96.4 % | 50 % | 90.8 % | +4.1 pts | `re audit anthropics/anthropic-sdk-typescript --json` |
| anthropics/claude-code | 790 | 78 | 31,739 | 29,673 | 93.5 % | 75.6 % | 58.1 % | 69 % | 81.6 % | +11.9 pts | `re audit anthropics/claude-code --json` |
| apache/fluss | 2,280 | 44 | 14,189 | 11,254 | 79.3 % | 78.8 % | 92.0 % | 11 % | 83.2 % | -4.4 pts | `re audit apache/fluss --json` |
| apache/spark | 48,939 | 19 | 1,379 | 1,208 | 87.6 % | 87.6 % | 94.1 % | 19 % | 83.8 % | +7.3 pts | `re audit apache/spark --json` |
| apache/texera | 7,303 | 547 | 211,558 | 167,626 | 79.2 % | 82.2 % | 90.6 % | 8 % | 83.2 % | -3.9 pts | `re audit apache/texera --json` |
| BerriAI/litellm | 45,139 | 5,914 | 2,536,387 | 1,669,345 | 65.8 % | 70.8 % | 68.9 % | 3 % | 60.3 % | +5.5 pts | `re audit BerriAI/litellm --json` |
| browser-use/browser-use | 7,077 | 284 | 20,209 | 4,673 | 23.1 % | 30.9 % | 24.8 % | 19 % | 42.5 % | -19.4 pts | `re audit browser-use/browser-use --json` |
| cfug/flutter.cn | 10,588 | 149 | 68,543 | 59,214 | 86.4 % | 75.1 % | 87.9 % | 48 % | 66.8 % | +19.6 pts | `re audit cfug/flutter.cn --json` |
| cline/cline | 7,540 | 158 | 111,580 | 78,241 | 70.1 % | 69.5 % | 71.3 % | 12 % | 55.5 % | +10.5 pts | `re audit cline/cline --json` |
| cloudflare/workers-sdk | 8,689 | 208 | 92,180 | 73,157 | 79.4 % | 77.8 % | 72.0 % | 8 % | 68.5 % | +10.8 pts | `re audit cloudflare/workers-sdk --json` |
| continuedev/continue | 16,266 | 228 | 27,756 | 18,063 | 65.1 % | 63.5 % | 68.1 % | 11 % | 58.4 % | +6.7 pts | `re audit continuedev/continue --json` |
| cozystack/cozystack | 5,559 | 2,790 | 624,499 | 486,062 | 77.8 % | 78.0 % | 84.9 % | 6 % | 82.5 % | -4.6 pts | `re audit cozystack/cozystack --json` |
| crewAIInc/crewAI | 2,895 | 222 | 3,355,820 | 159,701 | 4.8 % | 76.7 % | 88.7 % | 96 % | 63.1 % | -58.3 pts | `re audit crewAIInc/crewAI --json` |
| croviatrust/causari | 278 | 14 | 6,267 | 5,142 | 82.0 % | 82.0 % | 92.5 % | 25 % | 81.2 % | +3.4 pts | `re audit croviatrust/causari --json` |
| danny-avila/LibreChat | 6,209 | 90 | 84,535 | 78,320 | 92.6 % | 89.8 % | 86.3 % | 24 % | 86.4 % | +6.3 pts | `re audit danny-avila/LibreChat --json` |
| fastrepl/anarlog | 9,869 | 1,541 | 858,537 | 550,189 | 64.1 % | 49.4 % | 15.2 % | 14 % | 55.5 % | +8.6 pts | `re audit fastrepl/anarlog --json` |
| flutter/flutter | 89,581 | 157 | 86,626 | 18,223 | 21.0 % | 65.2 % | 83.3 % | 38 % | 39.8 % | -18.8 pts | `re audit flutter/flutter --json` |
| flutter/website | 8,574 | 153 | 50,159 | 47,569 | 94.8 % | 81.2 % | 92.7 % | 65 % | 76.1 % | +19.1 pts | `re audit flutter/website --json` |
| ghostty-org/ghostty | 13,701 | 89 | 4,591 | 3,331 | 72.6 % | 75.6 % | 87.5 % | 10 % | 50.0 % | +22.6 pts | `re audit ghostty-org/ghostty --json` |
| github/gh-aw-mcpg | 5,832 | 5,355 | 457,011 | 246,588 | 54.0 % | 61.0 % | 69.1 % | 3 % | 39.4 % | +14.5 pts | `re audit github/gh-aw-mcpg --json` |
| google-gemini/gemini-cli | 6,507 | 351 | 74,458 | 47,586 | 63.9 % | 62.5 % | 64.6 % | 4 % | 75.4 % | -12.3 pts | `re audit google-gemini/gemini-cli --json` |
| google/sam | 1,802 | 37 | 250 | 90 | 36.0 % | 38.1 % | 0.0 % | 70 % | 42.9 % | -8.5 pts | `re audit google/sam --json` |
| GoogleCloudPlatform/scion | 6,271 | 86 | 24,153 | 19,937 | 82.5 % | 75.0 % | 80.9 % | 35 % | 81.8 % | +0.7 pts | `re audit GoogleCloudPlatform/scion --json` |
| jdubois/boot-ui | 2,058 | 1,613 | 1,046,262 | 745,892 | 71.3 % | 79.7 % | 82.8 % | 11 % | 72.2 % | -0.9 pts | `re audit jdubois/boot-ui --json` |
| langchain-ai/langchain | 16,874 | 43 | 7,289 | 5,384 | 73.9 % | 69.8 % | 87.4 % | 24 % | 59.6 % | +13.7 pts | `re audit langchain-ai/langchain --json` |
| langgenius/dify | 14,288 | 656 | 712,465 | 337,616 | 47.4 % | 48.6 % | 54.7 % | 14 % | 40.8 % | +6.6 pts | `re audit langgenius/dify --json` |
| lidge-ai/cli-jaw | 5,721 | 1,361 | 445,819 | 196,468 | 44.1 % | 75.0 % | 68.0 % | 47 % | 56.0 % | -12.0 pts | `re audit lidge-ai/cli-jaw --json` |
| lidge-ai/ima2-gen | 2,070 | 455 | 127,661 | 46,826 | 36.7 % | 53.0 % | 60.0 % | 19 % | 65.6 % | -28.9 pts | `re audit lidge-ai/ima2-gen --json` |
| lobehub/lobe-chat | 14,723 | 2,328 | 2,940,356 | 2,524,124 | 85.8 % | 78.1 % | 81.2 % | 1 % | 80.5 % | +5.4 pts | `re audit lobehub/lobe-chat --json` |
| managarm/managarm | 6,610 | 816 | 62,188 | 48,162 | 77.4 % | 77.5 % | 88.2 % | 5 % | 75.8 % | +1.9 pts | `re audit managarm/managarm --json` |
| mem0ai/mem0 | 2,648 | 95 | 37,062 | 23,896 | 64.5 % | 64.6 % | 83.0 % | 9 % | 71.8 % | -8.0 pts | `re audit mem0ai/mem0 --json` |
| microsoft/BCApps | 4,388 | 770 | 401,798 | 347,023 | 86.4 % | 80.4 % | 85.6 % | 27 % | 88.0 % | -1.8 pts | `re audit microsoft/BCApps --json` |
| microsoft/vscode | 150,214 | 7,892 | 3,116,538 | 2,478,238 | 79.5 % | 72.9 % | 76.6 % | 28 % | 64.7 % | +14.8 pts | `re audit microsoft/vscode --json` |
| microsoft/vscode-copilot-chat | 3,713 | 336 | 995,871 | 939,159 | 94.3 % | 69.0 % | 76.3 % | 88 % | 55.7 % | +38.6 pts | `re audit microsoft/vscode-copilot-chat --json` |
| NovaSky-AI/SkyRL | 1,386 | 433 | 147,179 | 118,679 | 80.6 % | 77.9 % | 82.5 % | 4 % | 64.3 % | +16.4 pts | `re audit NovaSky-AI/SkyRL --json` |
| NVIDIA-NeMo/Switchyard | 583 | 27 | 15,667 | 10,426 | 66.5 % | 67.2 % | 75.6 % | 20 % | 78.6 % | -12.1 pts | `re audit NVIDIA-NeMo/Switchyard --json` |
| openai/codex | 12,337 | 364 | 183,908 | 108,340 | 58.9 % | 58.3 % | 58.0 % | 6 % | 51.0 % | +8.0 pts | `re audit openai/codex --json` |
| openai/openai-agents-python | 2,391 | 6 | 97,483 | 97,002 | 99.5 % | 96.4 % | 89.4 % | 97 % | no shared window | — | `re audit openai/openai-agents-python --json` |
| OpenHands/extensions | 404 | 321 | 92,214 | 73,514 | 79.7 % | 79.2 % | 85.8 % | 3 % | 75.3 % | +4.5 pts | `re audit OpenHands/extensions --json` |
| OpenHands/OpenHands · rewritten | 8,641 | 2,763 | 1,044,104 | 371,139 | 35.5 % | 31.1 % | 0.0 % | 13 % | 40.6 % | -5.1 pts | `re audit OpenHands/OpenHands --json` |
| OpenHands/software-agent-sdk | 2,658 | 1,827 | 549,997 | 444,625 | 80.8 % | 76.7 % | 78.1 % | 10 % | 78.4 % | +2.4 pts | `re audit OpenHands/software-agent-sdk --json` |
| PrefectHQ/prefect | 18,501 | 1,696 | 465,877 | 379,892 | 81.5 % | 80.3 % | 89.2 % | 3 % | 69.0 % | +12.5 pts | `re audit PrefectHQ/prefect --json` |
| pydantic/pydantic-ai | 4,314 | 354 | 689,024 | 584,351 | 84.8 % | 76.2 % | 75.0 % | 63 % | 83.6 % | +1.2 pts | `re audit pydantic/pydantic-ai --json` |
| QwenLM/qwen-code | 10,248 | 575 | 663,103 | 546,041 | 82.3 % | 76.8 % | 53.8 % | 21 % | 72.0 % | +10.3 pts | `re audit QwenLM/qwen-code --json` |
| ray-project/ray | 32,049 | 967 | 574,407 | 363,263 | 63.2 % | 76.7 % | 89.6 % | 25 % | 67.5 % | -4.3 pts | `re audit ray-project/ray --json` |
| RooCodeInc/Roo-Code | 6,210 | 43 | 88,016 | 39,395 | 44.8 % | 57.0 % | 62.6 % | 67 % | 45.6 % | -0.9 pts | `re audit RooCodeInc/Roo-Code --json` |
| run-llama/llama_index | 7,962 | 13 | 8,542 | 7,872 | 92.2 % | 92.2 % | 90.4 % | 73 % | 81.7 % | +10.5 pts | `re audit run-llama/llama_index --json` |
| secdev/scapy | 5,738 | 66 | 8,575 | 8,197 | 95.6 % | 91.6 % | 100.0 % | 29 % | 90.1 % | -1.6 pts | `re audit secdev/scapy --json` |
| smithersai/smithers | 20,912 | 14,431 | 9,013,181 | 2,331,784 | 25.9 % | 48.7 % | 61.1 % | 37 % | 34.1 % | -8.3 pts | `re audit smithersai/smithers --json` |
| sst/opencode | 20,489 | 54 | 26,245 | 3,909 | 14.9 % | 16.3 % | 0.0 % | 42 % | 17.0 % | -4.0 pts | `re audit sst/opencode --json` |
| TracecatHQ/tracecat | 6,011 | 613 | 430,994 | 286,397 | 66.5 % | 65.0 % | 64.3 % | 4 % | 68.0 % | -1.5 pts | `re audit TracecatHQ/tracecat --json` |
| vercel/next.js | 36,086 | 258 | 67,393 | 52,099 | 77.3 % | 74.9 % | 81.9 % | 7 % | 62.4 % | +14.9 pts | `re audit vercel/next.js --json` |
| vllm-project/llm-compressor | 3,266 | 166 | 22,771 | 19,210 | 84.4 % | 76.9 % | 88.8 % | 23 % | 77.9 % | +6.5 pts | `re audit vllm-project/llm-compressor --json` |
| xing-shuyin/pi-web-ui | 1,093 | 34 | 3,052 | 2,649 | 86.8 % | 86.1 % | 94.5 % | 12 % | 72.8 % | +11.1 pts | `re audit xing-shuyin/pi-web-ui --json` |
| youknowone/pyre | 85,755 | 2,272 | 2,470,555 | 1,730,904 | 70.1 % | 67.8 % | 70.6 % | 13 % | 62.5 % | +7.6 pts | `re audit youknowone/pyre --json` |
| zed-industries/zed | 37,301 | 129 | 48,690 | 25,410 | 52.2 % | 58.5 % | 69.0 % | 16 % | 66.4 % | -14.6 pts | `re audit zed-industries/zed --json` |

## By agent, across aggregated repositories (alphabetical)

| Agent | Repositories | Commits | Introduced | Still at HEAD | Line-weighted |
|---|---:|---:|---:|---:|---:|
| ai | 2 | 59 | 2,878 | 2,538 | 88.2 % |
| aider | 4 | 11,167 | 375,563 | 235,811 | 62.8 % |
| claude-code | 51 | 25,767 | 18,065,433 | 9,351,750 | 51.8 % |
| copilot | 4 | 4,099 | 259,675 | 145,942 | 56.2 % |
| cursor | 38 | 1,312 | 4,380,918 | 823,065 | 18.8 % |
| devin | 15 | 9,283 | 3,579,893 | 2,671,398 | 74.6 % |
| gemini | 15 | 1,574 | 478,635 | 280,115 | 58.5 % |
| github-copilot | 39 | 12,599 | 6,460,988 | 4,819,844 | 74.6 % |
| grok | 1 | 330 | 465,955 | 353,710 | 75.9 % |
| grok-4.6 | 1 | 1 | 51 | 50 | 98.0 % |
| jules | 8 | 38 | 3,534 | 2,076 | 58.7 % |
| llm | 1 | 974 | 196,846 | 156,863 | 79.7 % |
| openai-codex | 23 | 4,372 | 1,186,629 | 778,574 | 65.6 % |
| opencode | 2 | 6 | 3,249 | 2,880 | 88.6 % |
| openhands | 5 | 4,588 | 1,472,152 | 719,034 | 48.8 % |
| pi | 2 | 36 | 2,941 | 2,543 | 86.5 % |

## Measured but not aggregated (fewer than 5 AI-tagged commits)

| Repository | Commits | AI-tagged | Introduced | Still at HEAD | Reproduce |
|---|---:|---:|---:|---:|---|
| openai/openai-python | 1,749 | 3 | 63 | 62 | `re audit openai/openai-python --json` |
| stackblitz/bolt.new | 101 | 0 | 0 | 0 | `re audit stackblitz/bolt.new --json` |

## Excluded from this report

- Shallow clones (history truncated; method v4 refuses them): none
- Audits that failed in this run: fern-api/fern, llvm/llvm-project, richlander/dotnet-inspect
- Opted out by their maintainers (https://github.com/croviatrust/causari/blob/main/.github/survival-optout.txt): 0

## Where this measurement connected

The witness recorded, in 20 of 20 uploaded shard logs, 1 destination(s) over 128 connection(s): 1 allowed, 0 blocked, 0 failed, under an allowlist of 1 rule(s) in enforce mode. Every destination the witness saw is one the policy allows.

| Destination | Outcome | Connections | Bytes out | Bytes in |
|---|---|---:|---:|---:|
| github.com:443 | allowed | 128 | 524,815 | 30,908,210,581 |

Signed run sheet: https://causari.dev/reports/survival/2026/06/reach.sheet.json (profile crovia.pnx.v1, witness `urn:causari:survival-report:witness`, capture `proxy-connect`, disclosure `clear`).  
Policy: https://causari.dev/reports/survival/2026/06/egress-policy.json (source https://github.com/croviatrust/causari/blob/main/.github/egress-policy.json), bound in the sheet as `sha256:7a501cbe66356070f9c6c6a5372a8dd425be059bb2315a71bffcccff64692e21`.  
Verify: `tacet-pnx verify reach.sheet.json --policy egress-policy.json` or `re pnx verify reach.sheet.json --policy egress-policy.json`, or paste the sheet and the policy at https://croviatrust.com/registry/seal/verify/.  

Covered: the connection attempts in the shard logs that were uploaded and concatenated into this sheet. Not covered: any shard that uploaded no log; what the runner does outside the measurement step (checking out this repository, installing the tool, uploading the shard); and any connection that did not go through the witness. The record says where the measurement connected through the witness and how many bytes crossed, nothing about their content.

## Method

Method v4, causari 0.4.1. Detection from commit metadata only; survival from `git blame -w -M -C` at HEAD. Per-commit cap: a commit weighs at most the 95th percentile of per-commit introduced line counts in its repository, and never more than 10,000 lines. Sample floor: 5 VERIFIED commits. VERIFIED only; PROBABLE is listed but never summed. Full clones only. Baseline (method v3): the untagged lines of the same repository, by age; the gap is defined in the Baseline section. Details, limits and how to contest a number: https://causari.dev/method.

## What this report is, and is not

This report counts lines. For each repository it states how many lines were introduced by commits that carry machine-readable AI authorship metadata (trailers such as Co-Authored-By naming an agent, bot author identities, aider markers, and, from method v4, git-ai notes that name a non-empty tool; method v3 counted any git-ai note), and how many of those lines git blame still attributes to those commits at HEAD, under the method version stated on the page (blame with -w -M -C, a per-commit weight cap, a sample floor, full clones only; from method v3 the untagged lines of the same repository, in the matched age windows, stand next to the AI-tagged ones). Every row is reproducible with one command.

It is not a quality judgement, a productivity measure, total AI usage, proof of authorship, a security finding, a correctness finding, or a causal effect of AI. Deleted lines include removed features and rewritten prototypes; surviving lines include dead code. It is not a sample of all AI-assisted code: inline completions leave no trace in git, untagged agent commits are invisible, and the repositories were selected, not drawn at random: 30 hand-picked and 34 found by GitHub commit search as public repositories with at least 5 commits carrying the same AI authorship metadata and at least 100 stars, most-starred first (discovered 2026-09-27); the selection rule and the counts behind it are public. The latest discovery run did not finish, and this list was not replaced. The intervals describe the sampled repositories only.

Prior measurement work asks related questions with different instruments. GitClear publishes churn reports built from code-change patterns across the repositories it analyses; arXiv 2601.16809 ("Will It Survive?") follows the modification of agent-authored code in 201 projects with its own detector and finds that such code is modified less often than human-written code. This report does not reproduce either method and does not adjudicate between them: it publishes counts from git metadata alone, with the method version, the tool version and the exact bytes behind every number, so that the three can be read side by side.

## Cite

Crovia Trust. Survival Report #6 (2026-10-19). https://causari.dev/reports/survival/2026/06/
