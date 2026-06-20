---
name: palugada-docs
description: TRIGGER for tickets, issues, wiki/Confluence/Notion pages, PRDs, or specs.
allowed-tools: Bash(palugada *), Read
---

# Issues, wiki & PRDs

    palugada issue view <KEY>     # a ticket (Jira / GitHub Issues)
    palugada wiki page <ID>       # a wiki/doc page (Confluence / Notion)
    palugada prd fetch <KEY>      # save a ticket into the personal corpus
    palugada prd list             # list saved corpus docs
    palugada prd cat <name>       # read one
    palugada prd search <kw>      # search the corpus offline
