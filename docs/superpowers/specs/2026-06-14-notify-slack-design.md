# Design — `notify` + Slack (ChatNotify)

> **Status:** Built · **Date:** 2026-06-14 · Group A sub-project 2 (provider abstraction).
> Authored under a "complete all of Group A" directive — decisions made
> autonomously, recorded here.

## Problem / goal

No chat/notify capability exists. Add a `ChatNotify` trait + a Slack
incoming-webhook provider + a `palugada notify <msg>` command, establishing the
new-capability pattern (trait → factory → config → CLI) and the `Http::post`
infrastructure that `pr create` will reuse.

## Design

- **Trait** `ChatNotify { notify(&self, &str) -> Result<String,String>; verify(...) }`
  in `clients/mod.rs`.
- **Provider** `clients/slack.rs` — Slack incoming webhook. `notify` POSTs
  `{"text": message}` (built via `serde_json` so quotes/newlines escape); a body
  of `"ok"` maps to `"sent"`. `verify` does **not** post (would spam the channel)
  — it only confirms a webhook is configured.
- **Secret, not config:** the webhook URL embeds a token, so it lives in the auth
  profile as `chat_webhook` (`~/.palugada/secrets.yaml`), never in the committed
  project config. The project config only sets `chat: { provider: slack }`.
- **Config:** `Integrations` gains `chat: Option<Provider>`; `AuthProfile` gains
  `chat_webhook`. `config show` masks `chat_webhook`; `config verify` and
  `doctor` run `chat_notify(...).verify()` (config-presence check, no network).
- **HTTP:** `Http::post_json(url, headers, body) -> Result<String,String>` added
  (returns the raw response body); the ureq error helper now records the method.
- **CLI:** `palugada notify "build failed"`.

## Non-goals

- Teams/DingTalk providers (the `chat_webhook` secret name is generic so they
  slot in later); rich Slack blocks; posting on `verify`.

## Testing

- `slack::payload` escapes quotes/newlines into valid JSON (`{"text":"a\"b\nc"}`).
- Network send is not unit-tested (consistent with the other connectors).
- `cargo test` green; `config verify`/`doctor` compile with the new `chat` arm.

## Files

`src/http.rs` (post_json), `src/config.rs` (chat_webhook + Integrations.chat),
`src/clients/mod.rs` (ChatNotify + chat_notify factory + `mod slack`),
`src/clients/slack.rs` (new), `src/main.rs` (Notify cmd + cmd_notify + doctor/
verify/show wiring), `README.md`.
