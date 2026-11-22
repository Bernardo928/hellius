// Ícones em SVG inline (sem fonte de ícones nem rede). Traço de 1.5px, 16x16.
import type { ReactElement } from 'react';

type Props = { size?: number };

function Svg({ size = 16, children }: Props & { children: ReactElement | ReactElement[] }) {
  return (
    <svg width={size} height={size} viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true" focusable="false">
      {children}
    </svg>
  );
}

export const IconSidebar = (p: Props) => (
  <Svg {...p}>
    <rect x="1.75" y="2.75" width="12.5" height="10.5" rx="2" />
    <path d="M6 2.75v10.5" />
  </Svg>
);
export const IconInspector = (p: Props) => (
  <Svg {...p}>
    <rect x="1.75" y="2.75" width="12.5" height="10.5" rx="2" />
    <path d="M10 2.75v10.5" />
  </Svg>
);
export const IconBack = (p: Props) => (
  <Svg {...p}>
    <path d="M10 3.5 5.5 8l4.5 4.5" />
  </Svg>
);
export const IconForward = (p: Props) => (
  <Svg {...p}>
    <path d="M6 3.5 10.5 8 6 12.5" />
  </Svg>
);
export const IconCopy = (p: Props) => (
  <Svg {...p}>
    <rect x="5.25" y="5.25" width="8.5" height="8.5" rx="1.75" />
    <path d="M10.75 5.25V3.5a1.25 1.25 0 0 0-1.25-1.25h-6A1.25 1.25 0 0 0 2.25 3.5v6a1.25 1.25 0 0 0 1.25 1.25h1.75" />
  </Svg>
);
export const IconSelectAll = (p: Props) => (
  <Svg {...p}>
    <rect x="2.25" y="2.25" width="11.5" height="11.5" rx="2" strokeDasharray="2 2" />
    <path d="M5 8h6M5 5.5h6M5 10.5h4" />
  </Svg>
);
export const IconSettings = (p: Props) => (
  <Svg {...p}>
    <circle cx="8" cy="8" r="2.25" />
    <path d="M8 1.75v1.5M8 12.75v1.5M14.25 8h-1.5M3.25 8h-1.5M12.4 3.6l-1.06 1.06M4.66 11.34 3.6 12.4M12.4 12.4l-1.06-1.06M4.66 4.66 3.6 3.6" />
  </Svg>
);
export const IconInfo = (p: Props) => (
  <Svg {...p}>
    <circle cx="8" cy="8" r="6.25" />
    <path d="M8 7.25v4" />
    <path d="M8 4.75h.01" strokeWidth="2" />
  </Svg>
);
export const IconSearch = (p: Props) => (
  <Svg {...p}>
    <circle cx="7" cy="7" r="4.5" />
    <path d="m10.5 10.5 3.25 3.25" />
  </Svg>
);
export const IconSpark = (p: Props) => (
  <Svg {...p}>
    <path d="M8 1.75 9.4 6.6l4.85 1.4-4.85 1.4L8 14.25 6.6 9.4 1.75 8 6.6 6.6z" />
  </Svg>
);
export const IconClose = (p: Props) => (
  <Svg {...p}>
    <path d="m4 4 8 8M12 4l-8 8" />
  </Svg>
);
export const IconUp = (p: Props) => (
  <Svg {...p}>
    <path d="M3.5 10 8 5.5l4.5 4.5" />
  </Svg>
);
export const IconDown = (p: Props) => (
  <Svg {...p}>
    <path d="M3.5 6 8 10.5 12.5 6" />
  </Svg>
);
export const IconOpen = (p: Props) => (
  <Svg {...p}>
    <path d="M2.25 4.25a1.5 1.5 0 0 1 1.5-1.5h2.5l1.5 1.75h4.5a1.5 1.5 0 0 1 1.5 1.5v5.75a1.5 1.5 0 0 1-1.5 1.5h-8.5a1.5 1.5 0 0 1-1.5-1.5z" />
  </Svg>
);
