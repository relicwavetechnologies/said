'use client';

import { motion, useReducedMotion } from 'motion/react';

type Props = {
  className?: string;
  variant?: 'hero' | 'dark';
};

export function GradientMesh({ className = '', variant = 'hero' }: Props) {
  const reduce = useReducedMotion();

  const blobs =
    variant === 'dark'
      ? [
          { color: 'rgba(103,190,255,0.25)', size: 600, x: '15%', y: '20%' },
          { color: 'rgba(124,58,237,0.18)', size: 520, x: '70%', y: '60%' },
          { color: 'rgba(56,189,248,0.20)', size: 460, x: '50%', y: '85%' },
        ]
      : [
          { color: 'rgba(103,190,255,0.55)', size: 620, x: '10%', y: '25%' },
          { color: 'rgba(186,230,253,0.55)', size: 520, x: '78%', y: '15%' },
          { color: 'rgba(165,180,252,0.40)', size: 480, x: '60%', y: '78%' },
        ];

  return (
    <div className={`pointer-events-none absolute inset-0 overflow-hidden ${className}`} aria-hidden>
      {blobs.map((b, i) => (
        <motion.div
          key={i}
          className="absolute rounded-full"
          style={{
            width: b.size,
            height: b.size,
            left: b.x,
            top: b.y,
            background: `radial-gradient(circle at center, ${b.color} 0%, transparent 65%)`,
            filter: 'blur(40px)',
          }}
          animate={
            reduce
              ? undefined
              : {
                  x: [0, 60, -40, 0],
                  y: [0, -50, 30, 0],
                  scale: [1, 1.1, 0.95, 1],
                }
          }
          transition={{
            duration: 18 + i * 4,
            repeat: Infinity,
            ease: 'easeInOut',
            delay: i * 0.6,
          }}
        />
      ))}
    </div>
  );
}
