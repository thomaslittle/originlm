import { CheckCircle } from "../components/Icon";

const ROWS = [
  {
    label: "Providers and accounts",
    widget: "Display usage for the accounts it supports.",
    origin: "Supported clients, profiles and accounts retain separate identities.",
  },
  {
    label: "Usage and quotas",
    widget: "Show a reading and its reset time.",
    origin: "Provider-reported windows with freshness and reset times. Missing evidence stays unknown.",
  },
  {
    label: "Models and routes",
    widget: "Display available models.",
    origin: "Models per route, with subscription vs metered and capability evidence attached.",
  },
  {
    label: "History and issues",
    widget: "Present recent readings.",
    origin: "Observed turn costs, bounded native allowance history and reported issues.",
  },
  {
    label: "Where it runs",
    widget: "A dashboard or status surface.",
    origin: "A local control plane. Authorized operations can contact the selected provider.",
  },
  {
    label: "For developers",
    widget: "Rendering usage is the feature’s job.",
    origin: "Rust, npm and Python SDKs over the same runtime the dashboard reads.",
  },
  {
    label: "Permissions",
    widget: "Read access sufficient to display usage.",
    origin: "Per-app grants scoped to routes and workspaces, revocable at any time.",
  },
];

export function Comparison() {
  return (
    <div className="container compare-block">
      <div className="head-split head-split--tight">
        <h2 id="compare-title">A usage view is one half. The platform is the other.</h2>
        <p className="lead">
          See how a usage display differs from the runtime underneath OriginLM: shared evidence,
          typed clients and explicit permission for supported operations.
        </p>
      </div>

      <div className="compare-scroll" role="region" aria-label="Comparison table" tabIndex={0}>
        <table className="compare">
          <caption className="visually-hidden">
            A usage display’s purpose compared with OriginLM’s implemented scope.
          </caption>
          <thead>
            <tr>
              <th scope="col">Capability</th>
              <th scope="col">A usage display</th>
              <th scope="col" className="compare-us">OriginLM</th>
            </tr>
          </thead>
          <tbody>
            {ROWS.map((row) => (
              <tr key={row.label}>
                <th scope="row">{row.label}</th>
                <td>{row.widget}</td>
                <td className="compare-us">
                  <span className="yes"><CheckCircle size={16} />{row.origin}</span>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      <p className="fine-print">
        This compares product roles, not competitors or their feature sets. OriginLM claims
        follow the <a href="https://github.com/thomaslittle/originlm/blob/main/docs/adapters/SUPPORT-MATRIX.md">support matrix</a>,
        including where a provider reports nothing and we show unknown rather than a guess.
      </p>
    </div>
  );
}
