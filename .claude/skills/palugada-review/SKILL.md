---
name: palugada-review
description: TRIGGER when reviewing a diff, pull request, or merge request. Gather a context pack with palugada before editing.
allowed-tools: Bash(palugada *), Read, Grep, Glob, Write, Edit
---

# Review

When you review changes, get ONE budgeted context pack first:

    palugada brief review <target>     # recent changes + symbols + the relevant conventions

Then pull only the rules you need (don't guess — the knowledge lives in the profile):

    palugada for <task>                # a recipe; `palugada for --list` to see all
    palugada q <topic>                 # a convention; `palugada q --list` to see all

This flow is diff-scoped — point it at a ref: `palugada brief review <ref>`.
