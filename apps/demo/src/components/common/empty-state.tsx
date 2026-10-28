import type { ReactNode } from 'react';
import type { Icon } from '@phosphor-icons/react';
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle, EmptyContent } from '@/components/ui/empty';
import { cn } from '@/lib/utils';

export function EmptyState({ icon: IconComponent, title, description, className, children }: { icon: Icon; title: string; description?: string; className?: string; children?: ReactNode }) {
  return (
    <Empty className={cn('border border-dashed bg-background/30', className)}>
      <EmptyHeader>
        <EmptyMedia variant="icon">
          <IconComponent />
        </EmptyMedia>
        <EmptyTitle>{title}</EmptyTitle>
        {description ? <EmptyDescription>{description}</EmptyDescription> : null}
      </EmptyHeader>
      {children ? <EmptyContent>{children}</EmptyContent> : null}
    </Empty>
  );
}
