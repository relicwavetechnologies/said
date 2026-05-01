"use client";

import React, { useState } from "react";
import { 
  Settings, 
  Mic, 
  RefreshCw, 
  Volume2, 
  Check, 
  Download,
  Terminal
} from "lucide-react";
import { motion, AnimatePresence } from "framer-motion";

export default function SetupWizard() {
  const [activeTab, setActiveTab] = useState<"features" | "setup" | "download">("features");
  const [startAtLogin, setStartAtLogin] = useState(true);

  return (
    <div className="flex items-center justify-center p-4 sm:p-8 relative overflow-hidden">
      {/* Background decorations */}
      <div className="absolute top-[-20%] left-[-10%] w-[50%] h-[50%] bg-purple-500/10 blur-[120px] rounded-full pointer-events-none" />
      <div className="absolute bottom-[-20%] right-[-10%] w-[50%] h-[50%] bg-blue-500/10 blur-[120px] rounded-full pointer-events-none" />

      {/* Main Mac Window */}
      <motion.div 
        initial={{ opacity: 0, y: 20, scale: 0.98 }}
        animate={{ opacity: 1, y: 0, scale: 1 }}
        transition={{ duration: 0.5, ease: "easeOut" }}
        className="w-full max-w-5xl bg-white rounded-2xl shadow-2xl overflow-hidden flex flex-col relative text-black"
        style={{ minHeight: "650px" }}
      >
        {/* Window Controls */}
        <div className="flex gap-2 p-4 absolute top-0 left-0 z-10">
          <div className="w-3 h-3 rounded-full bg-[#ff5f56] border border-[#e0443e]"></div>
          <div className="w-3 h-3 rounded-full bg-[#ffbd2e] border border-[#dea123]"></div>
          <div className="w-3 h-3 rounded-full bg-[#27c93f] border border-[#1aab29]"></div>
        </div>

        {/* Content Split */}
        <div className="flex flex-col md:flex-row flex-1 mt-10">
          
          {/* Left Panel - Interaction */}
          <div className="w-full md:w-1/2 p-10 flex flex-col">
            <h1 className="text-3xl font-semibold tracking-tight mb-2">Wispr Bridge</h1>
            <p className="text-gray-500 mb-10">The ultimate voice-to-text bridge for Next.js applications.</p>

            <div className="flex flex-wrap gap-3 mb-10">
              <Pill 
                text="Features" 
                active={activeTab === "features"} 
                onClick={() => setActiveTab("features")} 
              />
              <Pill 
                text="Setup" 
                active={activeTab === "setup"} 
                onClick={() => setActiveTab("setup")} 
              />
              <Pill 
                text="Download" 
                active={activeTab === "download"} 
                onClick={() => setActiveTab("download")} 
              />
            </div>

            <div className="flex-1 relative">
              <AnimatePresence mode="wait">
                {activeTab === "features" && (
                  <motion.div
                    key="features"
                    initial={{ opacity: 0, x: -10 }}
                    animate={{ opacity: 1, x: 0 }}
                    exit={{ opacity: 0, x: 10 }}
                    className="flex flex-col gap-4"
                  >
                    <FeatureCard 
                      icon={Mic} 
                      title="Crystal Clear Audio" 
                      desc="High-fidelity voice capture with noise cancellation." 
                    />
                    <FeatureCard 
                      icon={Terminal} 
                      title="Developer First" 
                      desc="Simple SDK for seamless integration into Next.js." 
                    />
                    <FeatureCard 
                      icon={Settings} 
                      title="Customizable" 
                      desc="Tweak every aspect of the speech-to-text pipeline." 
                    />
                  </motion.div>
                )}

                {activeTab === "setup" && (
                  <motion.div
                    key="setup"
                    initial={{ opacity: 0, x: -10 }}
                    animate={{ opacity: 1, x: 0 }}
                    exit={{ opacity: 0, x: 10 }}
                    className="flex flex-col gap-6"
                  >
                    <div className="bg-gray-50 border border-gray-100 rounded-2xl p-6 flex items-start gap-4 shadow-sm">
                      <div className="bg-white p-2 rounded-full shadow-sm border border-gray-100 mt-1">
                        <Settings className="w-5 h-5 text-gray-600" />
                      </div>
                      <div className="flex-1">
                        <h3 className="font-semibold text-gray-900 mb-1">Allow Wispr to process audio</h3>
                        <p className="text-sm text-gray-500 mb-4">This requires Microphone permissions.</p>
                        <div className="flex justify-end">
                          <button className="px-5 py-2 rounded-full border border-gray-200 text-sm font-medium hover:bg-gray-50 transition-colors">
                            Allow
                          </button>
                        </div>
                      </div>
                    </div>

                    <div className="flex items-center justify-between px-2 mt-4">
                      <span className="font-medium text-gray-900">Start at Login (Recommended)</span>
                      <Toggle enabled={startAtLogin} setEnabled={setStartAtLogin} />
                    </div>
                  </motion.div>
                )}

                {activeTab === "download" && (
                  <motion.div
                    key="download"
                    initial={{ opacity: 0, x: -10 }}
                    animate={{ opacity: 1, x: 0 }}
                    exit={{ opacity: 0, x: 10 }}
                    className="flex flex-col gap-4 items-center justify-center h-full text-center"
                  >
                    <div className="w-16 h-16 bg-gray-50 rounded-full flex items-center justify-center mb-4 border border-gray-100">
                      <Download className="w-8 h-8 text-black" />
                    </div>
                    <h3 className="text-xl font-semibold mb-2">Get Wispr Bridge</h3>
                    <p className="text-gray-500 mb-8">Available for macOS 12.0 and later.</p>
                    <button className="bg-black text-white px-8 py-3 rounded-full font-medium hover:bg-gray-800 transition-colors flex items-center gap-2">
                      Download for Mac
                    </button>
                    <button className="mt-4 text-gray-400 hover:text-gray-600 text-sm font-medium transition-colors">
                      Other platforms
                    </button>
                  </motion.div>
                )}
              </AnimatePresence>
            </div>

            {/* Footer Action */}
            <div className="mt-10 flex justify-end items-center">
              <button className="bg-black text-white px-6 py-2.5 rounded-full font-medium hover:bg-gray-800 transition-colors flex items-center gap-2">
                Continue
              </button>
            </div>
          </div>

          {/* Right Panel - Illustration / Visuals */}
          <div className="w-full md:w-1/2 bg-[#fafafa] border-l border-gray-100 p-10 flex flex-col items-center justify-center relative">
            
            <AnimatePresence mode="wait">
              {activeTab === "features" && (
                <motion.div
                  key="right-features"
                  initial={{ opacity: 0 }}
                  animate={{ opacity: 1 }}
                  exit={{ opacity: 0 }}
                  className="w-full max-w-sm"
                >
                  <div className="bg-white rounded-xl shadow-sm border border-gray-100 p-6">
                    <div className="flex justify-between items-center mb-6">
                      <span className="text-sm font-medium text-gray-500">Pick your mic</span>
                      <span className="text-xs text-gray-400">Wispr Settings</span>
                    </div>
                    
                    <div className="border border-gray-200 rounded-xl p-4 flex flex-col gap-4">
                      <div className="flex items-center gap-2 text-sm font-medium text-gray-700">
                        <Mic size={16} /> Microphone
                      </div>
                      <div className="flex items-center gap-2">
                        <select className="flex-1 bg-gray-50 border border-gray-200 rounded-lg px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-black">
                          <option>Default (MacBook Pro Mic)</option>
                          <option>External USB Mic</option>
                        </select>
                        <button className="p-2 border border-gray-200 rounded-lg hover:bg-gray-50 flex items-center gap-2 text-sm font-medium">
                          <Volume2 size={16} /> Test
                        </button>
                        <button className="p-2 border border-gray-200 rounded-lg hover:bg-gray-50">
                          <RefreshCw size={16} className="text-gray-500" />
                        </button>
                      </div>
                    </div>
                  </div>
                </motion.div>
              )}

              {activeTab === "setup" && (
                <motion.div
                  key="right-setup"
                  initial={{ opacity: 0 }}
                  animate={{ opacity: 1 }}
                  exit={{ opacity: 0 }}
                  className="w-full max-w-sm"
                >
                  {/* Mocking the Mac Accessibility screen from the Aqua Voice design */}
                  <div className="bg-white rounded-xl shadow-lg border border-gray-100 overflow-hidden text-xs">
                    <div className="bg-gray-100 p-3 flex items-center gap-2 border-b border-gray-200">
                      <div className="flex gap-1.5">
                        <div className="w-2.5 h-2.5 rounded-full bg-[#ff5f56]"></div>
                        <div className="w-2.5 h-2.5 rounded-full bg-[#ffbd2e]"></div>
                        <div className="w-2.5 h-2.5 rounded-full bg-[#27c93f]"></div>
                      </div>
                      <div className="flex-1 text-center font-medium text-gray-600">Accessibility</div>
                    </div>
                    <div className="flex h-64">
                      <div className="w-1/3 bg-gray-50 border-r border-gray-200 p-2 flex flex-col gap-1">
                        <div className="p-1.5 rounded bg-gray-200 font-medium text-gray-700 flex items-center gap-2">
                          <div className="w-4 h-4 rounded bg-blue-500 flex items-center justify-center">
                            <Check size={10} className="text-white" />
                          </div>
                          Privacy & Security
                        </div>
                        <div className="p-1.5 rounded text-gray-500 flex items-center gap-2">
                          <Settings size={14} /> General
                        </div>
                      </div>
                      <div className="flex-1 p-4 bg-white">
                        <p className="text-gray-500 mb-4">Allow the applications below to control your computer.</p>
                        <div className="border border-gray-200 rounded-lg p-2 flex items-center justify-between bg-gray-50">
                          <div className="flex items-center gap-2">
                            <div className="w-6 h-6 rounded bg-black flex items-center justify-center">
                              <Mic size={12} className="text-white" />
                            </div>
                            <span className="font-medium text-gray-800">Wispr Bridge</span>
                          </div>
                          <div className="w-8 h-4 bg-blue-500 rounded-full flex items-center justify-end p-0.5">
                            <div className="w-3 h-3 bg-white rounded-full"></div>
                          </div>
                        </div>
                      </div>
                    </div>
                  </div>
                </motion.div>
              )}

              {activeTab === "download" && (
                <motion.div
                  key="right-download"
                  initial={{ opacity: 0 }}
                  animate={{ opacity: 1 }}
                  exit={{ opacity: 0 }}
                  className="w-full flex items-center justify-center"
                >
                  <div className="relative">
                    <div className="absolute inset-0 bg-gradient-to-tr from-blue-100 to-purple-50 rounded-full blur-3xl opacity-50"></div>
                    <pre className="relative bg-gray-900 text-gray-100 p-6 rounded-xl text-sm font-mono shadow-xl border border-gray-800 overflow-x-auto max-w-sm">
                      <code className="text-blue-400">npx</code> create-next-app@latest<br/><br/>
                      <code className="text-gray-500"># Install Wispr Bridge</code><br/>
                      <code className="text-blue-400">npm</code> install @wispr/bridge<br/><br/>
                      <code className="text-gray-500"># Initialize</code><br/>
                      <code className="text-blue-400">npx</code> wispr init
                    </pre>
                  </div>
                </motion.div>
              )}
            </AnimatePresence>

          </div>
        </div>
      </motion.div>
    </div>
  );
}

// Sub-components

function FeatureCard({ icon: Icon, title, desc }: any) {
  return (
    <div className="flex items-start gap-4 p-4 rounded-2xl hover:bg-gray-50 transition-colors border border-transparent hover:border-gray-100 cursor-pointer group">
      <div className="bg-gray-100 p-2.5 rounded-full text-gray-600 group-hover:bg-white group-hover:shadow-sm transition-all">
        <Icon size={20} />
      </div>
      <div>
        <h3 className="font-semibold text-gray-900">{title}</h3>
        <p className="text-sm text-gray-500">{desc}</p>
      </div>
    </div>
  );
}

function Pill({ text, active = false, onClick }: any) {
  return (
    <button 
      onClick={onClick}
      className={`px-4 py-1.5 rounded-full text-sm font-medium transition-all duration-200 border
        ${active 
          ? "bg-black text-white border-black shadow-md" 
          : "bg-white text-gray-600 border-gray-200 hover:bg-gray-50 hover:border-gray-300"
        }`}
    >
      {text}
    </button>
  );
}

function Toggle({ enabled, setEnabled }: { enabled: boolean; setEnabled: (val: boolean) => void }) {
  return (
    <button 
      onClick={() => setEnabled(!enabled)}
      className={`w-11 h-6 rounded-full flex items-center p-1 transition-colors duration-300 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-black
        ${enabled ? "bg-black" : "bg-gray-200"}`}
    >
      <div 
        className={`w-4 h-4 rounded-full bg-white shadow-sm transform transition-transform duration-300
          ${enabled ? "translate-x-5" : "translate-x-0"}`} 
      />
    </button>
  );
}
