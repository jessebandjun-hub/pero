---
kind: verification_report
date: 2026-10-29
---

# 语言设置

ROADMAP 「中英文切换」 stays checked. Desktop live switching, native menus,
and restart were already recorded in [i18n-verification.md](i18n-verification.md).
This note records the Host settings-file slice only. It does not add a
settings page and does not replace that Desktop evidence.

## Landed (L0)

The Host settings file stores `locale` as `en` or `zh-CN`.

- `settings.locale` writes one of those tags and returns it in a `settings` event.
- `settings.locale.get` returns the stored tag. With no file, it returns the same tag the Host emitted at startup.
- `fr`, `zh-cn`, and extra fields return `error.settings`. The settings file bytes stay as they were, and a missing file is not created.
- A new Host process reports the saved tag at startup and from `settings.locale.get`, including when `PERO_LOCALE` still says `en`.

`node tests/contracts/host.mjs` covers the happy path, rejection, and restart.
`cargo test -p pero-runtime --lib preferences_survive` covers a rejected write
that leaves the file bytes unchanged.

## Open

No Desktop settings page was exercised for this slice. The sidebar language
control and the native Language menu remain the UI, and their evidence is the
earlier i18n run. System tags such as `zh-Hans` still fall back to a catalog
through `normalize` when the Host reads a locale; only a stored preference
rejects them.
