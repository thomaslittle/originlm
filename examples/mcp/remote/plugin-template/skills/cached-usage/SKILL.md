---
name: cached-usage
description: Read selected cached Codex/Claude quota windows directly when the user asks about provider usage, remaining allowance or resets.
---

Call `kit_usage_cached` with `{}` directly. Do not launch or message a T3 agent,
send a provider prompt, refresh/probe a provider or read credentials to answer.
Report used and remaining separately, identify session/five-hour versus weekly
windows and model scope when reported, retain resets/observation time/freshness,
and redact account labels. Explicit mappings are operator assertions; an unmapped
account must not be presented as the account paying for this conversation.

Stale/unknown/missing/unsupported values are not zero or full allowance. Do not
infer recovery from elapsed reset times or confuse context counters with quota.
If the tool/connection is absent or denied, state that exact boundary and stop;
ask for scoped connection setup rather than triggering another agent or poller.
