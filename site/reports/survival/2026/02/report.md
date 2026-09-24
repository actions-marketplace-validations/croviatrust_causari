# Survival Report #2 — 2026-09-23 (revision 2)

Counts of surviving lines from AI-tagged commits in 54 open-source repositories, measured with causari 0.2.0, method v2. Counts, not grades: no rank, no verdict; rows are alphabetical.

Page: https://causari.dev/reports/survival/2026/02/  
Data: https://causari.dev/reports/survival/2026/02/report.json  
Feed: https://causari.dev/reports/survival/feed.xml  
Licence: CC-BY-4.0  
DOI: https://doi.org/10.5281/zenodo.22944117  

## Corrections

- Revision 2 (2026-09-24): All-Hands-AI/OpenHands and OpenHands/OpenHands are one repository, renamed on GitHub; revision 1 counted its audit twice (the two files under repos/ are byte-identical, SHA-256 1500353f…). It now counts once, under OpenHands/OpenHands. Revision 1 counted 55 repositories, 14,015,893 of 28,046,116 lines (50.0 %); its bytes are kept unchanged at report.r1.json, DOI 10.5281/zenodo.22928161.

## Aggregate

13,733,809 of 27,108,452 lines introduced by 58,061 AI-tagged commits in 54 open-source repositories are still at HEAD (50.7 %). 95 % interval over the sampled repositories: 32.8 % to 77.2 %.

- Repositories aggregated: 54
- Commits in those repositories (no merges): 796,760
- AI-tagged (VERIFIED) commits: 58,061
- Lines introduced by them: 27,108,452
- Still attributed to them at HEAD: 13,733,809
- Line-weighted ratio: 50.7 %
- 95 % bootstrap interval over the sampled repositories: 32.8 % to 77.2 %
- Median of per-repository capped ratios: 74.9 %
- 95 % bootstrap interval on that median: 69.4 % to 77.3 %

95 % percentile interval from 2000 bootstrap resamples of the 54 aggregated repositories (with replacement, seed 2). It describes the sampled repositories, not all AI-assisted code, and not the repositories not in this sample.

## Repositories (alphabetical)

| Repository | Commits | AI-tagged | Introduced | Still at HEAD | Line-weighted | Capped | Median per commit | Largest commit | Reproduce |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---|
| Aider-AI/aider | 12,461 | 11,156 | 374,687 | 235,782 | 62.9 % | 41.9 % | 33.3 % | 27 % | `re audit Aider-AI/aider --json` |
| airbytehq/airbyte | 52,899 | 2,199 | 1,058,017 | 906,457 | 85.7 % | 75.2 % | 86.0 % | 47 % | `re audit airbytehq/airbyte --json` |
| anthropics/anthropic-sdk-python | 1,408 | 39 | 4,961 | 4,116 | 83.0 % | 79.7 % | 83.3 % | 32 % | `re audit anthropics/anthropic-sdk-python --json` |
| anthropics/anthropic-sdk-typescript | 1,334 | 50 | 14,219 | 11,877 | 83.5 % | 85.6 % | 96.4 % | 39 % | `re audit anthropics/anthropic-sdk-typescript --json` |
| anthropics/claude-code | 727 | 71 | 31,257 | 29,225 | 93.5 % | 74.6 % | 47.2 % | 70 % | `re audit anthropics/claude-code --json` |
| apache/fluss | 2,184 | 34 | 12,656 | 10,135 | 80.1 % | 79.9 % | 95.1 % | 12 % | `re audit apache/fluss --json` |
| apache/spark | 48,648 | 21 | 1,634 | 1,399 | 85.6 % | 85.9 % | 94.1 % | 16 % | `re audit apache/spark --json` |
| apache/texera | 7,170 | 516 | 196,116 | 156,059 | 79.6 % | 83.0 % | 91.2 % | 9 % | `re audit apache/texera --json` |
| BerriAI/litellm | 43,225 | 4,706 | 1,341,172 | 821,281 | 61.2 % | 66.8 % | 68.2 % | 6 % | `re audit BerriAI/litellm --json` |
| browser-use/browser-use | 7,000 | 262 | 19,773 | 4,287 | 21.7 % | 28.9 % | 15.5 % | 19 % | `re audit browser-use/browser-use --json` |
| cfug/flutter.cn | 10,588 | 149 | 68,543 | 59,214 | 86.4 % | 75.1 % | 87.9 % | 48 % | `re audit cfug/flutter.cn --json` |
| cline/cline | 7,311 | 155 | 95,924 | 63,615 | 66.3 % | 68.0 % | 72.7 % | 12 % | `re audit cline/cline --json` |
| cloudflare/workers-sdk | 8,419 | 193 | 88,560 | 70,036 | 79.1 % | 77.3 % | 72.1 % | 8 % | `re audit cloudflare/workers-sdk --json` |
| continuedev/continue | 16,266 | 228 | 27,756 | 18,063 | 65.1 % | 63.5 % | 68.1 % | 11 % | `re audit continuedev/continue --json` |
| cozystack/cozystack | 4,828 | 2,122 | 498,651 | 380,634 | 76.3 % | 76.9 % | 84.6 % | 7 % | `re audit cozystack/cozystack --json` |
| crewAIInc/crewAI | 2,811 | 192 | 3,342,690 | 148,599 | 4.4 % | 75.7 % | 90.0 % | 97 % | `re audit crewAIInc/crewAI --json` |
| croviatrust/causari | 205 | 14 | 6,267 | 5,235 | 83.5 % | 83.5 % | 93.1 % | 25 % | `re audit croviatrust/causari --json` |
| danny-avila/LibreChat | 5,623 | 88 | 84,488 | 78,463 | 92.9 % | 90.4 % | 88.2 % | 24 % | `re audit danny-avila/LibreChat --json` |
| fastrepl/anarlog | 9,534 | 1,356 | 706,569 | 434,992 | 61.6 % | 41.4 % | 6.7 % | 17 % | `re audit fastrepl/anarlog --json` |
| flutter/flutter | 89,131 | 149 | 83,109 | 15,790 | 19.0 % | 65.8 % | 87.0 % | 40 % | `re audit flutter/flutter --json` |
| flutter/website | 8,538 | 148 | 49,776 | 47,362 | 95.2 % | 82.0 % | 94.8 % | 66 % | `re audit flutter/website --json` |
| ghostty-org/ghostty | 13,469 | 77 | 3,725 | 2,570 | 69.0 % | 73.4 % | 86.0 % | 13 % | `re audit ghostty-org/ghostty --json` |
| github/gh-aw-mcpg | 5,527 | 5,079 | 431,348 | 244,733 | 56.7 % | 63.3 % | 70.2 % | 3 % | `re audit github/gh-aw-mcpg --json` |
| google-gemini/gemini-cli | 6,429 | 348 | 71,636 | 45,146 | 63.0 % | 61.8 % | 64.1 % | 4 % | `re audit google-gemini/gemini-cli --json` |
| GoogleCloudPlatform/scion | 4,396 | 80 | 13,359 | 10,002 | 74.9 % | 75.8 % | 83.8 % | 18 % | `re audit GoogleCloudPlatform/scion --json` |
| jdubois/boot-ui | 1,384 | 1,015 | 517,095 | 417,286 | 80.7 % | 80.3 % | 82.3 % | 10 % | `re audit jdubois/boot-ui --json` |
| langchain-ai/langchain | 16,732 | 43 | 7,289 | 5,388 | 73.9 % | 69.8 % | 87.4 % | 24 % | `re audit langchain-ai/langchain --json` |
| langgenius/dify | 13,605 | 566 | 644,347 | 374,132 | 58.1 % | 58.3 % | 59.5 % | 16 % | `re audit langgenius/dify --json` |
| lobehub/lobe-chat | 13,763 | 1,899 | 2,119,123 | 1,826,501 | 86.2 % | 79.0 % | 81.6 % | 2 % | `re audit lobehub/lobe-chat --json` |
| managarm/managarm | 6,363 | 595 | 47,375 | 36,929 | 78.0 % | 78.0 % | 89.8 % | 6 % | `re audit managarm/managarm --json` |
| mem0ai/mem0 | 2,626 | 93 | 37,012 | 23,929 | 64.7 % | 64.8 % | 82.8 % | 9 % | `re audit mem0ai/mem0 --json` |
| microsoft/BCApps | 4,077 | 617 | 354,085 | 309,899 | 87.5 % | 82.0 % | 86.8 % | 30 % | `re audit microsoft/BCApps --json` |
| microsoft/vscode | 148,188 | 6,364 | 2,404,416 | 1,939,808 | 80.7 % | 71.6 % | 75.0 % | 36 % | `re audit microsoft/vscode --json` |
| microsoft/vscode-copilot-chat | 3,713 | 336 | 995,871 | 939,159 | 94.3 % | 69.0 % | 76.3 % | 88 % | `re audit microsoft/vscode-copilot-chat --json` |
| NovaSky-AI/SkyRL | 1,281 | 356 | 111,221 | 84,605 | 76.1 % | 72.0 % | 78.4 % | 5 % | `re audit NovaSky-AI/SkyRL --json` |
| NVIDIA-NeMo/Switchyard | 489 | 24 | 14,294 | 9,274 | 64.9 % | 65.6 % | 80.6 % | 22 % | `re audit NVIDIA-NeMo/Switchyard --json` |
| openai/codex | 11,292 | 364 | 183,908 | 109,637 | 59.6 % | 59.1 % | 58.6 % | 6 % | `re audit openai/codex --json` |
| OpenHands/extensions | 349 | 281 | 76,107 | 59,822 | 78.6 % | 78.0 % | 85.8 % | 4 % | `re audit OpenHands/extensions --json` |
| OpenHands/OpenHands | 8,305 | 2,632 | 937,664 | 282,084 | 30.1 % | 28.2 % | 0.0 % | 14 % | `re audit OpenHands/OpenHands --json` |
| OpenHands/software-agent-sdk | 2,403 | 1,663 | 468,435 | 371,146 | 79.2 % | 76.1 % | 77.6 % | 12 % | `re audit OpenHands/software-agent-sdk --json` |
| PrefectHQ/prefect | 18,408 | 1,651 | 457,040 | 373,178 | 81.7 % | 80.5 % | 89.7 % | 3 % | `re audit PrefectHQ/prefect --json` |
| pydantic/pydantic-ai | 3,115 | 264 | 618,017 | 541,400 | 87.6 % | 77.0 % | 78.2 % | 70 % | `re audit pydantic/pydantic-ai --json` |
| QwenLM/qwen-code | 9,239 | 555 | 597,734 | 513,051 | 85.8 % | 81.3 % | 65.5 % | 23 % | `re audit QwenLM/qwen-code --json` |
| ray-project/ray | 31,731 | 797 | 481,400 | 311,207 | 64.6 % | 79.0 % | 90.7 % | 30 % | `re audit ray-project/ray --json` |
| RooCodeInc/Roo-Code | 6,210 | 43 | 88,016 | 39,395 | 44.8 % | 57.0 % | 62.6 % | 67 % | `re audit RooCodeInc/Roo-Code --json` |
| run-llama/llama_index | 7,942 | 13 | 8,542 | 7,872 | 92.2 % | 92.2 % | 90.4 % | 73 % | `re audit run-llama/llama_index --json` |
| secdev/scapy | 5,715 | 61 | 8,472 | 8,217 | 97.0 % | 94.2 % | 100.0 % | 29 % | `re audit secdev/scapy --json` |
| smithersai/smithers | 11,647 | 7,302 | 6,805,290 | 1,016,597 | 14.9 % | 31.9 % | 0.0 % | 49 % | `re audit smithersai/smithers --json` |
| sst/opencode | 15,705 | 50 | 25,998 | 9,470 | 36.4 % | 35.4 % | 39.5 % | 43 % | `re audit sst/opencode --json` |
| TracecatHQ/tracecat | 5,823 | 520 | 347,883 | 233,664 | 67.2 % | 65.7 % | 64.6 % | 4 % | `re audit TracecatHQ/tracecat --json` |
| vercel/next.js | 35,593 | 224 | 58,025 | 44,762 | 77.1 % | 74.7 % | 82.7 % | 8 % | `re audit vercel/next.js --json` |
| vllm-project/llm-compressor | 3,227 | 154 | 17,210 | 13,838 | 80.4 % | 74.7 % | 87.2 % | 30 % | `re audit vllm-project/llm-compressor --json` |
| xing-shuyin/pi-web-ui | 760 | 30 | 2,298 | 2,025 | 88.1 % | 88.9 % | 94.8 % | 9 % | `re audit xing-shuyin/pi-web-ui --json` |
| zed-industries/zed | 36,944 | 117 | 47,392 | 24,462 | 51.6 % | 55.4 % | 69.3 % | 16 % | `re audit zed-industries/zed --json` |

## By agent, across aggregated repositories (alphabetical)

| Agent | Repositories | Commits | Introduced | Still at HEAD | Line-weighted |
|---|---:|---:|---:|---:|---:|
| ai | 4 | 59 | 8,280 | 7,152 | 86.4 % |
| aider | 4 | 11,167 | 375,563 | 235,811 | 62.8 % |
| claude-code | 46 | 16,448 | 12,658,706 | 5,774,404 | 45.6 % |
| copilot | 4 | 3,927 | 250,780 | 147,792 | 58.9 % |
| cursor | 34 | 1,170 | 4,287,008 | 751,155 | 17.5 % |
| devin | 12 | 7,415 | 2,166,047 | 1,615,028 | 74.6 % |
| gemini | 13 | 1,511 | 470,396 | 292,713 | 62.2 % |
| github-copilot | 36 | 9,960 | 4,878,197 | 3,916,188 | 80.3 % |
| jules | 7 | 36 | 3,357 | 2,047 | 61.0 % |
| llm | 1 | 307 | 71,040 | 50,788 | 71.5 % |
| openai-codex | 17 | 1,662 | 556,459 | 292,140 | 52.5 % |
| opencode | 2 | 6 | 3,249 | 2,893 | 89.0 % |
| openhands | 5 | 4,361 | 1,377,031 | 643,632 | 46.7 % |
| pi | 2 | 32 | 2,339 | 2,066 | 88.3 % |

## Measured but not aggregated (fewer than 5 AI-tagged commits)

| Repository | Commits | AI-tagged | Introduced | Still at HEAD | Reproduce |
|---|---:|---:|---:|---:|---|
| openai/openai-agents-python | 2,230 | 2 | 94,176 | 94,152 | `re audit openai/openai-agents-python --json` |
| openai/openai-python | 1,628 | 3 | 63 | 61 | `re audit openai/openai-python --json` |
| stackblitz/bolt.new | 101 | 0 | 0 | 0 | `re audit stackblitz/bolt.new --json` |

## Excluded from this report

- Shallow clones (history truncated; method v2 refuses them): none
- Audits that failed in this run: fern-api/fern, richlander/dotnet-inspect
- All-Hands-AI/OpenHands is the same repository as OpenHands/OpenHands (byte-identical audit output); counted once, under OpenHands/OpenHands
- Opted out by their maintainers (https://github.com/croviatrust/causari/blob/main/.github/survival-optout.txt): 0

## Method

Method v2, causari 0.2.0. Detection from commit metadata only; survival from `git blame -w -M -C` at HEAD. Per-commit cap: a commit weighs at most the 95th percentile of per-commit introduced line counts in its repository, and never more than 10,000 lines. Sample floor: 5 VERIFIED commits. VERIFIED only; PROBABLE is listed but never summed. Full clones only. Details, limits and how to contest a number: https://causari.dev/method.

## What this report is, and is not

This report counts lines. For each repository it states how many lines were introduced by commits that carry machine-readable AI authorship metadata (trailers such as Co-Authored-By naming an agent, bot author identities, aider markers, git-ai notes), and how many of those lines git blame still attributes to those commits at HEAD, under method v2 (blame with -w -M -C, a per-commit weight cap, a sample floor, full clones only). Every row is reproducible with one command.

It is not a quality judgement: deleted lines include removed features and rewritten prototypes; surviving lines include dead code. It is not a sample of all AI-assisted code: inline completions leave no trace in git, untagged agent commits are invisible, and the repositories were selected, not drawn at random: 30 hand-picked and 30 found by GitHub commit search as public repositories with at least 5 commits carrying the same AI authorship metadata and at least 100 stars, most-starred first (discovered 2026-09-23); the selection rule and the counts behind it are public. The intervals describe the sampled repositories only.

Prior measurement work asks related questions with different instruments. GitClear publishes churn reports built from code-change patterns across the repositories it analyses; arXiv 2601.16809 ("Will It Survive?") follows the modification of agent-authored code in 201 projects with its own detector and finds that such code is modified less often than human-written code. This report does not reproduce either method and does not adjudicate between them: it publishes counts from git metadata alone, with the method version, the tool version and the exact bytes behind every number, so that the three can be read side by side.

## Cite

Crovia Trust. Survival Report #2 (2026-09-23, revision 2). https://causari.dev/reports/survival/2026/02/ DOI 10.5281/zenodo.22944117
