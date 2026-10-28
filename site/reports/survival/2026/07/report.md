# Survival Report #7 — 2026-10-26

Counts of surviving lines from AI-tagged commits in 59 open-source repositories, measured with causari 0.5.0, method v6. Counts, not grades: no rank, no verdict; rows are alphabetical.

Page: https://causari.dev/reports/survival/2026/07/  
Data: https://causari.dev/reports/survival/2026/07/report.json  
Feed: https://causari.dev/reports/survival/feed.xml  
Licence: CC-BY-4.0  
DOI: https://doi.org/10.5281/zenodo.23670353  

## Aggregate

21,312,438 of 39,464,039 lines introduced by 78,879 AI-tagged commits in 59 open-source repositories are still at HEAD (54.0 %). 95 % interval over the sampled repositories: 37.4 % to 75.3 %.

- Repositories aggregated: 59
- Commits in those repositories (no merges): 937,548
- AI-tagged (VERIFIED) commits: 78,879
- Lines introduced by them: 39,464,039
- Still attributed to them at HEAD: 21,312,438
- Line-weighted ratio: 54.0 %
- 95 % bootstrap interval over the sampled repositories: 37.4 % to 75.3 %
- Median of per-repository capped ratios: 75.2 %
- 95 % bootstrap interval on that median: 70.9 % to 77.0 %

95 % percentile interval from 2000 bootstrap resamples of the 59 aggregated repositories (with replacement, seed 7). It describes the sampled repositories, not all AI-assisted code, and not the repositories not in this sample.


## Baseline: the same repositories' untagged lines, in the matched age windows

gap = AI-tagged line-weighted survival minus untagged survival re-weighted to the age mix of the AI-tagged lines of the same repository, over age windows where both cohorts hold at least 5 commits, computed inside each repository; untagged = commits with no machine-readable AI signal (human-written, inline-completed and untagged-agent code alike); age = commit date to HEAD date. A negative gap means AI-tagged lines survive less than untagged lines in the matched age windows of the same repository.

- Repositories with an age-matched gap: 58 of 59 with a baseline
- Median gap across them: +3.2 pts
- 95 % bootstrap interval on that median: -0.6 pts to +5.8 pts
- Gaps below zero: 23 · above zero: 35
- Cleared or rewritten (more than 50% of the repository's commits predate the oldest line still at HEAD): OpenHands/OpenHands. Nothing from before that date survives in them, tagged or not; their rows measure the rewrite as much as the code.

Age windows, counts summed across the repositories with a baseline, one row per age window; one large repository can dominate a window, so no gap is computed from these rows: the gap is computed inside each repository and only its median crosses repositories.

| Line age | AI-tagged commits | AI-tagged lines | Still at HEAD | AI-tagged | Untagged commits | Untagged lines | Still at HEAD | Untagged |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| 0–30 d | 15,961 | 9,180,195 | 6,447,823 | 70.2 % | 22,150 | 14,973,503 | 11,066,000 | 73.9 % |
| 30–90 d | 16,847 | 10,975,833 | 5,153,071 | 46.9 % | 43,651 | 22,513,322 | 16,057,666 | 71.3 % |
| 90–180 d | 16,551 | 10,336,298 | 4,378,400 | 42.4 % | 49,887 | 55,553,139 | 40,171,318 | 72.3 % |
| 180–365 d | 15,709 | 6,661,326 | 3,819,902 | 57.3 % | 93,746 | 25,833,487 | 12,475,034 | 48.3 % |
| 365–730 d | 10,716 | 2,240,216 | 1,480,213 | 66.1 % | 135,072 | 21,443,907 | 10,061,083 | 46.9 % |
| 730+ d | 3,095 | 70,171 | 33,029 | 47.1 % | 514,029 | 84,413,765 | 19,399,056 | 23.0 % |

## Repositories (alphabetical)

| Repository | Commits | AI-tagged | Introduced | Still at HEAD | Line-weighted | Capped | Median per commit | Largest commit | Untagged, matched windows | Gap | Reproduce |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|
| Aider-AI/aider | 12,461 | 11,156 | 374,687 | 234,788 | 62.7 % | 41.6 % | 32.4 % | 27 % | 52.7 % | +10.0 pts | `re audit Aider-AI/aider --json` |
| airbytehq/airbyte | 55,089 | 2,735 | 1,200,955 | 1,020,840 | 85.0 % | 76.7 % | 86.4 % | 41 % | 65.7 % | +19.3 pts | `re audit airbytehq/airbyte --json` |
| anthropics/anthropic-sdk-python | 1,588 | 61 | 21,526 | 19,717 | 91.6 % | 81.7 % | 84.7 % | 65 % | 89.6 % | +2.0 pts | `re audit anthropics/anthropic-sdk-python --json` |
| anthropics/anthropic-sdk-typescript | 1,540 | 85 | 31,902 | 28,847 | 90.4 % | 84.8 % | 96.6 % | 49 % | 88.7 % | +5.1 pts | `re audit anthropics/anthropic-sdk-typescript --json` |
| anthropics/claude-code | 803 | 78 | 31,739 | 29,670 | 93.5 % | 75.6 % | 58.1 % | 69 % | 81.8 % | +11.7 pts | `re audit anthropics/claude-code --json` |
| apache/fluss | 2,333 | 56 | 21,403 | 17,001 | 79.4 % | 79.2 % | 89.0 % | 9 % | 82.0 % | -2.8 pts | `re audit apache/fluss --json` |
| apache/spark | 49,080 | 19 | 1,379 | 1,208 | 87.6 % | 87.6 % | 94.1 % | 19 % | 84.9 % | +6.1 pts | `re audit apache/spark --json` |
| apache/texera | 7,339 | 556 | 215,145 | 169,998 | 79.0 % | 82.0 % | 90.5 % | 8 % | 82.5 % | -3.5 pts | `re audit apache/texera --json` |
| BerriAI/litellm | 45,473 | 6,059 | 2,593,555 | 1,711,887 | 66.0 % | 70.9 % | 69.2 % | 3 % | 61.8 % | +4.3 pts | `re audit BerriAI/litellm --json` |
| browser-use/browser-use | 7,079 | 286 | 20,238 | 4,696 | 23.2 % | 31.0 % | 26.2 % | 19 % | 43.3 % | -20.1 pts | `re audit browser-use/browser-use --json` |
| cfug/flutter.cn | 10,588 | 149 | 68,543 | 55,321 | 80.7 % | 73.0 % | 86.0 % | 48 % | 65.4 % | +15.3 pts | `re audit cfug/flutter.cn --json` |
| cline/cline | 7,608 | 164 | 118,250 | 84,218 | 71.2 % | 71.1 % | 73.5 % | 12 % | 55.2 % | +16.0 pts | `re audit cline/cline --json` |
| cloudflare/workers-sdk | 8,738 | 211 | 92,860 | 73,707 | 79.4 % | 77.9 % | 72.0 % | 8 % | 67.8 % | +11.6 pts | `re audit cloudflare/workers-sdk --json` |
| continuedev/continue | 16,266 | 228 | 27,756 | 17,961 | 64.7 % | 63.4 % | 66.8 % | 11 % | 57.7 % | +7.0 pts | `re audit continuedev/continue --json` |
| cozystack/cozystack | 5,610 | 2,841 | 629,058 | 487,810 | 77.5 % | 77.7 % | 84.5 % | 6 % | 78.1 % | -0.6 pts | `re audit cozystack/cozystack --json` |
| crewAIInc/crewAI | 2,960 | 232 | 3,362,834 | 164,926 | 4.9 % | 77.5 % | 88.5 % | 96 % | 63.1 % | -58.2 pts | `re audit crewAIInc/crewAI --json` |
| croviatrust/causari | 301 | 14 | 6,267 | 4,809 | 76.7 % | 76.7 % | 90.3 % | 25 % | 94.2 % | -16.6 pts | `re audit croviatrust/causari --json` |
| danny-avila/LibreChat | 6,344 | 90 | 84,535 | 77,900 | 92.2 % | 89.0 % | 86.2 % | 24 % | 86.4 % | +5.8 pts | `re audit danny-avila/LibreChat --json` |
| fastrepl/anarlog | 9,920 | 1,544 | 860,632 | 550,511 | 64.0 % | 49.5 % | 15.3 % | 14 % | 54.7 % | +9.3 pts | `re audit fastrepl/anarlog --json` |
| flutter/flutter | 89,691 | 158 | 86,630 | 18,228 | 21.0 % | 65.2 % | 83.2 % | 38 % | 39.8 % | -18.7 pts | `re audit flutter/flutter --json` |
| flutter/website | 8,587 | 154 | 50,374 | 43,676 | 86.7 % | 79.0 % | 90.0 % | 65 % | 86.2 % | +0.5 pts | `re audit flutter/website --json` |
| ghostty-org/ghostty | 13,816 | 97 | 4,980 | 3,600 | 72.3 % | 75.2 % | 85.3 % | 9 % | 47.6 % | +24.6 pts | `re audit ghostty-org/ghostty --json` |
| github/gh-aw-mcpg | 5,919 | 5,434 | 460,365 | 249,019 | 54.1 % | 61.2 % | 69.6 % | 3 % | 45.4 % | +9.2 pts | `re audit github/gh-aw-mcpg --json` |
| google-gemini/gemini-cli | 6,532 | 351 | 74,458 | 47,564 | 63.9 % | 62.5 % | 64.5 % | 4 % | 75.3 % | -12.2 pts | `re audit google-gemini/gemini-cli --json` |
| google/sam | 1,924 | 40 | 261 | 100 | 38.3 % | 47.8 % | 0.0 % | 67 % | 49.1 % | -10.8 pts | `re audit google/sam --json` |
| GoogleCloudPlatform/scion | 6,664 | 86 | 24,153 | 19,862 | 82.2 % | 74.5 % | 80.6 % | 35 % | 81.7 % | +0.6 pts | `re audit GoogleCloudPlatform/scion --json` |
| jdubois/boot-ui | 2,110 | 1,645 | 1,075,091 | 771,713 | 71.8 % | 79.9 % | 82.9 % | 11 % | 75.8 % | -4.0 pts | `re audit jdubois/boot-ui --json` |
| langchain-ai/langchain | 16,962 | 43 | 7,289 | 5,386 | 73.9 % | 69.8 % | 87.4 % | 24 % | 58.5 % | +14.9 pts | `re audit langchain-ai/langchain --json` |
| langgenius/dify | 14,488 | 670 | 1,042,259 | 637,595 | 61.2 % | 48.5 % | 55.0 % | 31 % | 46.0 % | +15.2 pts | `re audit langgenius/dify --json` |
| lidge-ai/cli-jaw | 5,721 | 1,361 | 445,819 | 196,423 | 44.1 % | 75.0 % | 68.0 % | 47 % | 55.7 % | -11.6 pts | `re audit lidge-ai/cli-jaw --json` |
| lidge-ai/ima2-gen | 2,070 | 455 | 127,661 | 46,599 | 36.5 % | 52.7 % | 60.0 % | 19 % | 63.4 % | -26.9 pts | `re audit lidge-ai/ima2-gen --json` |
| lobehub/lobe-chat | 15,049 | 2,468 | 3,128,049 | 2,674,179 | 85.5 % | 77.4 % | 81.4 % | 1 % | 82.4 % | +3.1 pts | `re audit lobehub/lobe-chat --json` |
| managarm/managarm | 6,701 | 891 | 65,212 | 50,539 | 77.5 % | 77.5 % | 88.0 % | 5 % | 72.9 % | +4.6 pts | `re audit managarm/managarm --json` |
| mem0ai/mem0 | 2,649 | 95 | 37,062 | 23,880 | 64.4 % | 64.6 % | 83.0 % | 9 % | 74.8 % | -11.1 pts | `re audit mem0ai/mem0 --json` |
| microsoft/BCApps | 4,455 | 797 | 480,678 | 402,709 | 83.8 % | 79.8 % | 85.2 % | 22 % | 86.6 % | -3.0 pts | `re audit microsoft/BCApps --json` |
| microsoft/vscode | 150,999 | 8,571 | 3,372,776 | 2,685,244 | 79.6 % | 73.6 % | 77.0 % | 26 % | 65.6 % | +14.1 pts | `re audit microsoft/vscode --json` |
| microsoft/vscode-copilot-chat | 3,713 | 336 | 995,871 | 938,654 | 94.3 % | 69.0 % | 76.3 % | 88 % | 55.7 % | +38.6 pts | `re audit microsoft/vscode-copilot-chat --json` |
| NovaSky-AI/SkyRL | 1,398 | 444 | 155,748 | 125,267 | 80.4 % | 78.5 % | 82.7 % | 4 % | 66.0 % | +14.4 pts | `re audit NovaSky-AI/SkyRL --json` |
| NVIDIA-NeMo/Switchyard | 619 | 28 | 15,791 | 10,535 | 66.7 % | 67.4 % | 77.1 % | 20 % | 82.1 % | -15.2 pts | `re audit NVIDIA-NeMo/Switchyard --json` |
| openai/codex | 12,591 | 364 | 183,908 | 107,867 | 58.7 % | 58.2 % | 57.4 % | 6 % | 50.6 % | +8.1 pts | `re audit openai/codex --json` |
| openai/openai-agents-python | 2,402 | 6 | 97,483 | 88,529 | 90.8 % | 89.6 % | 86.7 % | 97 % | no shared window | — | `re audit openai/openai-agents-python --json` |
| OpenHands/extensions | 430 | 336 | 105,749 | 86,577 | 81.9 % | 79.6 % | 84.7 % | 11 % | 79.0 % | +2.8 pts | `re audit OpenHands/extensions --json` |
| OpenHands/OpenHands · rewritten | 8,751 | 2,829 | 1,065,685 | 383,041 | 35.9 % | 31.8 % | 0.0 % | 13 % | 32.7 % | +3.3 pts | `re audit OpenHands/OpenHands --json` |
| OpenHands/software-agent-sdk | 2,721 | 1,864 | 570,924 | 463,315 | 81.2 % | 77.0 % | 78.7 % | 10 % | 79.1 % | +2.0 pts | `re audit OpenHands/software-agent-sdk --json` |
| PrefectHQ/prefect | 18,606 | 1,766 | 478,691 | 388,841 | 81.2 % | 80.0 % | 89.2 % | 3 % | 76.7 % | +4.6 pts | `re audit PrefectHQ/prefect --json` |
| pydantic/pydantic-ai | 4,709 | 378 | 715,347 | 535,359 | 74.8 % | 76.3 % | 75.0 % | 61 % | 83.1 % | -8.2 pts | `re audit pydantic/pydantic-ai --json` |
| QwenLM/qwen-code | 10,726 | 577 | 664,884 | 542,986 | 81.7 % | 76.2 % | 54.2 % | 21 % | 71.7 % | +10.0 pts | `re audit QwenLM/qwen-code --json` |
| ray-project/ray | 32,146 | 1,021 | 628,678 | 402,907 | 64.1 % | 77.0 % | 89.5 % | 23 % | 67.4 % | -3.3 pts | `re audit ray-project/ray --json` |
| RooCodeInc/Roo-Code | 6,210 | 43 | 88,016 | 39,395 | 44.8 % | 57.0 % | 62.6 % | 67 % | 45.3 % | -0.6 pts | `re audit RooCodeInc/Roo-Code --json` |
| run-llama/llama_index | 7,962 | 13 | 8,542 | 7,872 | 92.2 % | 92.2 % | 90.4 % | 73 % | 81.7 % | +10.5 pts | `re audit run-llama/llama_index --json` |
| secdev/scapy | 5,749 | 69 | 8,841 | 8,409 | 95.1 % | 90.9 % | 100.0 % | 28 % | 89.8 % | -2.1 pts | `re audit secdev/scapy --json` |
| smithersai/smithers | 22,001 | 15,220 | 10,043,379 | 2,207,308 | 22.0 % | 30.0 % | 0.0 % | 33 % | 24.2 % | -2.2 pts | `re audit smithersai/smithers --json` |
| sst/opencode | 21,268 | 55 | 26,288 | 3,890 | 14.8 % | 16.3 % | 0.0 % | 42 % | 16.2 % | -3.1 pts | `re audit sst/opencode --json` |
| TracecatHQ/tracecat | 6,102 | 664 | 474,452 | 319,330 | 67.3 % | 66.0 % | 66.5 % | 4 % | 69.8 % | -2.5 pts | `re audit TracecatHQ/tracecat --json` |
| vercel/next.js | 36,220 | 266 | 68,326 | 52,780 | 77.2 % | 74.6 % | 81.9 % | 7 % | 60.9 % | +16.4 pts | `re audit vercel/next.js --json` |
| vllm-project/llm-compressor | 3,299 | 171 | 25,754 | 21,840 | 84.8 % | 77.0 % | 87.2 % | 20 % | 79.0 % | +5.8 pts | `re audit vllm-project/llm-compressor --json` |
| xing-shuyin/pi-web-ui | 1,180 | 34 | 3,052 | 2,645 | 86.7 % | 86.0 % | 92.8 % | 12 % | 73.4 % | +10.4 pts | `re audit xing-shuyin/pi-web-ui --json` |
| youknowone/pyre | 85,865 | 2,378 | 2,740,233 | 1,914,718 | 69.9 % | 67.3 % | 70.1 % | 12 % | 65.5 % | +4.4 pts | `re audit youknowone/pyre --json` |
| zed-industries/zed | 37,423 | 137 | 52,016 | 28,242 | 54.3 % | 60.1 % | 70.6 % | 15 % | 66.6 % | -12.3 pts | `re audit zed-industries/zed --json` |

## By agent, across aggregated repositories (alphabetical)

| Agent | Repositories | Commits | Introduced | Still at HEAD | Line-weighted |
|---|---:|---:|---:|---:|---:|
| ai | 2 | 61 | 2,907 | 2,565 | 88.2 % |
| aider | 4 | 11,167 | 375,563 | 234,817 | 62.5 % |
| claude-code | 52 | 26,801 | 19,701,004 | 9,947,982 | 50.5 % |
| copilot | 4 | 4,149 | 261,595 | 147,421 | 56.4 % |
| cursor | 38 | 1,324 | 4,393,962 | 825,943 | 18.8 % |
| devin | 15 | 9,614 | 3,716,801 | 2,776,576 | 74.7 % |
| gemini | 15 | 1,582 | 481,650 | 273,656 | 56.8 % |
| github-copilot | 39 | 13,365 | 6,827,101 | 5,104,289 | 74.8 % |
| grok | 1 | 389 | 576,498 | 421,665 | 73.1 % |
| grok-4.6 | 1 | 1 | 51 | 50 | 98.0 % |
| jules | 8 | 38 | 3,534 | 2,076 | 58.7 % |
| llm | 1 | 1,025 | 201,405 | 159,346 | 79.1 % |
| openai-codex | 26 | 4,651 | 1,401,725 | 656,227 | 46.8 % |
| opencode | 2 | 6 | 3,249 | 2,880 | 88.6 % |
| openhands | 5 | 4,669 | 1,513,929 | 754,281 | 49.8 % |
| pi | 2 | 37 | 3,065 | 2,664 | 86.9 % |

## Measured but not aggregated (fewer than 5 AI-tagged commits)

| Repository | Commits | AI-tagged | Introduced | Still at HEAD | Reproduce |
|---|---:|---:|---:|---:|---|
| openai/openai-python | 1,796 | 3 | 63 | 62 | `re audit openai/openai-python --json` |
| stackblitz/bolt.new | 101 | 0 | 0 | 0 | `re audit stackblitz/bolt.new --json` |

## Excluded from this report

- Shallow clones (history truncated; method v6 refuses them): none
- Audits that failed in this run: fern-api/fern, llvm/llvm-project, richlander/dotnet-inspect
- Opted out by their maintainers (https://github.com/croviatrust/causari/blob/main/.github/survival-optout.txt): 0

## Where this measurement connected

The witness recorded, in 20 of 20 uploaded shard logs, 1 destination(s) over 127 connection(s): 1 allowed, 0 blocked, 0 failed, under an allowlist of 1 rule(s) in enforce mode. Every destination the witness saw is one the policy allows.

| Destination | Outcome | Connections | Bytes out | Bytes in |
|---|---|---:|---:|---:|
| github.com:443 | allowed | 127 | 442,983 | 30,919,020,650 |

Signed run sheet: https://causari.dev/reports/survival/2026/07/reach.sheet.json (profile crovia.pnx.v1, witness `urn:causari:survival-report:witness`, capture `proxy-connect`, disclosure `clear`).  
Policy: https://causari.dev/reports/survival/2026/07/egress-policy.json (source https://github.com/croviatrust/causari/blob/main/.github/egress-policy.json), bound in the sheet as `sha256:7a501cbe66356070f9c6c6a5372a8dd425be059bb2315a71bffcccff64692e21`.  
Verify: `tacet-pnx verify reach.sheet.json --policy egress-policy.json` or `re pnx verify reach.sheet.json --policy egress-policy.json`, or paste the sheet and the policy at https://croviatrust.com/registry/seal/verify/.  

Covered: the connection attempts in the shard logs that were uploaded and concatenated into this sheet. Not covered: any shard that uploaded no log; what the runner does outside the measurement step (checking out this repository, installing the tool, uploading the shard); and any connection that did not go through the witness. The record says where the measurement connected through the witness and how many bytes crossed, nothing about their content.

## Method

Method v6, causari 0.5.0. Detection from commit metadata only; survival from `git blame -w -M -C` at HEAD. Per-commit cap: a commit weighs at most the 95th percentile of per-commit introduced line counts in its repository, and never more than 10,000 lines. Sample floor: 5 VERIFIED commits. VERIFIED only; PROBABLE is listed but never summed. Full clones only. Baseline (method v3): the untagged lines of the same repository, by age; the gap is defined in the Baseline section. Details, limits and how to contest a number: https://causari.dev/method.

## What this report is, and is not

This report counts lines. For each repository it states how many lines were introduced by commits that carry machine-readable AI authorship metadata (trailers such as Co-Authored-By naming an agent, bot author identities, aider markers, and, from method v4, git-ai notes that name a non-empty tool; method v3 counted any git-ai note), and how many of those lines git blame still attributes to those commits at HEAD, under the method version stated on the page (blame with -w -M -C, a per-commit weight cap, a sample floor, full clones only; from method v3 the untagged lines of the same repository, in the matched age windows, stand next to the AI-tagged ones). Every row is reproducible with one command.

It is not a quality judgement, a productivity measure, total AI usage, proof of authorship, a security finding, a correctness finding, or a causal effect of AI. Deleted lines include removed features and rewritten prototypes; surviving lines include dead code. It is not a sample of all AI-assisted code: inline completions leave no trace in git, untagged agent commits are invisible, and the repositories were selected, not drawn at random: 30 hand-picked and 34 found by GitHub commit search as public repositories with at least 5 commits carrying the same AI authorship metadata and at least 100 stars, most-starred first (discovered 2026-09-27); the selection rule and the counts behind it are public. The latest discovery run did not finish, and this list was not replaced. The intervals describe the sampled repositories only.

Prior measurement work asks related questions with different instruments. GitClear publishes churn reports built from code-change patterns across the repositories it analyses; arXiv 2601.16809 ("Will It Survive?") follows the modification of agent-authored code in 201 projects with its own detector and finds that such code is modified less often than human-written code. This report does not reproduce either method and does not adjudicate between them: it publishes counts from git metadata alone, with the method version, the tool version and the exact bytes behind every number, so that the three can be read side by side.

## Cite

Crovia Trust. Survival Report #7 (2026-10-26). https://causari.dev/reports/survival/2026/07/ DOI 10.5281/zenodo.23670353
