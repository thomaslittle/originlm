import { CodeBlock } from "../components/CodeBlock";
import { Title } from "../components/Title";

const EVENTS: [string, string][] = [
  ["state", "ready, running, awaiting_approval, completed, error, cancelled"],
  ["text_delta", "Streamed reply text"],
  ["reasoning_summary", "When the client reports one"],
  ["tool", "Name and status only. Arguments and results are not copied"],
  ["approval_request", "A native tool asks; answer once with approvals.respond"],
  ["usage", "Unit and amount, as the client reports it"],
  ["artifact", "A file the turn produced, read with artifacts.read"],
  ["warning · error", "Coded, redacted messages"],
  ["final", "The turn’s last event"],
];

const MCP = `const session = await ai.sessions.create({
  selection: pick.selection,
  workspaceId: "design",
  hostMcpServers: [{
    name: "canvas",
    url: "http://127.0.0.1:48123/mcp",  // loopback only
    bearerToken,                        // never logged or echoed
    allowedTools: ["set_fill", "add_layer"],
  }],
});
// Host tool calls arrive as { type: "tool", name: "mcp__canvas__set_fill" }`;

export function Sessions() {
  return (
    <section className="section sessions" id="sessions" aria-labelledby="sessions-title">
      <div className="container">
        <Title id="sessions-title" text="Sessions that stream, resume and use your tools." />

        <div className="sessions-grid">
          <article className="sessions-col">
            <h3>One event stream per session</h3>
            <p>
              Subscribe once and read typed events from a cursor. Reconnect with the last cursor to
              resume; if the runtime cannot fill a gap it says so with <code>EVENT_GAP</code>.
              <code> turns.cancel</code> only touches turns your app started.
            </p>
            <dl className="event-list">
              {EVENTS.map(([type, text]) => (
                <div key={type}><dt><code>{type}</code></dt><dd>{text}</dd></div>
              ))}
            </dl>
          </article>

          <article className="sessions-col">
            <h3>Give the agent your app’s tools</h3>
            <p>
              Run a loopback MCP server in your app and attach it to a session. The client sees only
              the tools you list. Read-only sessions get host tools and nothing else.
            </p>
            <CodeBlock code={MCP} title="host-tools.ts" />
            <p className="fine">
              Claude Code and Codex: verified in live sessions on October 20, 2026. ACP clients such
              as Grok, and Muse Code: fixture-tested. OpenCode and HTTP routes return{" "}
              <code>CAPABILITY_UNSUPPORTED</code>.{" "}
              <a href="https://github.com/thomaslittle/originlm/blob/main/docs/integration/host-mcp.md">Host MCP guide</a>
            </p>
          </article>
        </div>
      </div>
    </section>
  );
}
