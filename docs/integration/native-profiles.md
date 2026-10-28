# Registering native AI clients and declaring billing

Native subscription-style integrations (Gemini CLI, Antigravity ACP, Grok) are never enabled by passive detection alone. A host registers the exact installed client/profile it wants to use, and separately grants metadata and execution. See the [support matrix](../adapters/SUPPORT-MATRIX.md) for what each provider supports and the [native helper guide](native-helper.md) for the full command line.

## Register an installed client

A host-owned JSON file names the installed client and its profile. It contains paths and labels, never credentials:

```json
{
  "profiles": [
    {
      "id": "grok-personal",
      "label": "Grok personal",
      "provider": "grok",
      "executable": "C:\\Users\\you\\.grok\\bin\\grok.exe",
      "profileHome": "C:\\Users\\you\\.grok",
      "billing": "subscription"
    }
  ]
}
```

Start the helper with `--native-profiles ABSOLUTE_JSON --allow-metadata grok` to inspect the registered native catalog. Text sessions additionally require `--allow-execution grok --workspace ABSOLUTE_DIRECTORY`.

The installed client owns authentication. The toolkit sends no copied token or API key, and never turns a subscription login into a generic API key. `billing` in this file is a host declaration and stays labeled as user-supplied evidence.

## Declare billing for discovered connections

For connections that were discovered rather than registered (for example a default Grok install or a managed OpenCode server) whose provider leaves billing unknown, a host may pass `--billing-declarations ABSOLUTE_JSON`:

```json
{ "declarations": [{ "connectionId": "connection_...", "providerId": "grok", "billing": "subscription" }] }
```

Rules:

- A declaration only fills billing that is still unknown after a probe (including OpenCode gateway sub-routes). It never replaces billing the provider reported.
- It must name the provider actually discovered on that connection.
- It is recorded as `userSupplied` evidence, never as verification.
- `unknown` and `mixed` are rejected, as is `local` for account clients (Gemini CLI, Antigravity, Grok, Muse Code).
- A declared, discovered default-profile Grok may run text-only sessions. Undeclared unknown billing stays refused, and metered/credit billing still requires explicit paid consent.

## ChatGPT managed sign-in

Hosts can also offer [ChatGPT managed sign-in](chatgpt-managed-auth.md) using `--chatgpt-app-name` and the separately granted `codex_managed` connection. The helper owns OAuth and protected credentials; the installed Codex login is kept separate. Add `--executable-only-provider codex` to keep only the private Codex executable candidate and omit legacy Codex profiles and routes from discovery; other providers keep their normal inventory.
