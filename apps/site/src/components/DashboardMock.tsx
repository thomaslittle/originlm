const METERS = [
  { name: "Claude Code", value: "62%", width: 62, detail: "resets in 3h 12m", warn: false },
  { name: "Codex", value: "84%", width: 84, detail: "resets in 41m", warn: true },
  { name: "Cursor", value: "41%", width: 41, detail: "monthly", warn: false },
  { name: "Ollama", value: "local", width: 18, detail: "7 models", warn: false },
];

const BARS = [38, 55, 42, 68, 61, 82, 74, 96, 63, 78, 88, 71];

export function DashboardMock() {
  return (
    <div className="dash-mock" aria-hidden="true">
      <div className="dash-bar">
        <i /><i /><i />
        <span>OriginLM · Control Center</span>
      </div>
      <div className="dash-body">
        <div className="dash-rail">
          <span data-on="true"><i />Usage</span>
          <span><i />Models</span>
          <span><i />Routes</span>
          <span><i />Issues</span>
          <span><i />Apps</span>
        </div>
        <div className="dash-main">
          <div className="dash-title">
            <strong>Today</strong>
            <span>3 providers · 41 models</span>
          </div>
          <div className="dash-metrics">
            <div className="dash-metric">
              <span>Connected</span>
              <strong>3</strong>
              <small>1 signed out</small>
            </div>
            <div className="dash-metric">
              <span>Turns today</span>
              <strong>128</strong>
              <small>+14% vs avg</small>
            </div>
            <div className="dash-metric">
              <span>Metered</span>
              <strong>$0.00</strong>
              <small>subscription</small>
            </div>
          </div>
          <div className="dash-spark">
            {BARS.map((height, index) => (
              <i key={index} style={{ height: `${height}%` }} />
            ))}
          </div>
          <div className="dash-rows">
            {METERS.map((meter) => (
              <div className="dash-row" key={meter.name} data-warn={meter.warn}>
                <b>{meter.name}</b>
                <span className="dash-meter"><i style={{ width: `${meter.width}%` }} /></span>
                <em>{meter.value}</em>
              </div>
            ))}
          </div>
        </div>
      </div>
    </div>
  );
}
