import appIcon from '@/assets/brand/originlm-app-256.png';
import { cn } from '@/lib/utils';

/** User-supplied offset-O app tile, kept at its native square aspect ratio. */
export function BrandMark({ size = 34, className }: { size?: number; className?: string }) {
  return <img src={appIcon} alt="" aria-hidden="true" draggable={false}
    className={cn('shrink-0', className)} width={size} height={size} />;
}
