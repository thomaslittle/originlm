import type { ReactNode, SVGProps } from "react";

type IconProps = SVGProps<SVGSVGElement> & { size?: number };

function base({ size = 18, ...props }: IconProps, children: ReactNode) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.7}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      focusable="false"
      {...props}
    >
      {children}
    </svg>
  );
}

export const ArrowRight = (p: IconProps) =>
  base(p, <><path d="M4 12h15" /><path d="m13 6 6 6-6 6" /></>);

export const ArrowUpRight = (p: IconProps) =>
  base(p, <><path d="M7 17 17 7" /><path d="M8 7h9v9" /></>);

export const Copy = (p: IconProps) =>
  base(p, <><rect x="9" y="9" width="11" height="11" rx="2" /><path d="M5 15V5a2 2 0 0 1 2-2h8" /></>);

export const Check = (p: IconProps) =>
  base(p, <path d="m4 12 5 5L20 6" />);

export const CheckCircle = (p: IconProps) =>
  base(p, <><circle cx="12" cy="12" r="9" /><path d="m8.5 12 2.5 2.5 4.5-5" /></>);

export const Github = (p: IconProps) =>
  base({ ...p, strokeWidth: 1.5 }, <path d="M9 19c-4.3 1.4-4.3-2.5-6-3m12 6v-3.5c0-1 .1-1.4-.5-2 2.8-.3 5.5-1.4 5.5-6a4.6 4.6 0 0 0-1.3-3.2 4.2 4.2 0 0 0-.1-3.2s-1.1-.3-3.5 1.3a12.3 12.3 0 0 0-6.2 0C6.5 2.8 5.4 3.1 5.4 3.1a4.2 4.2 0 0 0-.1 3.2A4.6 4.6 0 0 0 4 9.5c0 4.6 2.7 5.7 5.5 6-.6.6-.6 1.2-.5 2V22" />);

export const Shield = (p: IconProps) =>
  base(p, <><path d="M12 3 4.5 6v5.5c0 4.5 3 7.6 7.5 9.5 4.5-1.9 7.5-5 7.5-9.5V6z" /><path d="m9 12 2 2 4-4" /></>);

export const Lock = (p: IconProps) =>
  base(p, <><rect x="4.5" y="10" width="15" height="10" rx="2" /><path d="M8 10V7a4 4 0 0 1 8 0v3" /><path d="M12 14v2" /></>);

export const EyeOff = (p: IconProps) =>
  base(p, <><path d="M3 3l18 18" /><path d="M10.6 5.2A10.9 10.9 0 0 1 12 5c6 0 9.5 7 9.5 7a17 17 0 0 1-2.7 3.7" /><path d="M6.6 6.6C4 8.4 2.5 12 2.5 12s3.5 7 9.5 7a10 10 0 0 0 4.3-1" /><path d="M9.9 9.9a3 3 0 0 0 4.2 4.2" /></>);

export const Plugs = (p: IconProps) =>
  base(p, <><path d="m8 3 1 4" /><path d="m15 3-1 4" /><path d="M6 7h12v3a6 6 0 0 1-6 6 6 6 0 0 1-6-6z" /><path d="M12 16v5" /></>);

export const Plug = (p: IconProps) =>
  base(p, <><path d="M9 3v5" /><path d="M15 3v5" /><path d="M7 8h10v2a5 5 0 0 1-10 0z" /><path d="M12 15v6" /></>);

export const Gauge = (p: IconProps) =>
  base(p, <><path d="M4 18a9 9 0 1 1 16 0" /><path d="M12 18 7.5 10" /><circle cx="12" cy="18" r="1" /></>);

export const Stack = (p: IconProps) =>
  base(p, <><path d="m12 3 9 5-9 5-9-5z" /><path d="m3 13 9 5 9-5" /><path d="m3 17 9 5 9-5" /></>);

export const Terminal = (p: IconProps) =>
  base(p, <><rect x="3" y="4" width="18" height="16" rx="2" /><path d="m7 9 3 3-3 3" /><path d="M13 15h4" /></>);

export const Sparkle = (p: IconProps) =>
  base(p, <><path d="M12 3v4" /><path d="M12 17v4" /><path d="M3 12h4" /><path d="M17 12h4" /><path d="m5.6 5.6 2.8 2.8" /><path d="m15.6 15.6 2.8 2.8" /><path d="m18.4 5.6-2.8 2.8" /><path d="m8.4 15.6-2.8 2.8" /></>);

export const Menu = (p: IconProps) =>
  base(p, <><path d="M4 7h16" /><path d="M4 12h16" /><path d="M4 17h16" /></>);

export const Close = (p: IconProps) =>
  base(p, <><path d="m6 6 12 12" /><path d="m18 6-12 12" /></>);

export const Play = (p: IconProps) =>
  base(p, <path d="M8 5.5v13l11-6.5z" fill="currentColor" stroke="none" />);

export const Activity = (p: IconProps) =>
  base(p, <path d="M3 12h4l2.5-7 4 14 2.5-7h5" />);

export const KeyRound = (p: IconProps) =>
  base(p, <><circle cx="8" cy="15" r="3.5" /><path d="m10.5 12.5 8-8" /><path d="m15 4 4 4" /><path d="m13.5 6 2.5 2.5" /></>);

export const Globe = (p: IconProps) =>
  base(p, <><circle cx="12" cy="12" r="9" /><path d="M3 12h18" /><path d="M12 3c2.5 2.5 3.8 5.6 3.8 9s-1.3 6.5-3.8 9c-2.5-2.5-3.8-5.6-3.8-9S9.5 5.5 12 3z" /></>);

export const Layers = (p: IconProps) =>
  base(p, <><path d="m12 3 8 4.5-8 4.5-8-4.5z" /><path d="m4 12 8 4.5 8-4.5" /><path d="m4 16.5 8 4.5 8-4.5" /></>);

export const Cpu = (p: IconProps) =>
  base(p, <><rect x="6" y="6" width="12" height="12" rx="2" /><path d="M9 2v4" /><path d="M15 2v4" /><path d="M9 18v4" /><path d="M15 18v4" /><path d="M2 9h4" /><path d="M2 15h4" /><path d="M18 9h4" /><path d="M18 15h4" /></>);

export const Chart = (p: IconProps) =>
  base(p, <><path d="M4 20V10" /><path d="M10 20V4" /><path d="M16 20v-7" /><path d="M22 20H2" /></>);

export const Spark = (p: IconProps) =>
  base(p, <path d="M12 3c.7 4.8 2.5 6.6 7 7-4.5.4-6.3 2.2-7 7-.7-4.8-2.5-6.6-7-7 4.5-.4 6.3-2.2 7-7z" fill="currentColor" stroke="none" />);

export const Bell = (p: IconProps) =>
  base(p, <><path d="M18 8a6 6 0 1 0-12 0c0 5-2 6-2 6h16s-2-1-2-6" /><path d="M10.5 20a2 2 0 0 0 3 0" /></>);

export const Docs = (p: IconProps) =>
  base(p, <><path d="M5 3h9l5 5v13H5z" /><path d="M14 3v5h5" /><path d="M8 13h8" /><path d="M8 17h6" /></>);

export const Compass = (p: IconProps) =>
  base(p, <><circle cx="12" cy="12" r="9" /><path d="m15.5 8.5-2 5-5 2 2-5z" /></>);

export const Pulse = (p: IconProps) =>
  base(p, <><circle cx="12" cy="12" r="2.5" /><path d="M7.7 7.7a6 6 0 0 0 0 8.6" /><path d="M16.3 16.3a6 6 0 0 0 0-8.6" /><path d="M4.5 4.5a10.6 10.6 0 0 0 0 15" /><path d="M19.5 19.5a10.6 10.6 0 0 0 0-15" /></>);
