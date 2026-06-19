---
id: testing
title: Testing
description: bloc_test for Cubit state sequences, mocktail for repositories/datasources, fakes over mocks, and widget tests that keep framework out of pure-logic tests.
sections:
  - { id: bloc-test,    title: "bloc_test",       tokens: 150, code: true  }
  - { id: mocktail,     title: "mocktail Doubles", tokens: 130, code: true  }
  - { id: fakes,        title: "Fakes Over Mocks", tokens: 100, code: false }
  - { id: widget-tests, title: "Widget Tests",     tokens: 110, code: false }
tags: [dart, flutter, testing, bloc_test, mocktail]
---

# Testing

> Test Cubits by their state sequence and repositories by their behavior. Keep
> the framework out of pure-logic tests.

## bloc_test {#bloc-test}

Use `bloc_test` to assert the emitted state sequence for an action:

```dart
blocTest<SampleCubit, SampleState>(
  'emits [Loading, Loaded] on success',
  setUp: () => when(() => repo.getItems()).thenAnswer((_) async => [item]),
  build: () => SampleCubit(repository: repo),
  act: (c) => c.load(),
  expect: () => [const SampleLoading(), SampleLoaded([item])],
);
```

Cover the error path too (`[Loading, Error]`).

## mocktail Doubles {#mocktail}

Use `mocktail` for repositories/datasources; register fallback values for custom
types and stub with `when(...).thenAnswer(...)`:

```dart
class MockRepo extends Mock implements SampleRepository {}
// in setUpAll: registerFallbackValue(FakeQuery());
```

## Fakes Over Mocks {#fakes}

For value-ish collaborators, prefer a small hand-written fake (a real in-memory
implementation) over a mock — assert on resulting state, not on which methods
were called, so tests survive behavior-preserving refactors.

## Widget Tests {#widget-tests}

Test pages with `testWidgets` + a `BlocProvider` of a fake/seeded Cubit; pump and
assert rendered widgets per state. Keep pure logic in Cubits/use cases so most
coverage is fast unit tests, not widget tests.
