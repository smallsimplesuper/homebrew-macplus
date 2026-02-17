interface MacPlusLogoProps {
  size?: number;
  className?: string;
}

export default function MacPlusLogo({ size = 18, className }: MacPlusLogoProps) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 100 100"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
      className={className}
    >
      <defs>
        <linearGradient
          id="macplus-bg"
          x1="50"
          y1="0"
          x2="50"
          y2="100"
          gradientUnits="userSpaceOnUse"
        >
          <stop offset="0%" stopColor="#A855F7" />
          <stop offset="100%" stopColor="#7C3AED" />
        </linearGradient>
        <filter id="macplus-shadow" x="-10%" y="-10%" width="120%" height="130%">
          <feDropShadow dx="0" dy="2" stdDeviation="3" floodColor="rgba(0,0,0,0.25)" />
        </filter>
      </defs>
      <rect width="100" height="100" rx="22" fill="url(#macplus-bg)" />
      <g filter="url(#macplus-shadow)">
        <line
          x1="50"
          y1="28"
          x2="50"
          y2="72"
          stroke="white"
          strokeWidth="14"
          strokeLinecap="round"
        />
        <line
          x1="28"
          y1="50"
          x2="72"
          y2="50"
          stroke="white"
          strokeWidth="14"
          strokeLinecap="round"
        />
      </g>
    </svg>
  );
}
