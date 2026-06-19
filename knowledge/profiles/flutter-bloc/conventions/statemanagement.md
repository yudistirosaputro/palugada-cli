---
id: statemanagement
title: State Management
description: flutter_bloc Cubits with sealed-style State classes (Initial/Loading/Loaded/Error via Equatable), emit transitions, and BlocProvider/BlocBuilder in logic-free widgets.
sections:
  - { id: cubit,        title: "Cubit per Concern",  tokens: 130, code: true  }
  - { id: state-classes, title: "Sealed-style State", tokens: 170, code: true }
  - { id: emit,         title: "Emit Transitions",    tokens: 130, code: true  }
  - { id: consume,      title: "Consuming State",      tokens: 130, code: true  }
tags: [dart, flutter, bloc, cubit, state, equatable]
---

# State Management

> `flutter_bloc` Cubits hold state; widgets render it. Model state as a closed
> set of classes and transition with `emit`.

## Cubit per Concern {#cubit}

One `Cubit<State>` per screen or feature concern, injected via GetIt and exposing
intent methods:

```dart
class SampleCubit extends Cubit<SampleState> {
  SampleCubit({required this.repository}) : super(const SampleInitial());
  final SampleRepository repository;
}
```

## Sealed-style State {#state-classes}

Model state as an abstract base plus a closed set of subclasses, with `Equatable`
for value equality:

```dart
abstract class SampleState extends Equatable {
  const SampleState();
  @override
  List<Object?> get props => [];
}
class SampleInitial extends SampleState {}
class SampleLoading extends SampleState {}
class SampleLoaded extends SampleState {
  const SampleLoaded(this.items);
  final List<Item> items;
  @override
  List<Object?> get props => [items];
}
class SampleError extends SampleState {
  const SampleError(this.message);
  final String message;
  @override
  List<Object?> get props => [message];
}
```

## Emit Transitions {#emit}

Methods emit new immutable states; never mutate in place:

```dart
Future<void> fetch() async {
  emit(const SampleLoading());
  try {
    emit(SampleLoaded(await repository.getItems()));
  } catch (e) {
    emit(SampleError(e.toString()));
  }
}
```

## Consuming State {#consume}

Inject with `BlocProvider`, render with `BlocBuilder`, react to side-effects with
`BlocListener`. Keep widgets free of business logic — they only map state to UI:

```dart
BlocBuilder<SampleCubit, SampleState>(
  builder: (context, state) => switch (state) {
    SampleLoading() => const CircularProgressIndicator(),
    SampleLoaded(:final items) => ItemList(items),
    SampleError(:final message) => ErrorView(message),
    _ => const SizedBox.shrink(),
  },
);
```
