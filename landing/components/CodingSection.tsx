'use client';

import { useState } from 'react';

type Slide = {
  id: string;
  title: string;
  description: string;
  visual: React.ReactNode;
  designedFor: { name: string; mark: React.ReactNode }[];
};

const SLIDES: Slide[] = [
  {
    id: 'tech',
    title: 'Prompting with technical accuracy',
    description:
      'We benchmarked models on how accurately they capture developer language. Avalon consistently nails terms like GPT-4o, kubectl, and PyTorch.',
    visual: <SlideTechAccuracy />,
    designedFor: [
      { name: 'Cursor', mark: <CursorMark /> },
      { name: 'Windsurf', mark: <WindsurfMark /> },
      { name: 'VS Code', mark: <VSCodeMark /> },
    ],
  },
  {
    id: 'syntax',
    title: 'Syntax Highlighting',
    description:
      'Avalon highlights code the way developers expect — with precise syntax recognition across languages and frameworks.',
    visual: <SlideSyntax />,
    designedFor: [
      { name: 'Claude', mark: <ClaudeMark /> },
      { name: 'Lovable', mark: <LovableMark /> },
      { name: 'Bolt', mark: <BoltMark /> },
    ],
  },
  {
    id: 'speed',
    title: 'Prompt at the speed of thought',
    description:
      'Avalon turns natural, rambling speech into precise prompts — letting you build faster without stopping to edit your thoughts.',
    visual: <SlideSpeed />,
    designedFor: [
      { name: 'OpenAI', mark: <OpenAIMark /> },
      { name: 'Linear', mark: <LinearMark /> },
      { name: 'Replit', mark: <ReplitMark /> },
    ],
  },
];

export function CodingSection() {
  const [active, setActive] = useState(0);
  const slide = SLIDES[active];

  return (
    <section className="relative w-full bg-black text-white">
      <div className="mx-auto w-full max-w-[1280px] px-token-md pt-token-xl">
        {/* Header */}
        <div className="max-w-[760px]">
          <p className="text-[15px] text-white/45">Coding &amp; Prompting</p>
          <h2 className="mt-6 text-[44px] font-normal leading-[1.04] tracking-[-0.02em] sm:text-[52px] md:text-[60px] lg:text-[64px]">
            Prompt faster with your voice
          </h2>
          <p className="mt-6 max-w-[560px] text-[18px] leading-[1.55] text-white/55 md:text-[20px]">
            Speak your ideas into existence with ease. Aqua understands
            syntax, libraries, and frameworks as you speak.
          </p>
        </div>

        {/* Slide visual */}
        <div className="mt-16 md:mt-20">
          <div className="relative overflow-hidden rounded-[18px] border border-white/[0.08] bg-[#0c0c0e]">
            <div className="aspect-[16/9] w-full">{slide.visual}</div>
          </div>
        </div>

        {/* Caption row */}
        <div className="mt-8 grid grid-cols-1 items-start gap-y-6 md:grid-cols-12 md:gap-x-8">
          <div className="md:col-span-7">
            <h3 className="text-[20px] font-medium tracking-[-0.005em]">
              {slide.title}
            </h3>
            <p className="mt-3 max-w-[520px] text-[15px] leading-[1.6] text-white/55">
              {slide.description}
            </p>
          </div>
          <div className="md:col-span-5 md:flex md:justify-end">
            <DesignedFor logos={slide.designedFor} />
          </div>
        </div>

        {/* Pagination + Hold Space row */}
        <div className="mt-14 flex flex-wrap items-center justify-between gap-y-6">
          <div className="flex items-center gap-3 text-[15px] text-white/80">
            <span>Hold</span>
            <kbd className="inline-flex h-9 min-w-[72px] items-center justify-center rounded-[8px] bg-white/10 px-3.5 font-sans text-[13px] font-medium text-white shadow-[inset_0_0_0_1px_rgba(255,255,255,0.10)]">
              Space
            </kbd>
            <span>and try yourself</span>
          </div>

          <Pagination
            count={SLIDES.length}
            active={active}
            onSelect={setActive}
          />
        </div>
      </div>
    </section>
  );
}

function DesignedFor({ logos }: { logos: Slide['designedFor'] }) {
  return (
    <div className="flex items-center gap-3 text-[14px] text-white/45">
      <span>Designed for</span>
      <ul className="flex items-center gap-2">
        {logos.map((logo) => (
          <li
            key={logo.name}
            aria-label={logo.name}
            className="flex h-8 w-8 items-center justify-center rounded-[8px] bg-white/[0.06] ring-1 ring-white/[0.08]"
          >
            {logo.mark}
          </li>
        ))}
      </ul>
    </div>
  );
}

function Pagination({
  count,
  active,
  onSelect,
}: {
  count: number;
  active: number;
  onSelect: (index: number) => void;
}) {
  return (
    <div className="flex items-center gap-2" role="tablist" aria-label="Slides">
      {Array.from({ length: count }).map((_, i) => {
        const isActive = i === active;
        return (
          <button
            key={i}
            role="tab"
            aria-selected={isActive}
            aria-label={`Slide ${i + 1}`}
            onClick={() => onSelect(i)}
            className={`h-1.5 rounded-full transition-all ${
              isActive ? 'w-12 bg-white' : 'w-6 bg-white/20 hover:bg-white/30'
            }`}
          />
        );
      })}
    </div>
  );
}

/* ───────────────────────── slides ───────────────────────── */

function SlideTechAccuracy() {
  return (
    <div className="grid h-full grid-cols-12 gap-3 p-4 md:p-6">
      {/* Code editor */}
      <div className="relative col-span-7 overflow-hidden rounded-[12px] bg-[#0a0a0c] ring-1 ring-white/[0.06]">
        <CodeEditor />
      </div>
      {/* Prompt floating */}
      <div className="col-span-5 flex flex-col justify-center gap-3">
        <div className="rounded-[14px] bg-[#16171b] p-5 ring-1 ring-white/[0.08] shadow-[0_24px_60px_rgba(0,0,0,0.4)]">
          <p className="text-[14.5px] leading-[1.55] text-white/85">
            Can you modify the ToDoListController to use Zustand instead of
            React
          </p>
          <div className="mt-5 flex items-center justify-between">
            <span className="inline-flex items-center gap-1.5 rounded-full bg-white/[0.08] px-2.5 py-1 text-[12px] text-white/60">
              <ChatGPTDot /> GPT-4o
            </span>
            <span className="inline-flex items-center gap-1.5 rounded-full bg-white/[0.08] px-3 py-1 text-[12px] text-white/70">
              Send <span className="text-white/50">↵</span>
            </span>
          </div>
        </div>
        <ClaudeCodeTerminal compact />
      </div>
    </div>
  );
}

function SlideSyntax() {
  return (
    <div className="flex h-full items-center justify-center p-6 md:p-10">
      <ClaudeCodeTerminal />
    </div>
  );
}

function SlideSpeed() {
  return (
    <div className="grid h-full grid-cols-12 gap-3 p-4 md:p-6">
      <div className="col-span-7 overflow-hidden rounded-[12px] bg-[#0a0a0c] p-6 ring-1 ring-white/[0.06]">
        <ChatTranscript />
      </div>
      <div className="col-span-5 flex flex-col gap-3">
        <div className="grid h-1/2 grid-cols-3 gap-3">
          {Array.from({ length: 3 }).map((_, i) => (
            <div
              key={i}
              className="rounded-[10px] bg-white/[0.04] ring-1 ring-white/[0.06]"
            />
          ))}
        </div>
        <div className="h-3 rounded-full bg-white/[0.06]" />
        <div className="h-3 w-3/4 rounded-full bg-white/[0.06]" />
        <div className="grow rounded-[12px] bg-white/[0.04] ring-1 ring-white/[0.06]" />
      </div>
    </div>
  );
}

/* ───────────────── shared visual primitives ───────────────── */

function CodeEditor() {
  return (
    <div className="h-full overflow-hidden p-5 font-mono text-[12px] leading-[1.7] text-white/70">
      <CodeLine n={1}>
        <Tok kw>import</Tok>{' '}
        <Tok punct>{'{'}</Tok> Body, Button, Column, Container, Head, Heading, Hr,
        Html, Img, Link, Preview, Row, <Tok punct>{'}'}</Tok>{' '}
        <Tok kw>from</Tok>{' '}
        <Tok str>&apos;@react-email/components&apos;</Tok>;
      </CodeLine>
      <CodeLine n={2}>
        <Tok kw>import</Tok> * <Tok kw>as</Tok>{' '}
        <Tok cls>React</Tok> <Tok kw>from</Tok> <Tok str>&apos;react&apos;</Tok>;
      </CodeLine>
      <CodeLine n={3}> </CodeLine>
      <CodeLine n={4}>
        <Tok kw>const</Tok> <Tok cls>WelcomeEmail</Tok> ={' '}
        <Tok punct>(</Tok>
        <Tok punct>{'{'}</Tok>
      </CodeLine>
      <CodeLine n={5}>
        {'  '}username = <Tok str>&apos;Steve&apos;</Tok>,
      </CodeLine>
      <CodeLine n={6}>
        {'  '}company = <Tok str>&apos;ACME&apos;</Tok>,
      </CodeLine>
      <CodeLine n={7}>
        <Tok punct>{'}'}</Tok>: <Tok cls>WelcomeEmailProps</Tok>
        <Tok punct>)</Tok> ={'>'} <Tok punct>{'{'}</Tok>
      </CodeLine>
      <CodeLine n={8}>
        {'  '}
        <Tok kw>const</Tok> previewText ={' '}
        <Tok str>{'`Welcome to ${company}, ${username}!`'}</Tok>;
      </CodeLine>
      <CodeLine n={9}> </CodeLine>
      <CodeLine n={10}>
        {'  '}
        <Tok kw>return</Tok> <Tok punct>(</Tok>
      </CodeLine>
      <CodeLine n={11}>
        {'    '}
        <Tok tag>{'<Html>'}</Tok>
      </CodeLine>
      <CodeLine n={12}>
        {'      '}
        <Tok tag>{'<Head />'}</Tok>
      </CodeLine>
      <CodeLine n={13}>
        {'      '}
        <Tok tag>{'<Preview>'}</Tok>
        {'{previewText}'}
        <Tok tag>{'</Preview>'}</Tok>
      </CodeLine>
      <CodeLine n={14}>
        {'      '}
        <Tok tag>{'<Tailwind>'}</Tok>
      </CodeLine>
      <CodeLine n={15}>
        {'        '}
        <Tok tag>{'<Body'}</Tok> className=
        <Tok str>&quot;bg-white my-auto mx-auto font-sans&quot;</Tok>
        <Tok tag>{'>'}</Tok>
      </CodeLine>
    </div>
  );
}

function CodeLine({ n, children }: { n: number; children: React.ReactNode }) {
  return (
    <div className="flex">
      <span className="mr-4 w-6 select-none text-right text-white/25">{n}</span>
      <span className="whitespace-pre-wrap">{children}</span>
    </div>
  );
}

function Tok({
  children,
  kw,
  str,
  cls,
  tag,
  punct,
}: {
  children: React.ReactNode;
  kw?: boolean;
  str?: boolean;
  cls?: boolean;
  tag?: boolean;
  punct?: boolean;
}) {
  let cls_ = 'text-white/80';
  if (kw) cls_ = 'text-[#c084fc]';
  if (str) cls_ = 'text-[#86efac]';
  if (cls) cls_ = 'text-[#fbbf24]';
  if (tag) cls_ = 'text-[#7dd3fc]';
  if (punct) cls_ = 'text-white/50';
  return <span className={cls_}>{children}</span>;
}

function ClaudeCodeTerminal({ compact }: { compact?: boolean }) {
  return (
    <div
      className={`rounded-[12px] bg-[#0a0a0c] p-5 ring-1 ring-white/[0.06] ${
        compact ? 'flex-1' : ''
      }`}
    >
      {!compact && (
        <pre
          aria-hidden
          className="mb-5 select-none whitespace-pre text-center font-mono text-[10px] leading-[1.05] text-white/35"
        >
          {`██████ ██╗      █████╗ ██╗   ██╗██████  ███████\n██╔═══╝██║     ██╔══██╗██║   ██║██╔══██ ██╔════╝\n██║    ██║     ███████║██║   ██║██║  ██ █████╗\n██║    ██║     ██╔══██║██║   ██║██║  ██ ██╔══╝\n╚█████╗████████╗██║  ██║╚██████╔╝██████╔ ███████\n ╚════╝╚═══════╝╚═╝  ╚═╝ ╚═════╝ ╚═════  ╚══════\n\n      ██████  ██████  ██████  ███████\n     ██╔════╝██╔═══██╗██╔══██╗██╔════╝\n     ██║     ██║   ██║██║  ██║█████╗\n     ██║     ██║   ██║██║  ██║██╔══╝\n     ╚██████╗╚██████╔╝██████╔╝███████\n      ╚═════╝ ╚═════╝ ╚═════╝ ╚══════`}
        </pre>
      )}
      <div className="rounded-[8px] bg-white/[0.03] p-3 font-mono text-[11.5px] leading-[1.6] text-[#fca5a5] ring-1 ring-[#fca5a5]/20">
        <span className="text-white/40">+</span>{' '}
        <span className="text-white/85">Welcome to</span>{' '}
        <span className="text-white">Claude Code</span>{' '}
        <span className="text-white/85">research preview!</span>
      </div>
      {!compact && (
        <>
          <div className="mt-3 rounded-[8px] bg-white/[0.03] p-3 font-mono text-[11.5px] leading-[1.6] text-white/80 ring-1 ring-white/[0.07]">
            <span className="text-white/40">{'>'}</span> I can&rsquo;t run npm_run dev on this project. Can you look into it and tell me what&rsquo;s wrong?
          </div>
          <div className="mt-3 rounded-[8px] bg-white/[0.03] p-3 font-mono text-[11.5px] text-white/80 ring-1 ring-white/[0.07]">
            <span className="text-white/40">{'>'}</span>
          </div>
        </>
      )}
      {compact && (
        <div className="mt-3 rounded-[8px] bg-white/[0.03] p-3 font-mono text-[11px] text-white/70 ring-1 ring-white/[0.07]">
          <span className="text-white/40">{'>'}</span> Ask our agent...
        </div>
      )}
    </div>
  );
}

function ChatTranscript() {
  return (
    <div className="space-y-4 text-[12.5px] leading-[1.6] text-white/80">
      <p>
        Let me design a beautiful, modern fitness application inspired by clean,
        health-focused design principles. I&rsquo;ll create an intuitive,
        user-centered interface that motivates users and makes tracking workouts
        and progress effortless.
      </p>
      <p className="text-white/60">Design inspiration:</p>
      <ul className="space-y-1 pl-4 text-white/70 [&>li]:relative [&>li]:before:absolute [&>li]:before:-left-3 [&>li]:before:text-white/40 [&>li]:before:content-['•']">
        <li>Bold, energetic typography to inspire action</li>
        <li>Dynamic animations for workout transitions</li>
        <li>Card-based layouts for workouts</li>
        <li>Vibrant gradients and glass-morphism effects</li>
      </ul>
      <div className="ml-auto w-fit max-w-[280px] rounded-[10px] bg-white/[0.06] px-4 py-3 text-[12px] text-white/80 ring-1 ring-white/[0.08]">
        I want the hero section to have a big headline in bold, then a
        subheadline that&rsquo;s a little lighter, and maybe a button that says
        &lsquo;Get Started.&rsquo;
      </div>
    </div>
  );
}

function ChatGPTDot() {
  return <span className="inline-block h-3 w-3 rounded-full bg-[#10a37f]" />;
}

/* ───────────────── tiny brand glyphs ───────────────── */

function CursorMark() {
  return (
    <svg width="14" height="14" viewBox="0 0 24 24" fill="none">
      <path d="M4 4l16 8-7 1.5L11 21 4 4z" fill="#fff" />
    </svg>
  );
}
function WindsurfMark() {
  return (
    <svg width="14" height="14" viewBox="0 0 24 24" fill="none">
      <path
        d="M3 14c4-3 7-3 11 0s7 3 7 3M3 8c4-3 7-3 11 0s7 3 7 3"
        stroke="#fff"
        strokeWidth="1.6"
        strokeLinecap="round"
        fill="none"
      />
    </svg>
  );
}
function VSCodeMark() {
  return (
    <svg width="14" height="14" viewBox="0 0 24 24" fill="none">
      <path d="M17 3 7 12l10 9V3z" fill="#3b82f6" />
    </svg>
  );
}
function ClaudeMark() {
  return (
    <svg width="14" height="14" viewBox="0 0 24 24" fill="none">
      <path
        d="M12 2c2 4 4 6 8 8-4 2-6 4-8 8-2-4-4-6-8-8 4-2 6-4 8-8z"
        fill="#f97316"
      />
    </svg>
  );
}
function LovableMark() {
  return (
    <svg width="14" height="14" viewBox="0 0 24 24" fill="none">
      <circle cx="12" cy="12" r="9" fill="#f472b6" />
    </svg>
  );
}
function BoltMark() {
  return (
    <svg width="14" height="14" viewBox="0 0 24 24" fill="none">
      <path d="M13 2 4 14h6l-1 8 9-12h-6l1-8z" fill="#fbbf24" />
    </svg>
  );
}
function OpenAIMark() {
  return (
    <svg width="14" height="14" viewBox="0 0 24 24" fill="none">
      <circle cx="12" cy="12" r="9" stroke="#fff" strokeWidth="1.6" fill="none" />
      <path d="M8 12h8M12 8v8" stroke="#fff" strokeWidth="1.6" />
    </svg>
  );
}
function LinearMark() {
  return (
    <svg width="14" height="14" viewBox="0 0 24 24" fill="none">
      <rect x="4" y="4" width="16" height="16" rx="3" fill="#a78bfa" />
    </svg>
  );
}
function ReplitMark() {
  return (
    <svg width="14" height="14" viewBox="0 0 24 24" fill="none">
      <path d="M5 4h7v7H5zM12 11h7v7h-7zM5 11h7v9H5z" fill="#22d3ee" />
    </svg>
  );
}
