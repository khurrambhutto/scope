/**
 * Scope logo rendered as an inline SVG.
 *
 * The source of truth is docs/scope-logo.svg; this component inlines the same
 * artwork so it is available instantly in the UI without an extra asset request.
 * Keep the two files in sync if the brand mark changes.
 */
export function Logo({
  size = 32,
  className = "",
  ariaLabel = "Scope",
}: {
  size?: number;
  className?: string;
  ariaLabel?: string;
}) {
  return (
    <svg
      xmlns="http://www.w3.org/2000/svg"
      viewBox="0 0 512 512"
      width={size}
      height={size}
      className={className}
      aria-label={ariaLabel}
      role="img"
    >
      <circle cx="256" cy="256" r="220" fill="#ffffff" />
      <circle
        cx="256"
        cy="256"
        r="220"
        fill="none"
        stroke="#000000"
        strokeWidth="16"
      />
      <g transform="translate(256, 256) rotate(-23)">
        <path
          d="M -140, -15 L -110, -15 A 7.5 15 0 0 0 -110 15 L -140, 15 A 7.5 15 0 0 1 -140 -15 Z"
          fill="#000000"
        />
        <path
          d="M -102, -22 A 11 22 0 0 0 -102 22 L 0, 38 A 19 38 0 0 1 0 -38 Z"
          fill="#000000"
        />
        <path
          d="M 10, -46 A 23 46 0 0 0 10 46 L 120, 64 A 32 64 0 0 0 120 -64 Z"
          fill="#000000"
        />
        <ellipse cx="120" cy="0" rx="26" ry="52" fill="#ffffff" />
        <ellipse cx="120" cy="0" rx="20" ry="40" fill="#000000" />
        <line
          x1="104"
          y1="0"
          x2="110"
          y2="0"
          stroke="#ffffff"
          strokeWidth="1.5"
          strokeLinecap="round"
        />
        <line
          x1="130"
          y1="0"
          x2="136"
          y2="0"
          stroke="#ffffff"
          strokeWidth="1.5"
          strokeLinecap="round"
        />
        <line
          x1="120"
          y1="-28"
          x2="120"
          y2="-14"
          stroke="#ffffff"
          strokeWidth="1.5"
          strokeLinecap="round"
        />
        <line
          x1="120"
          y1="14"
          x2="120"
          y2="28"
          stroke="#ffffff"
          strokeWidth="1.5"
          strokeLinecap="round"
        />
        <path
          d="M 120, -8 L 127, -4 L 127, 4 L 120, 8 L 113, 4 L 113, -4 Z"
          fill="#ffffff"
        />
        <path
          d="M 120, 0 L 120, -8 M 120, 0 L 113, 4 M 120, 0 L 127, 4"
          stroke="#000000"
          strokeWidth="1.2"
          strokeLinejoin="round"
          strokeLinecap="round"
        />
      </g>
    </svg>
  );
}
