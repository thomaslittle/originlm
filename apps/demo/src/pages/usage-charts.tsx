// This module is itself the lazy chunk: usage.tsx loads it with React.lazy, so recharts never ships in the entry bundle.
// react-doctor-disable-next-line react-doctor/prefer-dynamic-import
import { Bar, BarChart, CartesianGrid, LabelList, PolarAngleAxis, RadialBar, RadialBarChart, XAxis, YAxis } from 'recharts';
import type { QuotaPool } from 'originlm';
import { ChartContainer, ChartTooltip, ChartTooltipContent, type ChartConfig } from '@/components/ui/chart';
import { Section } from '@/components/common/section';
import { percent, toneFor, type Reading } from '@/lib/usage';

/** A radial gauge for one window that has a provider-published total. */
export function UsageRing({ reading }: { reading: Reading }) {
  const fraction = reading.fraction ?? 0;
  const config = { used: { label: 'Used', color: toneFor(fraction) } } satisfies ChartConfig;
  return (
    <div className="relative mx-auto size-40 shrink-0">
      <ChartContainer config={config} className="aspect-square size-40" role="img" aria-label={`${reading.window.label}: ${percent(fraction)} of the total used`}>
        <RadialBarChart data={[{ name: 'used', value: fraction * 100, fill: toneFor(fraction) }]} startAngle={90} endAngle={-270} innerRadius={58} outerRadius={78} barSize={14}>
          <PolarAngleAxis type="number" domain={[0, 100]} angleAxisId={0} tick={false} />
          <RadialBar dataKey="value" background={{ fill: 'var(--muted)' }} cornerRadius={8} />
        </RadialBarChart>
      </ChartContainer>
      <div className="pointer-events-none absolute inset-0 grid place-content-center text-center">
        <span className="text-3xl font-semibold tracking-tight tabular-nums">{percent(fraction)}</span>
        <span className="text-[0.65rem] text-muted-foreground">used</span>
      </div>
    </div>
  );
}

/** Utilization of every pool that publishes a total. Pools without one stay out rather than getting an invented percentage. */
export function UtilizationChart({ readings }: { readings: { pool: QuotaPool; reading: Reading }[] }) {
  const data = readings.map(({ pool, reading }) => ({
    pool: pool.windows[0]?.label ?? pool.id,
    value: Math.round((reading.fraction ?? 0) * 100),
    fill: toneFor(reading.fraction ?? 0),
  }));
  const config = { value: { label: 'Used' } } satisfies ChartConfig;
  return (
    <Section title="Utilization by pool" description="Only windows with a provider-published total are compared. Pools without a total stay out of the chart rather than getting an invented percentage.">
      <ChartContainer config={config} className="h-56 w-full" role="img" aria-label="Percentage of each pool's allowance used">
        <BarChart data={data} layout="vertical" margin={{ left: 8, right: 36 }}>
          <CartesianGrid horizontal={false} strokeDasharray="3 3" />
          <YAxis dataKey="pool" type="category" tickLine={false} axisLine={false} width={150} tick={{ fill: 'var(--muted-foreground)', fontSize: 12 }} />
          <XAxis type="number" domain={[0, 100]} tickFormatter={(tick: number) => `${tick}%`} tickLine={false} axisLine={false} tick={{ fill: 'var(--muted-foreground)', fontSize: 11 }} />
          <ChartTooltip cursor={{ fill: 'var(--surface)' }} content={<ChartTooltipContent hideLabel formatter={(value) => `${value}% used`} />} />
          <Bar dataKey="value" radius={6} barSize={20} background={{ fill: 'var(--muted)', radius: 6 }}>
            <LabelList dataKey="value" position="right" formatter={(value) => `${value}%`} className="fill-foreground text-xs" />
          </Bar>
        </BarChart>
      </ChartContainer>
    </Section>
  );
}
