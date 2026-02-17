interface S3LogoProps {
  size?: number;
  className?: string;
}

export default function S3Logo({ size = 14, className }: S3LogoProps) {
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
        <linearGradient id="s3-bg" x1="50" y1="0" x2="50" y2="100" gradientUnits="userSpaceOnUse">
          <stop offset="0%" stopColor="#A855F7" />
          <stop offset="100%" stopColor="#7C3AED" />
        </linearGradient>
      </defs>
      <rect width="100" height="100" rx="22" fill="url(#s3-bg)" />
      <text
        x="50"
        y="50"
        textAnchor="middle"
        dominantBaseline="central"
        fill="white"
        fontWeight="bold"
        fontSize="48"
        fontFamily="system-ui, -apple-system, sans-serif"
      >
        S³
      </text>
    </svg>
  );
}
