// SPIKE: one TSX module drawn by Longbridge NSH inside a GPUI `AppView`.
//
// Fixture data only: the Rust host answers `host.call("getProviderCard")`
// with hard-coded JSON. Nothing here reaches the broker or a provider.
// `build.rs` compiles this file with `nsh-bundler`; the app loads the JS as
// source (`Bundle::source`), never as bytecode (the crate forbids unsafe).

import { effect, host, mount, signal, Text, View } from "nsh";

interface Usage {
  used: number;
  limit: number;
  resetsIn: string;
}

interface ProviderCard {
  name: string;
  kind: string;
  statusLabel: string;
  models: number;
  usage: Usage;
}

/** Segments in the quota meter; the design system uses twenty. */
const CELLS = 20;

const card = signal<ProviderCard | null>(null);
const failure = signal<string | null>(null);

const request = () => {
  host.call<ProviderCard>("getProviderCard").then(
    (value) => card.set(value),
    (reason: Error) => failure.set(reason?.message ?? String(reason)),
  );
};

const percent = (usage: Usage) =>
  usage.limit <= 0 ? 0 : Math.min(100, Math.max(0, Math.round((usage.used / usage.limit) * 100)));

const compact = (value: number) =>
  value >= 1_000_000
    ? `${(value / 1_000_000).toFixed(1)}M`
    : value >= 1_000
      ? `${Math.round(value / 1_000)}k`
      : String(value);

function Meter(props: { remaining: number }) {
  const filled = Math.round((props.remaining / 100) * CELLS);
  const fill =
    props.remaining < 20 ? "bg-destructive" : props.remaining < 50 ? "bg-warning" : "bg-success";
  return (
    <View class="h-2.5 w-full flex-row gap-[2px]">
      {Array.from({ length: CELLS }, (_, index) => (
        <View class={`flex-1 rounded-[1px] ${index < filled ? fill : "bg-muted"}`} />
      ))}
    </View>
  );
}

function Card(props: { data: ProviderCard }) {
  const used = percent(props.data.usage);
  return (
    <View class="gap-4">
      <View class="flex-row items-start gap-3">
        <View class="flex-1 gap-0.5">
          <Text class="text-[15px] font-semibold text-card-foreground">{props.data.name}</Text>
          <Text class="text-[12px] text-muted-foreground">{props.data.kind}</Text>
        </View>
        <View class="flex-row items-center gap-1.5 rounded-full border border-border bg-muted px-2 py-1">
          <View class="h-1.5 w-1.5 rounded-full bg-emerald-500" />
          <Text class="text-[11px] font-medium text-foreground">{props.data.statusLabel}</Text>
        </View>
      </View>

      <View class="flex-row items-end gap-2">
        <Text class="font-mono text-[24px] font-medium text-card-foreground">{String(props.data.models)}</Text>
        <Text class="pb-[2px] text-[12px] text-muted-foreground">models available</Text>
      </View>

      <View class="gap-2">
        <View class="flex-row items-baseline">
          <Text class="text-[12px] text-muted-foreground">Weekly quota</Text>
          <View class="flex-1" />
          <Text class="font-mono text-[12px] text-card-foreground">{String(used)}% used</Text>
        </View>
        <Meter remaining={100 - used} />
        <Text class="text-[11px] text-muted-foreground">
          {compact(props.data.usage.used)} of {compact(props.data.usage.limit)} · resets {props.data.usage.resetsIn}
        </Text>
      </View>
    </View>
  );
}

function App() {
  // Ask once the surface has rendered (a call at module scope runs before
  // the engine has committed the root).
  effect(() => request());

  return (
    <View class="flex-1 bg-card p-5">
      <If when={failure()}>
        <Text class="text-[12px] text-red-600">getProviderCard failed: {failure()}</Text>
      </If>
      <If when={!card()}>
        <Text class="text-[12px] text-muted-foreground">Loading provider…</Text>
      </If>
      <If when={card()}>
        <Card data={card()!} />
      </If>
    </View>
  );
}

mount(App);
