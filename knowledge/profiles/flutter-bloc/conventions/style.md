---
id: style
title: Style
description: flutter_lints, snake_case filenames, role suffixes (Page/View/Cubit/State/Repository/Impl/DataSource), package-per-feature, and const constructors.
sections:
  - { id: lints,     title: "flutter_lints",   tokens: 100, code: false }
  - { id: files,     title: "File Naming",      tokens: 100, code: false }
  - { id: suffixes,  title: "Role Suffixes",    tokens: 120, code: false }
  - { id: structure, title: "Package Structure", tokens: 100, code: false }
tags: [dart, flutter, style, flutter_lints, naming]
---

# Style

> Idiomatic Flutter the analyzer is happy with. Names carry the role.

## flutter_lints {#lints}

`analysis_options.yaml` includes `package:flutter_lints/flutter.yaml`. Treat
analyzer warnings as signal — fix them, don't ignore-comment them away. A clean
`flutter analyze` is part of "done".

## File Naming {#files}

Files are `snake_case.dart`. Typically one public class per file, named after it
(`sample_cubit.dart` → `SampleCubit`). Group by feature, then by layer
(`data/`, `domain/`, `presentation/`), then by kind (`cubit/`, `ui/`).

## Role Suffixes {#suffixes}

Class names carry their role as a suffix so kind is obvious and greppable:
`*Page` (routable screen), `*View` (sub-widget of a page), `*Cubit`, `*State`
(+ `*Initial`/`*Loading`/`*Loaded`/`*Error`), `*Repository` (abstract) and
`*RepositoryImpl` (concrete), `*DataSource` / `*DataSourceImpl`. palugada's
`fact` families key off exactly these suffixes.

## Package Structure {#structure}

One package per feature (a workspace member with its own `pubspec.yaml`),
exposing a barrel. Use `const` constructors wherever possible for widgets and
state classes to cut rebuilds. Keep cross-feature code in `libraries/shared`.
