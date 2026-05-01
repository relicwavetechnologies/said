type Props = {
  message: string;
  ctaLabel: string;
  ctaHref: string;
};

export function AnnouncementBar({ message, ctaLabel, ctaHref }: Props) {
  return (
    <div className="w-full bg-ink text-white">
      <div className="mx-auto flex h-9 items-center justify-center gap-2 px-token-sm text-[14px]">
        <span className="text-white/95">{message}</span>
        <span aria-hidden className="text-white/50">
          -
        </span>
        <a
          href={ctaHref}
          className="font-medium text-white underline decoration-white/80 underline-offset-[3px] transition hover:decoration-white"
        >
          {ctaLabel}
        </a>
      </div>
    </div>
  );
}
