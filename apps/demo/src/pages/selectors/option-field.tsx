import { useId } from 'react';
import type { Control, OptionView } from '@originlm/selector-core';
import { Field, FieldDescription, FieldLabel } from '@/components/ui/field';
import { Input } from '@/components/ui/input';
import { Select, SelectContent, SelectGroup, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';

interface OptionFieldProps {
  descriptor: OptionView;
  value: unknown;
  onSet: (descriptor: OptionView, value: unknown) => void;
}

const DEFAULT = '__provider_default__';

interface Item {
  label: string;
  value: string;
  disabled?: boolean;
}

interface ControlProps {
  descriptor: OptionView;
  id: string;
  describedBy: string;
  value: unknown;
  onSet: OptionFieldProps['onSet'];
}

function OptionSelect({ id, label, describedBy, items, value, onValueChange }: { id: string; label: string; describedBy: string; items: Item[]; value: string; onValueChange: (value: string) => void }) {
  return (
    <Select items={items} value={value} onValueChange={(next) => typeof next === 'string' && onValueChange(next)}>
      <SelectTrigger id={id} aria-label={label} aria-describedby={describedBy} className="h-8 w-full">
        <SelectValue />
      </SelectTrigger>
      <SelectContent alignItemWithTrigger={false}>
        <SelectGroup>
          {items.map((item) => (
            <SelectItem key={item.value} value={item.value} disabled={item.disabled}>
              {item.label}
            </SelectItem>
          ))}
        </SelectGroup>
      </SelectContent>
    </Select>
  );
}

function ChoiceControl({ descriptor, id, describedBy, value, onSet, control }: ControlProps & { control: Extract<Control, { type: 'select' }> }) {
  const defaultChoice = control.choices.find((choice) => choice.value === control.defaultValue);
  const items: Item[] = [
    { value: DEFAULT, label: control.defaultValue == null ? 'Use provider default' : `Provider default (${defaultChoice?.label ?? control.defaultValue})` },
    ...control.choices.map((choice) => ({
      value: JSON.stringify(choice.value),
      label: choice.label + (choice.disabledReason ? ` \u2014 ${choice.disabledReason}` : ''),
      disabled: Boolean(choice.disabledReason),
    })),
  ];
  return (
    <OptionSelect
      id={id}
      label={descriptor.label}
      describedBy={describedBy}
      items={items}
      value={value === undefined ? DEFAULT : JSON.stringify(value)}
      onValueChange={(next) => onSet(descriptor, next === DEFAULT ? undefined : JSON.parse(next))}
    />
  );
}

const BOOLEAN_ITEMS: Item[] = [
  { value: DEFAULT, label: 'Use provider default' },
  { value: 'true', label: 'Enabled' },
  { value: 'false', label: 'Disabled' },
];

function BooleanControl({ descriptor, id, describedBy, value, onSet }: ControlProps) {
  return (
    <OptionSelect
      id={id}
      label={descriptor.label}
      describedBy={describedBy}
      items={BOOLEAN_ITEMS}
      value={value === undefined ? DEFAULT : String(value)}
      onValueChange={(next) => onSet(descriptor, next === DEFAULT ? undefined : next === 'true')}
    />
  );
}

function NumberControl({ descriptor, id, describedBy, value, onSet, control }: ControlProps & { control: Extract<Control, { type: 'number' }> }) {
  return (
    <Input
      id={id}
      aria-label={descriptor.label}
      aria-describedby={describedBy}
      type="number"
      min={control.min}
      max={control.max}
      step={control.step}
      value={typeof value === 'number' ? value : ''}
      placeholder="Provider default"
      onChange={(event) => onSet(descriptor, event.target.value === '' ? undefined : event.target.valueAsNumber)}
    />
  );
}

function TextControl({ descriptor, id, describedBy, value, onSet, control }: ControlProps & { control: Extract<Control, { type: 'text' }> }) {
  return (
    <Input
      id={id}
      aria-label={descriptor.label}
      aria-describedby={describedBy}
      type="text"
      maxLength={control.maxLength}
      value={typeof value === 'string' ? value : ''}
      placeholder="Provider default"
      onChange={(event) => onSet(descriptor, event.target.value === '' ? undefined : event.target.value)}
    />
  );
}

function OptionControl(props: ControlProps) {
  const control = props.descriptor.control;
  switch (control.type) {
    case 'select':
      return <ChoiceControl {...props} control={control} />;
    case 'boolean':
      return <BooleanControl {...props} />;
    case 'number':
      return <NumberControl {...props} control={control} />;
    case 'text':
      return <TextControl {...props} control={control} />;
    default:
      return <div className="rounded-lg border border-dashed p-2.5 text-xs text-muted-foreground">{`Inspectable, not editable: ${control.nativeType}`}</div>;
  }
}

/**
 * Renders one native option descriptor exactly as the provider reported it. There is
 * no universal Low/Medium/High control: choices, ranges, and defaults come from the descriptor.
 */
export function OptionField({ descriptor, value, onSet }: OptionFieldProps) {
  const id = `native-${descriptor.id}`;
  const hintId = useId();
  const note = [`${descriptor.scope} option`, descriptor.billingSensitive ? 'May affect billing' : null, descriptor.permissionSensitive ? 'Changes permissions' : null]
    .filter(Boolean)
    .join(' \u00b7 ');
  return (
    <Field>
      <FieldLabel htmlFor={id}>{descriptor.label}</FieldLabel>
      <OptionControl descriptor={descriptor} id={id} describedBy={hintId} value={value} onSet={onSet} />
      <FieldDescription id={hintId} className="text-xs">
        {note}
      </FieldDescription>
    </Field>
  );
}
