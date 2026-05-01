'use client';

import { useEffect, useRef, useState } from 'react';
import { useInView, animate } from 'motion/react';

type Props = {
  to: number;
  from?: number;
  duration?: number;
  format?: (n: number) => string;
  className?: string;
};

export function CountUp({ to, from = 0, duration = 1.6, format, className }: Props) {
  const ref = useRef<HTMLSpanElement>(null);
  const inView = useInView(ref, { once: true, amount: 0.4 });
  const [value, setValue] = useState(from);

  useEffect(() => {
    if (!inView) return;
    const controls = animate(from, to, {
      duration,
      ease: [0.22, 1, 0.36, 1],
      onUpdate: (v) => setValue(v),
    });
    return () => controls.stop();
  }, [inView, from, to, duration]);

  const display = format ? format(value) : Math.round(value).toLocaleString();
  return (
    <span ref={ref} className={className}>
      {display}
    </span>
  );
}
