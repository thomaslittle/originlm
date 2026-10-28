# @originlm/ui-react

> Alpha. Not yet published to npm.

Optional, scoped React provider/model selection components. Import the stylesheet explicitly:

```tsx
import { ProviderModelSelector } from '@originlm/ui-react';
import '@originlm/ui-react/styles.css';
<ProviderModelSelector snapshot={snapshot} variant="rail" value={selection} onValueChange={setSelection} />
```

The default is a vertical provider/account rail with models inside the chosen connection. Other variants are `compact`, `command`, `capabilities`, `comparison`, `allowance`, `preset`, and `mobile`. All consume public SDK snapshots and the same headless selection engine. They make no requests and do not execute models. Synthetic snapshots are explicitly labeled.

## Customization

Use controlled `value`/`onValueChange` or `defaultValue`, requirements/presets, controlled favorites, density, className, provider-icon and model render slots. CSS variables `--originlm-bg`, `--originlm-panel`, `--originlm-text`, `--originlm-muted`, `--originlm-line`, `--originlm-accent`, and `--originlm-selected` control appearance. All styles are scoped; no global reset or body rules. System dark/high-contrast/reduced-motion preferences are honored.

`onSensitiveOption` is a host callback for explicit approval before changing billing/permission-sensitive settings. It is not a replacement for native execution grants. A missing callback denies the change. Changed selections invalidate pending approvals.

`onSelectionInvalidated` reports catalog/revision changes instead of silently approving a different configuration. Missing capabilities remain unknown. `allowUnavailableSelection` permits inspection of unavailable routes, not execution. Favorites retain route/account identity.

Keyboard model lists use arrow keys, Home/End, Page Up/Down, and Enter. Large lists render a bounded window while retaining the active option for assistive technology. Comparison mode uses bounded pagination. Host render slots should not insert interactive controls inside listbox options; use the separate configuration/favorite actions.

See `examples/react/Inspector.tsx`. Tests use real React SSR and DOM rendering with synthetic data, including 5,000-model navigation, native future option values, escaped names, changed settings, and denied sensitive changes.
