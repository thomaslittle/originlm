import { Title } from "../components/Title";
import { useId, useRef, useState } from "react";
import { CopyButton } from "../components/CopyButton";
import { Plug, Terminal } from "../components/Icon";

type Track = "app" | "project";

type Command = {
  id: string;
  label: string;
  command: string;
  note?: string;
};

const TRACKS: Record<Track, { title: string; blurb: string; badge: string; commands: Command[]; foot: string }> = {
  app: {
    title: "Control center",
    blurb: "Build the native desktop app from a source checkout.",
    badge: "source preview",
    commands: [
      {
        id: "source",
        label: "From source",
        command: "cargo run -p originlm-desktop -- --example",
        note: "Example mode reads no accounts; run without --example to read this machine. Windows desktop qualification is pending",
      },
    ],
    foot: "Hosted downloads and signed installers are not published yet. The service installer is prepared in scripts/service.",
  },
  project: {
    title: "SDK",
    blurb: "Attaches to a shared broker, or starts its own packaged helper.",
    badge: "alpha",
    commands: [
      { id: "npm", label: "npm", command: "npm install originlm@alpha", note: "The alpha tag is 0.1.0-alpha.25; npm’s latest tag still points at an older alpha" },
      { id: "pnpm", label: "pnpm", command: "pnpm add originlm@0.1.0-alpha.25" },
      { id: "bun", label: "bun", command: "bun add originlm@0.1.0-alpha.25" },
      { id: "yarn", label: "yarn", command: "yarn add originlm@0.1.0-alpha.25" },
      { id: "cargo", label: "cargo", command: "cargo add originlm@=0.1.0-alpha.25" },
      { id: "pip", label: "pip", command: "pip install originlm==0.1.0a25" },
    ],
    foot: "Published at 0.1.0-alpha.25 on npm (the alpha tag), crates.io and PyPI. Cargo links the runtime, and the helper crate is cargo install originlm-cli@=0.1.0-alpha.25. The npm and Python clients speak the local protocol.",
  },
};

export function Install() {
  const [track, setTrack] = useState<Track>("project");
  const [tab, setTab] = useState("npm");
  const trackId = useId();
  const tabRefs = useRef<Record<string, HTMLButtonElement | null>>({});
  const data = TRACKS[track];
  const current = data.commands.find((command) => command.id === tab) ?? data.commands[0];

  function chooseTrack(next: Track) {
    setTrack(next);
    setTab(next === "app" ? "source" : "npm");
  }

  function onTabKey(event: React.KeyboardEvent, index: number) {
    const keys = ["ArrowRight", "ArrowLeft", "Home", "End"];
    if (!keys.includes(event.key)) return;
    event.preventDefault();
    const length = data.commands.length;
    const next =
      event.key === "Home" ? 0
      : event.key === "End" ? length - 1
      : event.key === "ArrowRight" ? (index + 1) % length
      : (index - 1 + length) % length;
    setTab(data.commands[next].id);
    tabRefs.current[data.commands[next].id]?.focus();
  }

  return (
    <div className="container install">
      <div className="head-split">
        <Title id="install-title" text="Install" />
        <p className="lead">
          The SDK is on npm, crates.io and PyPI as an alpha. The control center builds from source;
          signed installers are not published yet.
        </p>
      </div>

      <div className="install-shell">
        <div className="install-tracks" role="tablist" aria-label="Install track" aria-orientation="vertical">
          {(["project", "app"] as Track[]).map((key) => {
            const item = TRACKS[key];
            return (
              <button
                key={key}
                type="button"
                role="tab"
                id={`${trackId}-${key}`}
                aria-selected={track === key}
                aria-controls={`${trackId}-panel`}
                tabIndex={track === key ? 0 : -1}
                className="track-card"
                onClick={() => chooseTrack(key)}
                onKeyDown={(event) => {
                  if (event.key === "ArrowDown" || event.key === "ArrowUp") {
                    event.preventDefault();
                    chooseTrack(key === "app" ? "project" : "app");
                    document.getElementById(`${trackId}-${key === "app" ? "project" : "app"}`)?.focus();
                  }
                }}
              >
                <span className="track-card-top">
                  {key === "app" ? <Plug size={18} /> : <Terminal size={18} />}
                  <strong>{item.title}</strong>
                </span>
                <span className="track-card-blurb">{item.blurb}</span>
                <code>{key === "app" ? "source build" : "npm, pnpm, bun, yarn, cargo, pip"}</code>
              </button>
            );
          })}
        </div>

        <div
          className="install-panel"
          role="tabpanel"
          id={`${trackId}-panel`}
          aria-labelledby={`${trackId}-${track}`}
        >
          <div className="install-panel-head">
            <h3>{data.title}</h3>
            <span className="install-badge">{data.badge}</span>
          </div>

          <div className="install-tabs" role="tablist" aria-label={`${data.title} packages`}>
            {data.commands.map((command, index) => (
              <button
                key={command.id}
                ref={(node) => { tabRefs.current[command.id] = node; }}
                type="button"
                role="tab"
                className="install-tab"
                aria-selected={current.id === command.id}
                tabIndex={current.id === command.id ? 0 : -1}
                onClick={() => setTab(command.id)}
                onKeyDown={(event) => onTabKey(event, index)}
              >
                {command.label}
              </button>
            ))}
          </div>

          <div className="install-cmd">
            <span className="prompt" aria-hidden="true">$</span>
            <code>{current.command}</code>
            <CopyButton value={current.command} label="Copy" className="copy-btn--lg" />
          </div>

          <p className="install-foot">
            {current.note ? <><strong>{current.note}.</strong> </> : null}
            {data.foot}
          </p>
        </div>
      </div>
    </div>
  );
}
