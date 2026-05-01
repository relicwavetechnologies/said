type Feature = {
  icon: React.ReactNode;
  title: string;
  description: string;
};

const FEATURES: Feature[] = [
  {
    icon: <AppsIcon />,
    title: 'Works with all your apps',
    description:
      'Fits naturally into every app you use each day, without setup or friction.',
  },
  {
    icon: <WavesIcon />,
    title: 'Your thoughts set the pace',
    description:
      'Keeps up with your ideas so you can focus on thinking, not typing.',
  },
  {
    icon: <EyeIcon />,
    title: 'Your screen is its dictionary',
    description:
      'Understands what’s on your screen, from code syntax to everyday text.',
  },
];

export function FeaturesSection() {
  return (
    <section className="relative w-full">
      <div className="mx-auto w-full max-w-[1280px] px-token-md">
        <div className="grid grid-cols-1 gap-x-12 gap-y-14 pt-16 pb-token-xl md:grid-cols-3 md:pt-20">
          {FEATURES.map((feature) => (
            <FeatureCard key={feature.title} {...feature} />
          ))}
        </div>
      </div>
    </section>
  );
}

function FeatureCard({ icon, title, description }: Feature) {
  return (
    <div className="mx-auto flex max-w-[340px] flex-col items-center text-center">
      <div className="text-text/85" aria-hidden>
        {icon}
      </div>
      <h3 className="mt-7 text-[18px] font-medium tracking-[-0.005em] text-text">
        {title}
      </h3>
      <p className="mt-3 text-[15px] leading-[1.6] text-muted">{description}</p>
    </div>
  );
}

function AppsIcon() {
  return (
    <svg width="56" height="56" viewBox="0 0 56 56" fill="none" xmlns="http://www.w3.org/2000/svg" aria-hidden>
      <g stroke="currentColor" strokeWidth="1.4" strokeLinecap="round">
        <rect x="9" y="14" width="38" height="9" rx="4.5" />
        <circle cx="14" cy="18.5" r="1.6" fill="currentColor" stroke="none" />
        <rect x="9" y="29" width="38" height="9" rx="4.5" />
        <circle cx="14" cy="33.5" r="1.6" fill="currentColor" stroke="none" />
      </g>
    </svg>
  );
}

function WavesIcon() {
  return (
    <svg width="56" height="56" viewBox="0 0 56 56" fill="none" xmlns="http://www.w3.org/2000/svg" aria-hidden>
      <g stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" fill="none">
        <path d="M8 22c4-3 8-3 12 0s8 3 12 0 8-3 12 0 4 3 4 3" />
        <path d="M8 28c4-3 8-3 12 0s8 3 12 0 8-3 12 0 4 3 4 3" />
        <path d="M8 34c4-3 8-3 12 0s8 3 12 0 8-3 12 0 4 3 4 3" />
      </g>
    </svg>
  );
}

function EyeIcon() {
  return (
    <svg width="56" height="56" viewBox="0 0 56 56" fill="none" xmlns="http://www.w3.org/2000/svg" aria-hidden>
      <g stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round" fill="none">
        <path d="M6 28c5-8 13-12 22-12s17 4 22 12c-5 8-13 12-22 12S11 36 6 28Z" />
        <circle cx="28" cy="28" r="6" />
        <path d="M25.5 26a2 2 0 0 1 2-2" />
      </g>
    </svg>
  );
}
