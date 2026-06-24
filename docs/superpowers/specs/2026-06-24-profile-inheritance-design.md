# Profile inheritance (`extends`) — live, per-section convention/recipe reuse

**Date:** 2026-06-24
**Status:** approved (design)
**Branch:** `feat/profile-inheritance`

## Problem

palugada profiles are flat and self-contained. Every profile under
`knowledge/profiles/<id>/` carries its own full copy of conventions and recipes,
so the four shipped profiles (`android-mvvm`, `flutter-bloc`, `kmp`, `rust-cli`)
each re-author near-identical `architecture`/`errorhandling`/`style`/`testing`
documents. There is **no** `extends`/`inherit`/`base` mechanism (a repo-wide grep
finds only unrelated hits). The only layering that exists is the per-**project**
overlay in `src/effective.rs` (`<repo>/.palugada/conventions/` on top of the bound
profile, `Origin::{Profile,Project,Overridden}`) — that is project-on-profile, and
it is consulted only by `brief`/`effective`, never by `q`/`for`/`s`.

The concrete need: starting from `android-mvvm`, create a sibling Kotlin profile
(e.g. `android-mvi`) that differs only in *state management* (MVVM `LiveData` →
MVI `StateFlow` + reducer). Today that forces copy-pasting the entire profile and
maintaining the duplicate forever. The user wants the child to **author only what
differs** and inherit the rest live, so a fix to a parent convention propagates to
all children. In the recipe (`for feature`) the child should just re-point which
convention/section gets called, not re-describe the whole flow.

## Goal

Add a single optional `extends: <profile-id>` field to `profile.yaml` that makes a
child profile **live-inherit** its parent's conventions and recipes, overridable at
**section granularity**, resolved through a single shared layer that all readers
go through (`q`, `for`, `s`, `brief`, `validate`, web console).

Decisions locked during brainstorming:

| Decision | Choice |
|---|---|
| Inheritance model | **Live inheritance** (`extends`), not copy-on-create |
| Convention override granularity | **Per-section** (identity = section anchor id) |
| What is inherited | **conventions + recipes only** — manifest (`flows`, `review_map`, `fact_families`, `exec`) and `extractors.yaml` stay the child's own |
| Arity / chaining | **Single base, chained** (`extends:` scalar; parent may itself extend) + cycle detection + depth limit |

**Non-goals (YAGNI for v1):**

- Multiple bases / mixins (`extends: [a, b]`) — single scalar only.
- Inheriting the manifest (`flows`/`review_map`/`fact_families`/`exec`) or
  `extractors.yaml` — explicitly out; the child declares its own (seeded at
  scaffold time, see §5).
- `for`/`brief` *expanding* a recipe's `convention_refs` inline — `for` stays raw
  (prints the resolved recipe body verbatim, as today).
- A dedicated `convention override <topic> --section <id>` authoring helper — plain
  `convention add` already works; the merge is a read-time concern.
- Changing the per-**project** overlay behaviour of `q`/`for`/`s` (they still don't
  consult the project overlay; only `brief`/`effective` do). This feature only adds
  base-chain resolution.

## Approach

Introduce one shared resolver module, **`src/inherit.rs`**, that owns the entire
notion of an `extends` chain. Every existing reader is rerouted through it so the
merge logic lives in exactly one place (rejected alternative: chasing the chain
independently inside each of the ~6 readers — duplication and drift). `inherit.rs`
composes *beneath* the existing project overlay in `effective.rs`, giving the final
precedence order:

```
root ancestor  →  …  →  parent  →  child           (inherit.rs: this feature)
                                      ↘
                                       project overlay   (effective.rs: unchanged, wins last)
```

When a profile has no `extends`, its chain is `[self]` and every resolver call is
behaviourally identical to today — that is the backward-compatibility guarantee.

## Components

### 1. Manifest field (`profile.yaml`)

Add one optional scalar, read by a lightweight helper rather than by extending the
several narrow serde structs:

```yaml
id: android-mvi
extends: android-mvvm     # optional; a single profile id in the same knowledge root
languages: [kotlin]
fact_families: [...]      # NOT inherited — child's own
flows: {...}              # NOT inherited
review_map: {...}         # NOT inherited
```

- New helper in `inherit.rs`: `read_extends(kn, id) -> Option<String>` — parses just
  the `extends` key from `<id>/profile.yaml`. Because inheritance does
  not touch the manifest, the existing manifest structs (`ProfileFacts`,
  `effective::ProfileReview`, `exec::ProfileExec`, `brief::ProfileMeta`,
  `profile::ProfileId`) are **not** modified beyond optionally adding `extends:
  Option<String>` to `ProfileId` for `list`/`validate` display.
- A future `base:`/`extends:` typo is silently ignored today (no `deny_unknown_fields`);
  `validate` (§4) will warn if `extends` names a missing profile.

### 2. Resolver (`src/inherit.rs`) — the core

```rust
// Ordered most-derived first: [child, parent, grandparent, ...].
// Errors on cycle, missing parent, or depth > MAX_DEPTH (8).
fn resolve_chain(kn: &Path, id: &str) -> Result<Vec<String>, String>;

// Per-SECTION merge across the chain. None if topic absent in the whole chain.
fn resolve_convention(kn, id, topic) -> Result<Option<MergedConvention>, String>;

// Whole-recipe override by id (recipes are not sectioned).
fn resolve_recipe(kn, id, recipe) -> Result<Option<Recipe>, String>;

// Union of topic/recipe ids across the chain, child wins by id. For s/web/validate/outline.
fn merged_conv_index(kn, id)   -> Result<Vec<ConvTopic>, String>;
fn merged_recipe_index(kn, id) -> Result<Vec<RecipeEntry>, String>;
```

**Section-merge algorithm (conventions).** Topic identity = the convention id
(filename stem). Section identity = the `{#anchor}` slug. For a topic `T` defined at
several levels of the chain, walk **root → … → child**:

1. **Spine** = the section order of the most-distant ancestor *that defines topic
   `T`* (if `T` first appears at the parent, the parent sets the base order; the
   root need not define `T` at all).
2. For each descendant's copy of `T`, for each of its sections, keyed by anchor id:
   - id already present in the accumulator → **replace in place** (keep its slot);
   - id is new → **append** after the current last section, in the descendant's
     declared file order.
3. Result: a single ordered, deduped section list — **child wins** on conflict, new
   sections are appended, untouched inherited sections keep their position.
4. The topic's `title`/`description` and each section's `tokens`/`code` metadata come
   from the most-derived profile that owns that piece.
5. The merged body is reconstructed as `# Title` + each `## Heading {#anchor}` block
   in merged order, so existing renderers (`q`, outline, web) consume it unchanged.

**Recipe merge.** Recipes are not sectioned, so override is whole-recipe by id:
the nearest profile in the chain that defines recipe `R` wins; recipes the child
does not redefine are inherited verbatim. A child recipe's `convention_refs`
resolve against the **merged** convention set (§3/§4) — which is exactly why an
inherited `feature` recipe whose ref is `architecture#data-flow` automatically
points at the child's overridden `data-flow` section with no edit.

**Safety.** `resolve_chain` tracks visited ids → cycle error
(`inheritance cycle: a → b → a`); `MAX_DEPTH = 8` → depth error; a parent id with no
directory → missing-parent error. These surface both at read time and in `validate`.

### 3. Read commands honoring inheritance

All of these reroute through `inherit.rs` (replacing today's direct
`profiles/<profile>/...` joins in `src/knowledge.rs`):

| Command | New behaviour |
|---|---|
| `q <topic>[.N\|#id]` | Resolve the merged convention. Topic only in an ancestor → returned from the ancestor; child overriding one section → merged. Error only if the topic is absent in the **entire** chain (message names the chain searched). |
| `for <task>` | Resolve the merged recipe (whole-recipe override by id), then print the raw body as today — but the body now comes from the resolved (own-or-inherited) recipe. |
| `s <kw>` | Search the **merged index** (union over the chain), dedupe by id (child wins), annotate origin. |
| `brief <flow>` | `flows`/`review_map` are read from the child's own `profile.yaml` (not inherited), but each `convention(x)`/`recipe(y)` step resolves via `inherit.rs`. `effective::convention_outline_overlaid` is extended to walk the base chain **before** applying the project overlay (overlay still wins last). |

**New addressing `q <topic>#<section-id>`.** `parse_topic_arg` (knowledge.rs) gains
an `#`-suffix form that addresses a section by its anchor id, in addition to the
existing positional `.N` (which now indexes the deterministic *merged* order).
By-id addressing is the stable form under section overrides and matches how recipe
`convention_refs` already point at sections (by id).

### 4. `validate()` across the chain (`src/profile.rs`)

`profile validate <id>` extends its `web_render_checks` to be chain-aware:

- Resolve the `extends` chain; **error** on cycle / missing parent / depth exceeded.
- Build the merged convention id+section set and the merged recipe id set.
- Resolve `review_map` (family → convention id) and flow steps `convention(x)` /
  `recipe(y)` against the **merged** set, not just the local profile.
- Resolve each recipe's `convention_refs {topic, section}` against the merged set
  (topic exists somewhere in the chain **and** that section id exists in the merged
  sections of that topic).
- **Relaxation:** today validate requires every indexed topic to have its `<id>.md`
  on disk *in the same profile*. For a child, inherited topics need not exist
  locally — they must resolve somewhere in the chain. What still holds: the child's
  own `<id>.md` files stay consistent with the child's own `_index.json` (so a child
  `architecture.md` that contains only `## Data Flow` validates against a local index
  entry listing only that one section).

The child's `conventions/_index.json` therefore stays small — it lists only the
topics the child authors or partially overrides; the merged index is computed at
read time.

### 5. Authoring & scaffold

- **`palugada profile new <id> --extends <parent>`** (and the web New-profile form).
  Resolves the split between live-inherit and copy cleanly:
  - **Inherited parts (conventions/recipes)** are left **empty & live** — the child
    starts with empty `conventions/_index.json` + `recipes/_index.json`.
  - **Non-inherited parts** are **copy-seeded once** from the parent so the child is
    immediately valid and indexable: the parent's `fact_families`, `flows`,
    `review_map`, `exec`, plus the parent's `extractors.yaml` (and any
    `extractors/*.scm`) are written into the child, with `extends: <parent>` set in
    the generated `profile.yaml`. The user then trims the manifest as needed.
- **Per-section override authoring:** `palugada convention add architecture.md
  --profile android-mvi` where the file contains only the `## Data Flow` (StateFlow)
  section. The existing writer (`add_convention_from_markdown` → `write_convention_files`)
  upserts the child's local `_index.json`. No new authoring command; the merge is a
  read-time concern.
- **Web console** (`src/web.rs`, `src/web/app.js`): the New-profile form
  (`renderProfiles`) gains an optional "Extends (base profile)" selector;
  `create_profile` threads `extends` through and seeds the manifest from the parent;
  `GET /api/profile/<id>` returns the merged view with per-section **provenance**
  (own / overridden / inherited-from-`<parent>`) so the Doc Reader can label
  inherited vs overridden sections and keep the clickable recipe→convention refs.

### 6. Errors & limits

| Condition | Behaviour |
|---|---|
| `extends` names a missing profile | error: `profile android-mvi extends 'android-foo' which does not exist` (at read + validate) |
| Inheritance cycle | error: `inheritance cycle: android-mvi → android-mvvm → android-mvi` |
| Chain depth > 8 | error: `inheritance chain too deep (> 8) starting at android-mvi` |
| Topic/section absent in the whole chain | `q` error names the chain searched |

## Testing

- **Unit (`src/inherit.rs`)**: section-merge — replace-in-place, append-new,
  spine-order preservation, a 3-level chain (`android-base → android-mvvm →
  android-mvi`), child adding a brand-new section, child adding a brand-new topic;
  `resolve_chain` cycle detection and depth-limit errors; missing-parent error;
  `merged_conv_index`/`merged_recipe_index` union + dedupe (child wins by id).
- **Unit (`src/knowledge.rs`)**: `parse_topic_arg` handles `topic`, `topic.N`, and
  the new `topic#section-id`; `.N` indexes the merged order.
- **Integration**: build a temp `knowledge/` with `android-base` (architecture +
  testing + style), `android-mvvm` (`extends: android-base`, adds `statemanagement`
  LiveData), `android-mvi` (`extends: android-mvvm`, overrides only
  `architecture#data-flow` and `statemanagement` with StateFlow). Assert:
  `q architecture` returns layers+uistate (inherited) + data-flow (child);
  `q architecture#data-flow` returns the StateFlow version; `q testing` returns the
  grandparent's; `for feature` resolves the inherited recipe; `s stateflow` finds the
  child override; `profile validate android-mvi` passes; introducing a cycle makes
  validate fail.
- **Regression**: every existing single profile (no `extends`) yields byte-identical
  `q`/`for`/`s`/`brief` output (chain == `[self]`).
- **CI parity**: `cargo build --release` + `cargo test --release` stay green, no new
  warnings.

## File structure

| Path | Action | Responsibility |
|---|---|---|
| `src/inherit.rs` | Create | `resolve_chain` + section-merge + merged indexes; cycle/depth/missing-parent errors |
| `src/main.rs` | Modify | route into `inherit.rs`; `profile new --extends`; `cmd_query` handles `#section-id` |
| `src/knowledge.rs` | Modify | `query`/`recipe`/`recipe_body`/`search`/`list_topics`/`list_recipes` go through resolver; `parse_topic_arg` `#id` form |
| `src/effective.rs` | Modify | `convention_outline_overlaid`/`effective_rules` compose the base chain beneath the project overlay |
| `src/profile.rs` | Modify | `validate` chain-aware (merged-set ref checks, relaxed on-disk rule, cycle/depth/missing-parent); `scaffold_new` `--extends` manifest-seed; `ProfileId.extends` for list/validate |
| `src/web.rs` | Modify | `create_profile` `extends` param + manifest seed; `GET /api/profile/<id>` returns merged view + per-section provenance |
| `src/web/app.js` | Modify | New-profile form "Extends" selector; Doc Reader labels inherited/overridden sections |
| `knowledge/profiles/android-mvi/**` | Create (demo/fixture, optional) | worked MVVM→MVI example: `profile.yaml extends: android-mvvm`, `conventions/architecture.md` with only `## Data Flow` (StateFlow), small `_index.json` |

## Worked example (MVVM → MVI)

```
knowledge/profiles/
  android-mvvm/                    (parent, exists)
    conventions/architecture.md    ## Layers / ## UI State / ## Data Flow (LiveData)
    recipes/feature.md             refs: architecture#uistate, architecture#data-flow

  android-mvi/                     (child, new)
    profile.yaml                   extends: android-mvvm   (+ manifest + extractors seeded from parent)
    conventions/
      _index.json                  topics: [architecture (sections: [data-flow])]
      architecture.md              ## Data Flow  (StateFlow + reducer)   <- only this section
    recipes/                       (empty → feature is inherited as-is)
```

Read against profile `android-mvi`:

- `q architecture` → **Layers + UI State** (inherited from `android-mvvm`) +
  **Data Flow** (StateFlow, child override).
- `q architecture#data-flow` → the child's StateFlow version.
- `for feature` → the parent recipe verbatim; its `architecture#data-flow` ref now
  resolves to the child's StateFlow section because refs resolve against the merged
  set. This is exactly "in the feature recipe, just set which convention gets called."

## Risk / notes

- **Section-merge ordering** is the subtle part; the spine = root-order rule plus
  append-new keeps `.N` deterministic and inherited sections stable. The new
  `#section-id` addressing removes reliance on positional `.N` under overrides.
- **`q`/`for` gaining base-chain resolution** is a behaviour change for those
  commands (they were pure single-profile reads). It is required for the feature and
  is gated to chains of length > 1, so single profiles are unaffected.
- **Manifest seed at scaffold is a one-time copy** (not inheritance) — intentional,
  because the manifest is explicitly out of inheritance scope and a child needs a
  valid manifest to index. Editing the parent's manifest later does not propagate.
- **Validation must resolve through the chain** or it will false-fail children that
  rely on inherited topics; the relaxed on-disk rule (§4) is the key change.
- No version bump / npm release as part of this work unless the user asks.
