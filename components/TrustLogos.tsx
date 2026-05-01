'use client';

import { Marquee } from './motion/Marquee';

const LOGOS = [
  { name: 'Amazon', mark: <AmazonMark /> },
  { name: 'Notion', mark: <NotionMark /> },
  { name: 'Snowflake', mark: <SnowflakeMark /> },
  { name: 'Linear', mark: <LinearMark /> },
  { name: 'Figma', mark: <FigmaMark /> },
  { name: 'Stripe', mark: <StripeMark /> },
  { name: 'Vercel', mark: <VercelMark /> },
] as const;

export function TrustLogos() {
  return (
    <div className="w-full">
      <p className="mb-5 text-center text-[13px] uppercase tracking-[0.18em] text-muted">
        Trusted by winners at
      </p>
      <Marquee speed={36} className="text-muted">
        {LOGOS.map((logo) => (
          <span
            key={logo.name}
            aria-label={logo.name}
            className="flex h-10 items-center text-text/40 grayscale transition duration-300 hover:text-text hover:grayscale-0"
          >
            {logo.mark}
          </span>
        ))}
      </Marquee>
    </div>
  );
}

function AmazonMark() {
  return (
    <svg viewBox="0 0 60 28" width="68" height="26" fill="none" xmlns="http://www.w3.org/2000/svg" aria-hidden>
      <path
        d="M3 5h6c4 0 6.5 1.6 6.5 4.6 0 2.4-1.6 4-4.4 4.4l5 6h-3.4l-4.5-5.6h-1.7V20H3V5Zm5.7 7.3c2 0 3-.8 3-2.5 0-1.6-1-2.4-3-2.4H6v4.9h2.7Zm10.6 7.7V5h6.1c4.6 0 7.1 2.7 7.1 7.4 0 4.8-2.5 7.6-7.1 7.6h-6.1Zm2.7-2.4h3.2c2.9 0 4.5-1.7 4.5-5.2 0-3.4-1.6-5-4.5-5h-3.2v10.2ZM37 20V5h2.7v15H37Zm6 0V5h3l6 11.4V5h2.6v15h-3l-6-11.5V20H43Z"
        fill="currentColor"
      />
      <path
        d="M5 24c4 2 9 3 14 3s11-1 14-3c1-.6 2 .4 1 1.4-2 1.7-7 3.6-13 3.6S6 27.1 4 25.4c-1-1 0-2 1-1.4Z"
        fill="currentColor"
      />
    </svg>
  );
}
function NotionMark() {
  return (
    <svg viewBox="0 0 24 24" width="28" height="28" fill="none" xmlns="http://www.w3.org/2000/svg" aria-hidden>
      <rect x="2" y="2" width="20" height="20" rx="3" stroke="currentColor" strokeWidth="1.6" />
      <path d="M7.5 7v10M7.5 7l9 10M16.5 7v10" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" />
    </svg>
  );
}
function SnowflakeMark() {
  return (
    <svg viewBox="0 0 24 24" width="28" height="28" fill="none" xmlns="http://www.w3.org/2000/svg" aria-hidden>
      <g stroke="currentColor" strokeWidth="1.6" strokeLinecap="round">
        <path d="M12 2v20" />
        <path d="M2 12h20" />
        <path d="M4.9 4.9l14.2 14.2" />
        <path d="M19.1 4.9 4.9 19.1" />
      </g>
    </svg>
  );
}
function LinearMark() {
  return (
    <svg viewBox="0 0 100 24" width="64" height="22" fill="none" xmlns="http://www.w3.org/2000/svg" aria-hidden>
      <text x="0" y="18" fontFamily="Inter, sans-serif" fontWeight="600" fontSize="18" fill="currentColor">
        Linear
      </text>
    </svg>
  );
}
function FigmaMark() {
  return (
    <svg viewBox="0 0 100 24" width="60" height="22" fill="none" xmlns="http://www.w3.org/2000/svg" aria-hidden>
      <text x="0" y="18" fontFamily="Inter, sans-serif" fontWeight="600" fontSize="18" fill="currentColor">
        Figma
      </text>
    </svg>
  );
}
function StripeMark() {
  return (
    <svg viewBox="0 0 100 24" width="64" height="22" fill="none" xmlns="http://www.w3.org/2000/svg" aria-hidden>
      <text x="0" y="18" fontFamily="Inter, sans-serif" fontWeight="700" fontSize="18" fill="currentColor">
        Stripe
      </text>
    </svg>
  );
}
function VercelMark() {
  return (
    <svg viewBox="0 0 100 24" width="64" height="22" fill="none" xmlns="http://www.w3.org/2000/svg" aria-hidden>
      <path d="M2 18h20L12 4 2 18Z" fill="currentColor" />
    </svg>
  );
}
