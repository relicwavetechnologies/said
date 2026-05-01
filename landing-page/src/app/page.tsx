"use client";

import React from "react";
import { motion } from "framer-motion";
import { Mic, ArrowRight, Zap, Code } from "lucide-react";
import SetupWizard from "@/components/SetupWizard";

export default function Home() {
  return (
    <div className="min-h-screen bg-white text-[#111] font-sans selection:bg-blue-100">
      {/* Navigation */}
      <nav className="fixed top-0 left-0 right-0 z-50 flex items-center justify-between px-6 py-4 bg-white/80 backdrop-blur-md border-b border-gray-100">
        <div className="flex items-center gap-2">
          <div className="w-6 h-6 rounded-full bg-blue-500 flex items-center justify-center">
            <Mic size={14} className="text-white" />
          </div>
          <span className="font-semibold text-lg tracking-tight">WISPR</span>
        </div>

        <div className="hidden md:flex items-center gap-8 text-sm font-medium text-gray-500">
          <a href="#features" className="hover:text-black transition-colors">Features</a>
          <a href="#demo" className="hover:text-black transition-colors">How it works</a>
          <a href="#api" className="hover:text-black transition-colors">API</a>
          <a href="#pricing" className="hover:text-black transition-colors">Pricing</a>
        </div>

        <div className="flex items-center gap-4">
          <button className="text-sm font-medium text-gray-600 hover:text-black hidden sm:block">
            Sign In
          </button>
          <button className="bg-blue-500 text-white px-5 py-2 rounded-full text-sm font-medium hover:bg-blue-600 transition-colors shadow-sm shadow-blue-500/20">
            Download
          </button>
        </div>
      </nav>

      {/* Hero Section */}
      <section className="relative pt-40 pb-20 px-6 overflow-hidden flex flex-col items-center justify-center min-h-[90vh] text-center">
        {/* Subtle background pattern/glow */}
        <div className="absolute inset-0 z-0 overflow-hidden pointer-events-none">
          <div className="absolute top-[-10%] left-1/2 -translate-x-1/2 w-[800px] h-[800px] bg-blue-50/50 rounded-full blur-[100px]" />
          <div className="absolute bottom-[-20%] left-[20%] w-[600px] h-[600px] bg-purple-50/40 rounded-full blur-[100px]" />
        </div>

        <div className="relative z-10 max-w-4xl mx-auto flex flex-col items-center">
          <motion.div 
            initial={{ opacity: 0, y: 20 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.5 }}
            className="inline-flex items-center gap-2 px-3 py-1 rounded-full bg-gray-50 border border-gray-200 text-xs font-medium text-gray-600 mb-8"
          >
            <span className="w-2 h-2 rounded-full bg-blue-500 animate-pulse" />
            Now available for Next.js 15
          </motion.div>

          <motion.h1 
            initial={{ opacity: 0, y: 20 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.5, delay: 0.1 }}
            className="text-5xl md:text-7xl font-medium tracking-tight leading-[1.1] mb-6"
          >
            We've typed for 150 years.<br className="hidden md:block" />
            It's time to speak.
          </motion.h1>

          <motion.p 
            initial={{ opacity: 0, y: 20 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.5, delay: 0.2 }}
            className="text-xl md:text-2xl text-gray-500 mb-10 max-w-2xl font-light"
          >
            Wispr Bridge turns your voice into clear, formatted text. Designed for Next.js applications with zero configuration.
          </motion.p>

          <motion.div 
            initial={{ opacity: 0, y: 20 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.5, delay: 0.3 }}
            className="flex flex-col sm:flex-row items-center gap-4"
          >
            <button className="bg-[#111] text-white px-8 py-3.5 rounded-full font-medium hover:bg-black transition-all flex items-center gap-2 shadow-lg shadow-black/10 hover:scale-[1.02]">
              <Mic size={18} />
              Start Transcribing
            </button>
            <button className="bg-white text-[#111] border border-gray-200 px-8 py-3.5 rounded-full font-medium hover:bg-gray-50 transition-all flex items-center gap-2 hover:scale-[1.02]">
              Read the Docs
              <ArrowRight size={18} className="text-gray-400" />
            </button>
          </motion.div>
        </div>
      </section>

      {/* Comparison Section */}
      <section className="py-32 px-6 bg-gray-50 border-y border-gray-100">
        <div className="max-w-6xl mx-auto text-center mb-16">
          <h2 className="text-3xl md:text-5xl font-medium tracking-tight mb-6">
            5x faster than typing and<br />twice as accurate
          </h2>
          <p className="text-gray-500 max-w-xl mx-auto text-lg font-light">
            Forget the keyboard. Write five times faster with your voice and save hours every week with flawless accuracy.
          </p>
        </div>

        <div className="max-w-5xl mx-auto bg-white rounded-3xl shadow-xl border border-gray-100 overflow-hidden flex flex-col md:flex-row">
          {/* Wispr Side */}
          <div className="flex-1 p-10 md:p-16 border-b md:border-b-0 md:border-r border-gray-100 relative">
            <div className="flex justify-between items-center mb-8 text-sm font-medium">
              <div className="flex items-center gap-2 text-blue-500">
                <Mic size={16} /> Using Wispr
              </div>
              <div className="text-gray-400 font-mono">230 WPM</div>
            </div>
            <p className="text-xl leading-relaxed text-gray-800">
              Make a new React component called TaskDashboard. Add a <span className="bg-blue-50 text-blue-600 px-1.5 py-0.5 rounded">useState</span> hook for selectedTaskId initialized to null, and another for isSidebarOpen set to true.<span className="inline-block w-3 h-3 rounded-full bg-blue-500 ml-1 shadow-[0_0_8px_rgba(59,130,246,0.6)]" />
            </p>
          </div>

          {/* Keyboard Side */}
          <div className="flex-1 p-10 md:p-16 bg-[#fafafa]">
            <div className="flex justify-between items-center mb-8 text-sm font-medium">
              <div className="flex items-center gap-2 text-gray-500">
                <Code size={16} /> Using Keyboard
              </div>
              <div className="text-gray-400 font-mono">40 WPM</div>
            </div>
            <p className="text-xl leading-relaxed text-gray-400">
              Make a new React component called TaskDashboard. Add a useState hook for selectedTaskId initialized to null, and another for isSidebarOpen set to true.<span className="inline-block w-0.5 h-6 bg-blue-500 ml-0.5 align-middle animate-pulse" />
            </p>
          </div>
        </div>
      </section>

      {/* Coding & Prompting Section (Dark) */}
      <section className="py-32 px-6 bg-[#050505] text-white border-t border-gray-900">
        <div className="max-w-6xl mx-auto">
          <div className="mb-16 max-w-2xl">
            <h3 className="text-gray-400 font-medium mb-4 tracking-wide">Coding & Prompting</h3>
            <h2 className="text-4xl md:text-5xl font-medium tracking-tight mb-6">
              Prompt faster with your voice
            </h2>
            <p className="text-gray-400 text-lg font-light leading-relaxed">
              Speak your ideas into existence with ease. Wispr understands syntax, libraries, and frameworks as you speak.
            </p>
          </div>

          <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
            {/* Editor Mockup */}
            <div className="bg-[#111] border border-gray-800 rounded-3xl p-6 relative overflow-hidden flex flex-col h-[500px]">
              <div className="absolute inset-0 bg-gradient-to-br from-blue-500/5 to-transparent pointer-events-none" />
              
              {/* Floating Voice Prompt Pill */}
              <div className="absolute bottom-10 left-1/2 -translate-x-1/2 z-20 bg-gradient-to-r from-blue-600/90 to-blue-800/90 backdrop-blur-xl border border-blue-400/30 text-white px-6 py-4 rounded-2xl shadow-2xl flex items-center gap-4 w-[90%] max-w-sm">
                <span className="text-lg font-medium tracking-tight">Can you modify the ToDoList...</span>
                <span className="w-3 h-3 rounded-full bg-blue-300 animate-pulse ml-auto shadow-[0_0_10px_rgba(147,197,253,0.8)]" />
              </div>

              {/* Code Area */}
              <div className="font-mono text-sm text-gray-500 flex-1 overflow-hidden relative z-10">
                <div className="flex gap-4 mb-4 opacity-50">
                  <div className="flex gap-1.5">
                    <div className="w-3 h-3 rounded-full bg-gray-700" />
                    <div className="w-3 h-3 rounded-full bg-gray-700" />
                    <div className="w-3 h-3 rounded-full bg-gray-700" />
                  </div>
                </div>
                <pre className="text-xs leading-loose">
                  <span className="text-pink-400">import</span> {'{'} Body, Button, Container, Head {'}'} <span className="text-pink-400">from</span> <span className="text-green-400">'@react-email/components'</span>;<br/>
                  <span className="text-pink-400">import</span> * <span className="text-pink-400">as</span> React <span className="text-pink-400">from</span> <span className="text-green-400">'react'</span>;<br/>
                  <br/>
                  <span className="text-blue-400">const</span> WelcomeEmail = ({'{'}<br/>
                  &nbsp;&nbsp;username = <span className="text-green-400">'Steve'</span>,<br/>
                  &nbsp;&nbsp;company = <span className="text-green-400">'ACME'</span><br/>
                  {'}'}: WelcomeEmailProps) {`=>`} {'{'}<br/>
                  &nbsp;&nbsp;<span className="text-blue-400">const</span> previewText = <span className="text-green-400">`Welcome to {'${company}'}, {'${username}'}!`</span>;<br/>
                  <br/>
                  &nbsp;&nbsp;<span className="text-pink-400">return</span> (<br/>
                  &nbsp;&nbsp;&nbsp;&nbsp;{`<Html>`}<br/>
                  &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;{`<Head />`}<br/>
                  &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;{`<Preview>{previewText}</Preview>`}<br/>
                  &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;{`<Tailwind>`}<br/>
                  &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;{`<Body className="bg-white my-auto mx-auto font-sans">`}<br/>
                  &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;{`<Container className="my-10 mx-auto p-5 w-[465px]">`}<br/>
                </pre>
              </div>
            </div>

            {/* AI Agent Mockup */}
            <div className="bg-[#111] border border-gray-800 rounded-3xl p-8 relative overflow-hidden flex flex-col h-[500px]">
               <div className="absolute inset-0 bg-gradient-to-tr from-gray-900 to-transparent pointer-events-none" />
               <div className="relative z-10 flex-1 flex flex-col">
                  {/* Floating AI Input */}
                  <div className="bg-gray-900 border border-gray-700 rounded-xl p-4 shadow-xl mb-auto">
                    <p className="text-gray-400 text-sm mb-12">Plan, search, build anything...</p>
                    <div className="flex justify-between items-center">
                      <div className="flex items-center gap-2 text-xs text-gray-500 font-medium bg-gray-800 px-3 py-1.5 rounded-md">
                        <Zap size={12} /> GPT-4o
                      </div>
                      <button className="text-xs text-gray-400 hover:text-white transition-colors">Send ↵</button>
                    </div>
                  </div>

                  <div className="mt-auto">
                    <h4 className="text-4xl font-bold tracking-tighter text-gray-800 mb-6 font-mono opacity-50">CLAUDE<br/>CODE</h4>
                    <div className="space-y-3 font-mono text-sm">
                      <div className="text-gray-500">{`> Welcome to Claude Code research preview!`}</div>
                      <div className="text-gray-600">{`> Ask our agent...`}</div>
                    </div>
                  </div>
               </div>
            </div>
          </div>

          <div className="mt-16 flex items-center justify-between border-t border-gray-900 pt-8">
            <div>
              <h4 className="font-medium text-white mb-2">Prompt at the speed of thought</h4>
              <p className="text-gray-500 text-sm max-w-md leading-relaxed">
                Wispr turns natural, rambling speech into precise prompts — letting you build faster without stopping to edit your thoughts.
              </p>
            </div>
            <div className="hidden sm:flex items-center gap-3 text-sm text-gray-500">
              Hold <span className="bg-gray-800 text-gray-300 px-3 py-1 rounded-md border border-gray-700 font-mono text-xs">Space</span> and try yourself
            </div>
          </div>
        </div>
      </section>

      {/* Productivity Section (Light) */}
      <section className="py-32 px-6 bg-white border-t border-gray-100">
        <div className="max-w-6xl mx-auto">
          <div className="mb-16 max-w-2xl">
            <h3 className="text-gray-500 font-medium mb-4 tracking-wide">Productivity</h3>
            <h2 className="text-4xl md:text-5xl font-medium tracking-tight mb-6 text-gray-900">
              Clearer messages.<br/>Faster updates. Less effort.
            </h2>
            <p className="text-gray-500 text-lg font-light leading-relaxed">
              Speak your updates, and Wispr turns them into polished messages, summaries, and replies across all your favorite tools.
            </p>
          </div>

          <div className="grid grid-cols-1 lg:grid-cols-12 gap-6">
            {/* Chat App Mockup (Spans 8 cols) */}
            <div className="lg:col-span-8 bg-gray-50 border border-gray-200 rounded-3xl p-6 relative h-[500px] flex flex-col">
              <div className="flex gap-4 mb-6 border-b border-gray-200 pb-4">
                <div className="flex gap-1.5 items-center">
                  <div className="w-3 h-3 rounded-full bg-gray-300" />
                  <div className="w-3 h-3 rounded-full bg-gray-300" />
                  <div className="w-3 h-3 rounded-full bg-gray-300" />
                </div>
              </div>

              <div className="flex-1 overflow-hidden flex flex-col gap-6 relative">
                {/* Date separator */}
                <div className="flex items-center gap-4">
                  <div className="h-px bg-gray-200 flex-1" />
                  <span className="text-xs font-medium text-gray-400 px-2">Today</span>
                  <div className="h-px bg-gray-200 flex-1" />
                </div>

                {/* Message 1 */}
                <div className="flex gap-4">
                  <div className="w-10 h-10 rounded bg-orange-200 flex-shrink-0" />
                  <div>
                    <div className="flex items-baseline gap-2 mb-1">
                      <span className="font-semibold text-gray-900">robert</span>
                      <span className="text-xs text-gray-400">10:19 AM</span>
                    </div>
                    <p className="text-gray-700 text-sm leading-relaxed">
                      Morning team! 👋 This is the final week for the Symphony project, so let's make sure everything is on track. <span className="text-blue-500 font-medium">@toni</span> could you share a quick status update with us?
                    </p>
                  </div>
                </div>

                {/* Message 2 */}
                <div className="flex gap-4">
                  <div className="w-10 h-10 rounded bg-blue-200 flex-shrink-0" />
                  <div>
                    <div className="flex items-baseline gap-2 mb-1">
                      <span className="font-semibold text-gray-900">toni</span>
                      <span className="text-xs text-gray-400">10:21 AM</span>
                    </div>
                    <p className="text-gray-700 text-sm leading-relaxed mb-2">
                      Morning Robert ☀️ Thanks for checking in. The Symphony project is progressing well — most tasks are on track.
                    </p>
                    <p className="text-gray-700 text-sm leading-relaxed">
                      We're wrapping up final testing and documentation this week.<br/>
                      I'll flag any blockers right away, but so far things look good.
                    </p>
                  </div>
                </div>

                {/* Input Area */}
                <div className="absolute bottom-0 left-0 right-0 bg-white border-2 border-blue-400 rounded-xl p-3 shadow-lg">
                  <p className="text-gray-400 text-sm mb-4">Message to #Acme...</p>
                  <div className="w-16 h-1.5 bg-gray-800 rounded-full mx-auto" />
                </div>
              </div>
            </div>

            {/* Sidebar Mockup (Spans 4 cols) */}
            <div className="lg:col-span-4 bg-gray-50 border border-gray-200 rounded-3xl p-6 h-[500px] flex flex-col gap-4">
              <div className="h-10 bg-white border border-gray-200 rounded-lg w-full mb-4" />
              {[1, 2, 3, 4].map((i) => (
                <div key={i} className="flex gap-3 items-center">
                  <div className="w-10 h-10 rounded-full bg-gray-200" />
                  <div className="flex-1 space-y-2">
                    <div className="h-3 bg-gray-200 rounded w-1/2" />
                    <div className="h-2 bg-gray-200 rounded w-3/4" />
                  </div>
                </div>
              ))}
              
              <div className="mt-auto p-4 bg-blue-50 border border-blue-100 rounded-xl">
                <p className="text-blue-600 text-xs font-medium">Draft to Mikal Robbins</p>
              </div>
            </div>
          </div>
        </div>
      </section>

      {/* Setup / Onboarding Widget Section */}
      <section className="py-32 px-6 bg-[#0a0a0a] text-white relative overflow-hidden">
        <div className="absolute inset-0 z-0">
          <div className="absolute top-0 right-0 w-full h-full bg-gradient-to-b from-[#111] to-[#0a0a0a]" />
          <div className="absolute top-1/4 right-1/4 w-96 h-96 bg-blue-500/10 rounded-full blur-[120px] pointer-events-none" />
        </div>

        <div className="max-w-6xl mx-auto relative z-10">
          <div className="text-center mb-20">
            <h2 className="text-3xl md:text-5xl font-medium tracking-tight mb-6 text-white">
              Setup is seamlessly native
            </h2>
            <p className="text-gray-400 max-w-xl mx-auto text-lg font-light">
              We built Wispr Bridge to feel like a native macOS application. Connect your microphone and start transcribing in seconds.
            </p>
          </div>

          {/* Embedding the Setup Wizard we built earlier */}
          <div className="scale-[0.85] md:scale-100 origin-top">
             <SetupWizard />
          </div>
        </div>
      </section>

      {/* Footer */}
      <footer className="bg-white border-t border-gray-100 py-12 px-6">
        <div className="max-w-6xl mx-auto flex flex-col md:flex-row justify-between items-center gap-6">
          <div className="flex items-center gap-2">
            <div className="w-5 h-5 rounded-full bg-blue-500 flex items-center justify-center">
              <Mic size={12} className="text-white" />
            </div>
            <span className="font-semibold text-gray-900">WISPR</span>
          </div>
          <div className="flex gap-8 text-sm font-medium text-gray-500">
            <a href="#" className="hover:text-black transition-colors">Privacy</a>
            <a href="#" className="hover:text-black transition-colors">Terms</a>
            <a href="#" className="hover:text-black transition-colors">Twitter</a>
            <a href="#" className="hover:text-black transition-colors">GitHub</a>
          </div>
        </div>
      </footer>
    </div>
  );
}