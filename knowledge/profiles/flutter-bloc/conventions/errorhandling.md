---
id: errorhandling
title: Error Handling
description: Catch in the Cubit and emit an Error state; never let exceptions reach the widget tree; repositories surface typed failures the Cubit translates to state.
sections:
  - { id: cubit-catch,   title: "Catch in the Cubit",   tokens: 130, code: true  }
  - { id: no-throw-to-ui, title: "Never Throw to the UI", tokens: 110, code: false }
  - { id: repository-errors, title: "Repository Failures", tokens: 130, code: true }
  - { id: messages,      title: "Errors Live in State",  tokens: 100, code: false }
tags: [dart, flutter, error, state, exception]
---

# Error Handling

> Failures become state, not crashes. The Cubit is the boundary where exceptions
> turn into an `*Error` state the UI can render.

## Catch in the Cubit {#cubit-catch}

Wrap fallible calls in the Cubit method and emit an error state:

```dart
Future<void> load() async {
  emit(const SampleLoading());
  try {
    emit(SampleLoaded(await repository.getItems()));
  } on AppException catch (e) {
    emit(SampleError(e.message));
  } catch (_) {
    emit(const SampleError('Something went wrong'));
  }
}
```

## Never Throw to the UI {#no-throw-to-ui}

Widgets must never see a raw exception. Anything that can throw is awaited inside
a Cubit (or a use case the Cubit calls), so an uncaught error can't bubble into
the widget tree and white-screen the app.

## Repository Failures {#repository-errors}

Repositories translate datasource/transport errors into typed domain failures
(or a sealed `Result`), so the Cubit handles a small, known set rather than
arbitrary exceptions:

```dart
Future<List<Item>> getItems() async {
  try {
    return await remote.fetch();
  } on DioException catch (e) {
    throw AppException.network(e.message);
  }
}
```

## Errors Live in State {#messages}

User-facing copy belongs in the `*Error` state's `message`, rendered by the UI —
not in thrown strings or `print`. Keep messages actionable and free of stack
detail.
