---
name: palugada-bugfix
description: TRIGGER when fixing a bug, crash, or regression. Gather a context pack with palugada before editing.
allowed-tools: Bash(palugada *), Read, Grep, Glob, Write, Edit
---

# Bugfix

When you fix a bug, get ONE budgeted context pack first:

    palugada brief bugfix <target>     # recent changes + symbols + the relevant conventions

Then pull only the rules you need (don't guess — the knowledge lives in the profile):

    palugada for <task>                # a recipe; `palugada for --list` to see all
    palugada q <topic>                 # a convention; `palugada q --list` to see all

Locate code with the `palugada-search` skill — never blind-grep.
