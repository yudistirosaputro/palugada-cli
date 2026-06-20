---
name: palugada-git
description: >
  TRIGGER for git work — branch, commit, push, rebase/merge conflict, pull/merge
  request, pipeline. Also when the user mentions gh, glab, GitHub, or GitLab.
allowed-tools: Bash(palugada *), Bash(git *), Bash(gh *), Bash(glab *), Read, Grep, Glob, Write, Edit
---

# Git & PR/MR

    palugada git whoami           # confirm the authenticated git-host user
    palugada pr recent <file>     # recent commits touching a file (host reverse-index)

## Commits & branches

    type(scope): lowercase summary      e.g. feat(watchlist): add sort
    type/TICKET-short-description        e.g. feat/UATP-1602-watchlist-sort

## PR / MR

Use `gh` (GitHub) or `glab` (GitLab) to create / list / review / merge.

## Safety

- Never `git push --force` — use `--force-with-lease`.
- Only rebase YOUR feature branch, never a shared one.
- Resolve conflicts per-hunk; build + test after; `git rebase --abort` is safe if unsure.
