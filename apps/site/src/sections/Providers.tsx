import { useMemo, useState } from "react";
import { Check } from "../components/Icon";
import { OP_LABELS, PROVIDERS, type ProviderEntry, type ProviderOps } from "../data/providers";

const FILTERS = [
  { id: "all", label: "All providers" },
  { id: "Native client", label: "Native clients" },
  { id: "Local runtime", label: "Local runtimes" },
  { id: "API route", label: "API routes" },
] as const;

const STATE_LABEL: Record<ProviderOps[keyof ProviderOps], string> = {
  yes: "supported",
  partial: "conditional",
  none: "not supported",
};

function OpMark({ state }: { state: ProviderOps[keyof ProviderOps] }) {
  return (
    <span className="op-mark" data-state={state}>
      {state === "yes" ? <Check size={15} /> : state === "partial" ? <Half /> : <Dash />}
      <span className="visually-hidden">{STATE_LABEL[state]}</span>
    </span>
  );
}

function Half() {
  return (
    <svg width={15} height={15} viewBox="0 0 16 16" aria-hidden="true" focusable="false">
      <circle cx="8" cy="8" r="5.5" fill="none" stroke="currentColor" strokeWidth="1.5" />
      <path d="M8 2.5a5.5 5.5 0 0 1 0 11z" fill="currentColor" />
    </svg>
  );
}

function Dash() {
  return (
    <svg width={15} height={15} viewBox="0 0 16 16" aria-hidden="true" focusable="false">
      <path d="M4.5 8h7" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
    </svg>
  );
}

function ProviderRow({ provider }: { provider: ProviderEntry }) {
  return (
    <tr>
      <th scope="row">
        <span className="provider-name">{provider.name}</span>
        <span className="provider-kind">{provider.kind}</span>
        <details>
          <summary>Evidence and limits</summary>
          <p>{provider.note}</p>
        </details>
      </th>
      {OP_LABELS.map((operation) => (
        <td key={operation.key} title={`${operation.label}: ${STATE_LABEL[provider.ops[operation.key]]}`}>
          <OpMark state={provider.ops[operation.key]} />
        </td>
      ))}
    </tr>
  );
}

export function Providers() {
  const [filter, setFilter] = useState<(typeof FILTERS)[number]["id"]>("all");
  const visible = useMemo(
    () => (filter === "all" ? PROVIDERS : PROVIDERS.filter((provider) => provider.kind === filter)),
    [filter],
  );

  return (
    <section className="section providers" id="providers" aria-labelledby="providers-title">
      <div className="container">
        <div className="head-split">
          <h2 id="providers-title">The AI already on the machine, with the truth about what each one exposes.</h2>
          <p className="lead">
            Eighteen documented integrations. “Supported” does not mean every operation works everywhere:
            plan data can be missing, a client can refuse text-only sessions, and an offline local
            runtime is still detected. We show the gaps instead of filling them in.
          </p>
        </div>

        <div className="provider-toolbar">
          <div className="segmented" role="group" aria-label="Filter providers">
            {FILTERS.map((item) => (
              <button
                key={item.id}
                type="button"
                className="filter-chip"
                aria-pressed={filter === item.id}
                onClick={() => setFilter(item.id)}
              >
                {item.label}
              </button>
            ))}
          </div>
          <span className="provider-count" aria-live="polite">
            {visible.length} of {PROVIDERS.length}
          </span>
        </div>

        <div className="provider-scroll" role="region" aria-label="Provider support table" tabIndex={0}>
          <table className="provider-table">
            <caption className="visually-hidden">What each provider exposes, operation by operation.</caption>
            <thead>
              <tr>
                <th scope="col">Provider</th>
                {OP_LABELS.map((operation) => <th scope="col" key={operation.key}>{operation.label}</th>)}
              </tr>
            </thead>
            <tbody>
              {visible.map((provider) => <ProviderRow key={provider.id} provider={provider} />)}
            </tbody>
          </table>
        </div>

        <div className="provider-legend">
          <span><span aria-hidden="true"><OpMark state="yes" /></span> supported</span>
          <span><span aria-hidden="true"><OpMark state="partial" /></span> conditional, fixture-tested, or gated by a reported capability</span>
          <span><span aria-hidden="true"><OpMark state="none" /></span> not supported or not reported</span>
        </div>

        <p className="fine-print">
          Condensed from the{" "}
          <a href="https://github.com/thomaslittle/originlm/blob/main/docs/adapters/SUPPORT-MATRIX.md">provider support matrix</a>.
          Evidence and verification follow each field; unknown is never rendered as zero, and passive
          detection is not execution.
        </p>
      </div>
    </section>
  );
}
