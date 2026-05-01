type Props = {
  message: string;
  ctaLabel: string;
  ctaHref: string;
};

export function AnnouncementBar({ message, ctaLabel, ctaHref }: Props) {
  return (
    <div className="w-full bg-ink text-white">
      <div className="mx-auto flex h-10 max-w-7xl items-center justify-center gap-2 px-token-sm text-[13px]">
        <span>{message}</span>
        <span aria-hidden className="opacity-60">
          —
        </span>
        <a
          href={ctaHref}
          className="font-medium underline decoration-white/70 underline-offset-2 transition hover:decoration-white"
        >
          {ctaLabel}
        </a>
      </div>
    </div>
  );
}
