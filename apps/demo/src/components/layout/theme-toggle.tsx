import { useSyncExternalStore } from 'react';
import { MoonIcon, SunIcon } from '@phosphor-icons/react';
import { Button } from '@/components/ui/button';
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip';
import { getTheme, setTheme, subscribeTheme } from '@/lib/theme';

export function ThemeToggle() {
  const theme = useSyncExternalStore(subscribeTheme, getTheme, () => 'dark' as const);
  const next = theme === 'dark' ? 'light' : 'dark';
  return (
    <Tooltip>
      <TooltipTrigger
        render={
          <Button variant="ghost" size="icon" aria-label={`Switch to ${next} theme`} onClick={() => setTheme(next)}>
            {theme === 'dark' ? <SunIcon /> : <MoonIcon />}
          </Button>
        }
      />
      <TooltipContent>{`Switch to ${next} theme`}</TooltipContent>
    </Tooltip>
  );
}
