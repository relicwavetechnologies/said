import { useCallback, useEffect, useRef, useState } from "react";
import { getRecordingAudioUrl } from "@/lib/invoke";

/**
 * Shared audio-playback hook for recording rows. Returns the currently-playing
 * recording id (null when idle), a `play(id, audioId)` toggler that pauses
 * if the same id is already playing, and a `stop()` for hard-stops.
 */
export function useAudioPlayer() {
  const audioRef                  = useRef<HTMLAudioElement | null>(null);
  const blobUrlRef                = useRef<string | null>(null);
  const [playingId, setPlayingId] = useState<string | null>(null);

  // Cleanup on unmount
  useEffect(() => () => {
    audioRef.current?.pause();
    audioRef.current = null;
    if (blobUrlRef.current) {
      URL.revokeObjectURL(blobUrlRef.current);
      blobUrlRef.current = null;
    }
  }, []);

  const play = useCallback(async (recordingId: string, audioId: string | null) => {
    if (!audioId) return;

    // Toggle off if the same row is already playing
    if (playingId === recordingId) {
      audioRef.current?.pause();
      audioRef.current = null;
      setPlayingId(null);
      if (blobUrlRef.current) {
        URL.revokeObjectURL(blobUrlRef.current);
        blobUrlRef.current = null;
      }
      return;
    }

    // Stop any other in-progress playback
    if (audioRef.current) {
      audioRef.current.pause();
      audioRef.current = null;
    }
    if (blobUrlRef.current) {
      URL.revokeObjectURL(blobUrlRef.current);
      blobUrlRef.current = null;
    }

    const ep = await getRecordingAudioUrl(recordingId);
    if (!ep) return;

    try {
      const res = await fetch(ep.url, {
        headers: { Authorization: `Bearer ${ep.secret}` },
      });
      if (!res.ok) return;
      const blob  = await res.blob();
      const url   = URL.createObjectURL(blob);
      const audio = new Audio(url);
      audioRef.current   = audio;
      blobUrlRef.current = url;
      setPlayingId(recordingId);
      audio.play();
      audio.onended = () => {
        setPlayingId(null);
        if (blobUrlRef.current) {
          URL.revokeObjectURL(blobUrlRef.current);
          blobUrlRef.current = null;
        }
        audioRef.current = null;
      };
    } catch {
      setPlayingId(null);
    }
  }, [playingId]);

  const stop = useCallback(() => {
    audioRef.current?.pause();
    audioRef.current = null;
    if (blobUrlRef.current) {
      URL.revokeObjectURL(blobUrlRef.current);
      blobUrlRef.current = null;
    }
    setPlayingId(null);
  }, []);

  return { playingId, play, stop };
}
