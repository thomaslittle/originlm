import { useId, type ReactNode } from 'react';
import { Checkbox } from '@/components/ui/checkbox';
import { Field, FieldLabel } from '@/components/ui/field';
import { cn } from '@/lib/utils';

interface CheckRowProps {
  label: ReactNode;
  checked: boolean;
  onCheckedChange: (checked: boolean) => void;
  disabled?: boolean;
  className?: string;
  /** Render as a bordered chip, used for filter groups. */
  chip?: boolean;
}

/** A labelled shadcn checkbox. The label is the accessible name and the click target. */
export function CheckRow({ label, checked, onCheckedChange, disabled, className, chip }: CheckRowProps) {
  const id = useId();
  return (
    <Field
      orientation="horizontal"
      className={cn(
        'w-auto items-center gap-2',
        chip && 'rounded-full border bg-card/50 px-3 py-1.5 transition-colors hover:bg-muted/60 has-data-checked:border-primary/40 has-data-checked:bg-primary/10',
        className,
      )}
    >
      <Checkbox id={id} checked={checked} disabled={disabled} onCheckedChange={onCheckedChange} />
      <FieldLabel htmlFor={id} className="w-auto flex-none cursor-pointer text-xs font-normal text-foreground">
        {label}
      </FieldLabel>
    </Field>
  );
}
