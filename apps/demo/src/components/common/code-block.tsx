import { useState } from 'react';
import { CheckIcon, CopyIcon } from '@phosphor-icons/react';
import { Button } from '@/components/ui/button';
import { cn } from '@/lib/utils';

/** Monospace block with a copy action. The clipboard is only touched on click. */
export function CodeBlock({ children, className, label, id }: { children: string; className?: string; label?: string; id?: string }) {
  const [copied, setCopied] = useState(false);
  const [copyError, setCopyError] = useState(false);
  async function copy() {
    setCopyError(false);
    try {
      await navigator.clipboard.writeText(children);
      setCopied(true);
      setTimeout(() => setCopied(false), 1600);
    } catch {
      setCopied(false);
      setCopyError(true);
    }
  }
  return (
    <div className="group/code relative">
      {label ? <span className="mb-2 block text-xs font-medium text-muted-foreground">{label}</span> : null}
      <pre id={id} className={cn('code pr-16', className)}>
        {children}
      </pre>
      <Button
        variant="outline"
        size="icon-lg"
        aria-label={copied ? 'Copied' : 'Copy to clipboard'}
        onClick={copy}
        className={cn('absolute right-2 size-11 opacity-100', label ? 'top-8' : 'top-2')}
      >
        {copied ? <CheckIcon /> : <CopyIcon />}
      </Button>
      {copyError ? <p role="status" className="mt-2 text-sm text-destructive">Clipboard unavailable. Select and copy the code manually.</p> : null}
    </div>
  );
}
