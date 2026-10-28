# Muse Code native adapter

The adapter drives the installed Muse Code client through `muse serve`, the Muse Session Protocol (MSP) v1 host: newline-delimited JSON-RPC 2.0 on the child's stdin/stdout. Wire shapes were checked against `muse schema generate-json-schema` and `generate-ts` from Muse Code 1.4.7 (stable surface fingerprint `sha256:6de32db08bbbe0deb478d2bfcaba001e9fb490986bc80db3aceb4d996abf7df9`; experimental surface for `account/read`). Integration behaviour follows T3 Code's Muse provider (see [Sources](#sources-and-attribution)).

Provider id `muse`, display name "Muse Code". CLI grants: `--allow-metadata muse` and, separately, `--allow-execution muse --workspace ABSOLUTE_DIRECTORY`.

## Discovery

Passive discovery looks for a `muse` command on `PATH`, in `~/.local/bin`, and on Windows in the official installer's per-user directory `%LOCALAPPDATA%\Programs\muse`. `LOCAL_AI_KIT_MUSE_BINARY` (an absolute path to the launcher or a `muse-bin-*` binary) replaces that search for installs elsewhere; a missing configured file is reported as `muse_CONFIGURED_EXECUTABLE_UNAVAILABLE`. The profile root is Muse's configuration directory, `$XDG_CONFIG_HOME/muse` or `~/.config/muse` (also on Windows, as the launcher computes it). Passive scans never open files there, including Muse's credential store.

On Windows the installer's `muse.cmd` runs a PowerShell launcher that can download updates. The kit never runs it: once a probe or session is granted, it reads the launcher's own `.muse-version` (a release such as `1.4.7-R6101.1`, validated against the launcher's pattern) and executes the matching `muse-bin-<release>.exe` directly, as the launcher would. Elsewhere the discovered `muse` command runs directly. Every launch sets `MUSE_NO_AUTO_UPDATE=1` and inherits only the kit's scrubbed environment, so a `META_API_KEY` in the helper's environment is never forwarded (as in T3 Code); Muse uses the credential saved by `muse login` or `muse auth set`.

## Metadata (no prompt)

A probe starts a read-only, memory-only host (`--no-session-log --disable-shell --disable-write`, web/fetch/artifact tools and workflow launches off) in an empty temporary directory and sends only `initialize` (opting into the experimental surface), `account/read`, `model/list` and `usage/read`. None of these runs a model.

| Field | Source | Mapping |
| --- | --- | --- |
| Version | `initialize.serverInfo.version` | Reported. |
| Auth and billing | experimental `account/read` (`state`, `credentialRequired`, `label`) | `accountLogin` → authenticated, subscription; `apiKey`/`envKey` → authenticated, API-metered; `loggedOut` → unauthenticated (or authenticated with unknown billing when `credentialRequired` is false, a keyless deployment). An unrecognized lane or a Muse without `account/read` leaves both unknown, which blocks execution rather than assuming access. |
| Account | `account/read.label` | Display label (an email for a Meta login), released only under the identity permission like every other adapter. |
| Models | `model/list` rows with `providerId: "meta"` | Native ids and labels, `isDefault`, `contextLimit` as the context window. A `fakeCatalog` source or another provider's catalog yields no live models. |
| Reasoning effort | `variants`, `defaultReasoningEffort` | A turn-scoped `reasoningEffort` select of the reported tiers only; `"unknown"` produces no control, and unknown future tiers are skipped. |
| Images | MSP `turn/start` image parts | `input.image` with runtime origin and the condition that per-model vision is not reported; a model that cannot see images fails the turn with Muse's error. |
| Plan and quota | `usage/read` `SubscriptionUsage` | Tier as the plan; current-window and weekly percentages as one quota pool. Before Muse has observed usage there is none (`USAGE_NOT_YET_OBSERVED`). |

Muse can list a model the account cannot use. Selection cannot detect that without inference; the turn fails with Muse's own message, kept verbatim after redaction (for example `Muse turn failed (modelError): Model '…' does not exist or you lack access to it`), so the host can ask the user to pick another model.

Session creation repeats `account/read` and `model/list` in the session's own host and refuses a signed-out account (`AUTH_REQUIRED`) or a changed catalog revision (`SELECTION_STALE`) before any session starts.

## Sessions and permission modes

MSP fixes shell, write and sandbox posture for a host's lifetime; only the approval mode is chosen on the wire. One host serves one kit session.

| Kit session | `muse serve` posture | MSP `approvalMode` | Muse approvals |
| --- | --- | --- | --- |
| `read_only` (default; the only mode that accepts host MCP servers) | `--disable-shell --disable-write`, `tools.web_search.mode="off"`, `tools.web_fetch.enabled=false`, `tools.artifact.enabled=false`, `run.workflow_trigger_mode="off"` | `denyUnmatched` | Declined; a `NATIVE_REQUEST_DECLINED` warning is emitted |
| any mode with `reviewRequests` (T3 Code's **Supervised**) | as the mode | `promptUnmatched` | Each one is a kit approval (`approval_request`), answered with Muse's once-only choice |
| `workspace_write` | Muse's default posture and sandbox | `denyUnmatched` | Declined: Muse edits and runs what its policy already allows |
| `native_policy` (T3 Code's **Full access**; needs the native-policy grant) | Muse's default posture and sandbox | `allowAll` | None for the session; a workflow child's request, which does not inherit `allowAll`, is approved once (as T3 Code does) |

Settings are passed as `-c KEY=JSON`; Muse refuses to start on a key it does not know, so an incompatible Muse fails closed instead of keeping a tool. In host-reviewed sessions the adapter also sets `MUSE_DISABLE_APPROVAL_JUDGE=1` so Muse's model-based approval judge does not decide in the host's place; if Muse still reports a judge decision, a `NATIVE_APPROVAL_JUDGE` warning says so.

Approvals use `approval/decide` with the approval's `currentRequirementId`. Allowing picks the `approved`/`once` choice; denying picks `denied`/`once`, falling back to `abort` (Muse's shell approvals offer only "Reject"). Persistent or session-wide amendments are never chosen. A multi-stage approval asks again for each stage. `approval/request` and its `approval/requested` notification are one approval. Approvals from a native subagent are attributed to "A Muse subagent".

Muse is not an OS sandbox for the kit. `--disable-shell`/`--disable-write` and the approval mode are enforced by Muse, as other native clients enforce their own policies.

### Native tools in read-only sessions

Muse cannot remove its read-only tools, so a read-only session lets exactly these run and reports them as neutral `tool` events: `read_file`, `search`, `tool_search`, `read_skill`, `write_todos`, `request_user_input`, and the `subagent_*` tools (subagents run under the same host posture). Any other native tool, and any MCP tool that is not an allowed host tool, fails the turn with `INTEGRATION_RESTRICTED`. This differs from Claude Code and Codex, whose built-in tools are fully removed: with Muse the agent can still read files in the granted workspace.

### Host MCP servers

Requires the `sessionMcp` capability (requested in `initialize`; otherwise `CAPABILITY_UNSUPPORTED`). Host servers are added to `session/start` (and `session/resume`) as `config.mcpServers.<name> = {transport: "streamableHttp", url, headers: {Authorization: "Bearer …"}, mode: "optional"}`. The token travels only in that stdio frame. `optional` is deliberate, as in T3 Code: if Muse cannot reach the server the turn still runs, without those tools. Calls surface as `tool` events named `mcp__<server>__<tool>`, Muse's own naming.

## Turns and events

`turn/start` carries the prompt as a `text` part, then one `image` part per staged image (`base64Data`, `mediaType`, `width`, `height`), `ifBusy: "queue"`, and the selected `reasoningEffort`. The kit turn is the Muse turn whose id Muse acknowledges (`turnId == commandId`).

| MSP | Kit event |
| --- | --- |
| `item/delta` field `text` of an `agentMessage`; text not streamed before `item/completed` | `text_delta` |
| `reasoning` summary parts (`summary.N`) | `reasoning_summary`. Raw reasoning `text` is never forwarded. |
| `toolCall` items | `tool` `started`, then `completed`/`failed`/`cancelled` |
| `web_search`/`web_fetch` tool results and `webSearch` items | `tool` events plus extension `muse.web_search` |
| `workflow` items | extension `muse.workflow` |
| `subagent` items | extension `muse.subagent` |
| `session/todoListChanged` | extension `muse.todo` |
| terminal `compaction` items | extension `muse.compaction` |
| `session/tokenUsage` of the kit turn's Muse turns | `usage` `input_tokens` (counted-once prompt tokens), `output_tokens`, `reasoning_tokens` at the end of the turn |
| last `session/contextUsage` | `usage` `context_tokens` |
| `turn/retryScheduled` | warning `PROVIDER_RETRY` |
| `session/modelRouteUnserved` | warning `MODEL_ROUTE_UNSERVED` |
| `userInput/request` | `userInput/cancel` plus warning `NATIVE_QUESTION_DECLINED` (see gaps) |
| `view/gap` | error `EVENT_GAP` |

Skill-reminder children (`reminderChild`), the echoed prompt and hook rows are not output. Extensions are bounded and sanitized, and repeated identical payloads are coalesced:

- `muse.web_search`: `{itemId, tool, status, query?, results: [{url?, title?, snippet?}], failure?}`, at most 20 hits, `http(s)` links only, snippets up to 500 characters. A fetch reports its page URL and an excerpt.
- `muse.workflow`: `{itemId, status, entryId?, label?, children: [{childId, attempt, label?, status?, terminal?, failure?}], finalSummary?, durationMs?}`. Muse re-sends the children whole and drops their labels in later revisions; the first label seen is kept.
- `muse.subagent`: `{itemId, status, controlStatus?, subagentId?, role?, objective?, result?, failure?}`.
- `muse.todo`: `{items: [{text, status}]}`, at most 64 items.
- `muse.compaction`: `{itemId, status, outcome?, trigger?}`.

### Background work

Workflows and subagents may outlive the Muse turn that launched them, and Muse starts a turn of its own to report a finished workflow. When work launched by the kit turn is still running as its Muse turn completes, the kit turn stays open: workflow and subagent progress keep arriving, the report turn Muse starts is joined (its text, tools and approvals belong to the kit turn), and the kit turn ends when nothing it owns is running, after a 1.5-second wait for a report turn. The runtime's 10-minute turn bound still applies. Turns Muse starts that the kit turn does not own are ignored, including one it is still running when a new kit turn is queued behind it.

### Cancellation, failure and close

Cancelling sends `turn/interrupt` for the kit turn's Muse turns and `workflow/cancel` for its running workflows (bounded to 1.5 seconds); the runtime then closes the host. A failed Muse turn maps its `TurnError`: `authRequired` → `AUTH_REQUIRED` with a `muse login` hint and none of Muse's text (it can carry login codes); a quota failure → `RATE_LIMITED` with the reset time as retry-after; anything else → `PROVIDER_UNAVAILABLE` with the redacted kind and message. A stream that ends mid-turn is `PROVIDER_UNAVAILABLE`. Closing a session closes its own host only.

### Resumption

`persistent: true` drops `--no-session-log`, so Muse keeps the session in its own history (`~/.local/share/muse/sessions`). Resuming calls `session/resume` with `excludeItems: true` (the host already has its transcript) and the host MCP servers again, then `session/setApprovalMode`, and `session/setModel` if the selected model differs. A session Muse no longer has, or an invalid id, is `RESUME_UNAVAILABLE`; it never falls back to a fresh conversation. Suspension is possible after a completed turn.

## Gaps

These Muse features have no kit contract today and are not mapped:

- Structured questions (`request_user_input`): the kit's approvals are allow/deny only, so questions are declined and the model continues without an answer.
- Steering a running turn (`turn/steer`), queued follow-ups, compaction on request (`session/compact`), goals, side chats, user shell commands, skills/plugin/hook management, computer use, and video input.
- Conversation fork and rollback (T3 Code does not use them either: Muse rejects completed-turn fork boundaries).
- Native subagent drill-down: child session transcripts and workflow children's conversations are not exposed; only their status, objective and result.
- Per-model vision support, plan and quota before Muse has observed usage, and prices.
- No Plan mode; Muse has none.

## Sources and attribution

- T3 Code (`pingdotgg/t3code`, MIT, Copyright (c) 2026 T3 Tools Inc.) at `e9221e44dadd78126c83d2eeed0b51c25dcf0fb4`: `packages/provider-muse/src/server/{adapter,sdk,protocol,modelCatalog,status,itemPresentation,textGeneration}.ts` and `docs/user/providers-muse.md` (behaviour reference; includes `6ba6a0da` "run on Muse 1.4.6, with linked web search hits and workflow results" and `c1d528a3` "native subagents open in their own child threads"). The Rust code is a port of the behaviour, not a copy of the TypeScript. The transcripts in `crates/originlm-adapters/tests/fixtures/muse/t3/` are copied unchanged from `apps/server/src/orchestration-v2/testkit/fixtures/*/muse_transcript.ndjson` at that commit. See [THIRD_PARTY_NOTICES.md](../../THIRD_PARTY_NOTICES.md).
- `muse schema generate-json-schema` / `generate-ts` (Muse Code 1.4.7-R6101.1), and `muse serve --help`.

## Verification and evidence

- `cargo test -p originlm-adapters --test muse_process --test muse_catalog` plus the `muse::` unit tests: 26 replay tests run the adapter against `tests/support/muse_peer.py` replaying T3's eight recorded transcripts (Muse 1.4.3–1.4.6), two transcripts recorded from Muse 1.4.7's echo provider by `tests/support/record_muse.mjs`, and eight synthetic variants of that recording (`tests/support/make_muse_synthetic.mjs`): host tools, a foreign MCP tool, read-only tool policy, web search/fetch with reasoning, todos and retries, a declined question, sign-in required, an inaccessible model, and a truncated stream. Eight catalog tests cover auth lanes, two accounts with one model name, unknown efforts, fake/foreign catalogs, malformed rows and usage windows.
- `cargo test -p originlm-discovery --test passive`: install directory, binary override, no credential reads.
- Opt-in live checks (`LAI_LIVE_MUSE=<path>`, `crates/originlm-adapters/tests/muse_live.rs`) never spend quota: metadata uses no model, and turns use Muse's deterministic `--provider echo`. `LAI_LIVE_MUSE_PERSIST=1` additionally resumes an echo session after its host exits (this writes an echo session into Muse's history).

October 13, 2026, Windows 11, Muse Code 1.4.7-R6101.1 (signed in with a Meta account): metadata returned version 1.4.7, `accountLogin` (authenticated, subscription), one catalog model with five effort tiers, and no usage yet. Echo turns through the kit runtime streamed the expected text, ran with an unreachable optional host MCP server, and a persistent echo session resumed in a new host. Through the helper (`serve --stdio --allow-metadata muse --allow-execution muse`), `sessions.create` with a host MCP server opened a real read-only Muse session and `sessions.close` closed it, without a prompt. No Meta model turn was run: live inference, host-tool calls and approvals on a real model are not qualified.
