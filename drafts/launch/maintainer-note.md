# Note to a maintainer whose repository appears in the Survival Report

Send once, by e-mail or as a single issue, only after the report that lists
the repository is published. Replace every `{…}`. Do not ask for a star, a
retweet or a badge. If the maintainer answers "please remove us", do it the
same day and reply with the commit.

---

Subject: {owner/repo} is measured in the Causari Survival Report — how, and how to opt out

Hello {name},

I maintain Causari, a small open-source tool that counts, from git history alone, how many lines introduced by AI-tagged commits are still at HEAD. Your repository {owner/repo} is one of the {N} open-source repositories in Survival Report #{K} ({date}):

{link to the report page}

What was measured, for {owner/repo} at commit {short sha}:

- Commits carrying machine-readable AI authorship metadata (Co-Authored-By trailers naming an agent, bot authors, AI-* trailers, git-ai notes): {AI-tagged commits}
- Lines those commits introduced: {introduced}
- Lines `git blame -w -M -C` still attributes to them at HEAD: {still at HEAD}

Nothing else: no quality judgement, no rank, no colour. Rows are alphabetical. The report says what the count cannot see — inline completions leave no git trace, untagged commits are never counted, a rewritten line is a death even when the meaning is unchanged — and the same limits are written at https://causari.dev/method.

You can reproduce the row with one command; the JSON behind it is linked from the report:

    curl -fsSL https://causari.dev/install.sh | sh
    re audit {owner/repo} --json

If your numbers differ, or a commit was misclassified, please open an issue with the JSON and I will correct the row in public: https://github.com/croviatrust/causari/issues

If you would rather not be measured at all, add one line with your repository name to

    .github/survival-optout.txt

in https://github.com/croviatrust/causari (a pull request, or reply to this message and I will do it). The next weekly run drops the row and it is never re-added. No questions asked.

Thank you for your work on {owner/repo}.

{maintainer name}
Causari, https://causari.dev · Apache-2.0
