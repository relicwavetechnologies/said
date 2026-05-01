'use client';

import Image from 'next/image';
import { motion } from 'motion/react';
import { fadeUp, staggerParent, EASE_OUT, VIEWPORT_ONCE } from '@/lib/motion';

type Feature = {
  icon: React.ReactNode;
  title: string;
  description: string;
  image: string;
};

const FEATURES: Feature[] = [
  {
    icon: <AppsIcon />,
    title: 'Works with all your apps',
    description:
      'Fits naturally into every app you use each day, without setup or friction.',
    image: '/images/feature-apps.jpg',
  },
  {
    icon: <WavesIcon />,
    title: 'Your thoughts set the pace',
    description:
      'Keeps up with your ideas so you can focus on thinking, not typing.',
    image: '/images/feature-thoughts.jpg',
  },
  {
    icon: <EyeIcon />,
    title: 'Your screen is its dictionary',
    description:
      'Understands what’s on your screen, from code syntax to everyday text.',
    image: '/images/feature-screen.jpg',
  },
];

export function FeaturesSection() {
  return (
    <section className="relative w-full">
      <div className="mx-auto w-full max-w-[1280px] px-token-md">
        <motion.div
          variants={staggerParent(0.14)}
          initial="hidden"
          whileInView="show"
          viewport={VIEWPORT_ONCE}
          className="grid grid-cols-1 gap-x-8 gap-y-10 pt-16 pb-token-xl md:grid-cols-3 md:pt-20"
        >
          {FEATURES.map((feature) => (
            <motion.div key={feature.title} variants={fadeUp}>
              <FeatureCard {...feature} />
            </motion.div>
          ))}
        </motion.div>
      </div>
    </section>
  );
}

function FeatureCard({ icon, title, description, image }: Feature) {
  return (
    <motion.div
      whileHover={{ y: -6 }}
      transition={{ type: 'spring', stiffness: 220, damping: 18 }}
      className="group relative h-full overflow-hidden rounded-[20px] bg-white p-7 text-left shadow-card ring-1 ring-black/[0.04] transition-shadow duration-300 hover:shadow-card-hover"
    >
      <div className="absolute inset-0 -z-10 opacity-0 transition-opacity duration-500 group-hover:opacity-[0.06]">
        <Image
          src={image}
          alt=""
          fill
          sizes="(min-width: 768px) 33vw, 100vw"
          className="object-cover"
        />
      </div>

      <div
        className="flex h-12 w-12 items-center justify-center rounded-2xl bg-accent/10 text-accent-deep transition-all duration-300 group-hover:scale-110 group-hover:bg-accent/15"
        aria-hidden
      >
        {icon}
      </div>
      <h3 className="mt-6 text-[19px] font-medium tracking-[-0.005em] text-text">
        {title}
      </h3>
      <p className="mt-3 text-[15px] leading-[1.6] text-muted">{description}</p>

      <motion.span
        aria-hidden
        className="mt-6 inline-flex items-center gap-1.5 text-[13px] font-medium text-accent-deep opacity-0 transition-opacity duration-300 group-hover:opacity-100"
      >
        Learn more
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none">
          <path
            d="M5 12h14M13 6l6 6-6 6"
            stroke="currentColor"
            strokeWidth="1.8"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        </svg>
      </motion.span>
    </motion.div>
  );
}

function AppsIcon() {
  return (
    <svg width="24" height="24" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg" aria-hidden>
      <g stroke="currentColor" strokeWidth="1.6" strokeLinecap="round">
        <rect x="3" y="6" width="18" height="4.5" rx="2.25" />
        <circle cx="6" cy="8.25" r="0.9" fill="currentColor" stroke="none" />
        <rect x="3" y="13.5" width="18" height="4.5" rx="2.25" />
        <circle cx="6" cy="15.75" r="0.9" fill="currentColor" stroke="none" />
      </g>
    </svg>
  );
}

function WavesIcon() {
  return (
    <svg width="24" height="24" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg" aria-hidden>
      <g stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" fill="none">
        <path d="M3 9c2-1.5 4-1.5 6 0s4 1.5 6 0 4-1.5 6 0" />
        <path d="M3 13c2-1.5 4-1.5 6 0s4 1.5 6 0 4-1.5 6 0" />
        <path d="M3 17c2-1.5 4-1.5 6 0s4 1.5 6 0 4-1.5 6 0" />
      </g>
    </svg>
  );
}

function EyeIcon() {
  return (
    <svg width="24" height="24" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg" aria-hidden>
      <g stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" fill="none">
        <path d="M2.5 12c2-4 5.5-6.5 9.5-6.5s7.5 2.5 9.5 6.5c-2 4-5.5 6.5-9.5 6.5S4.5 16 2.5 12Z" />
        <circle cx="12" cy="12" r="2.7" />
      </g>
    </svg>
  );
}
