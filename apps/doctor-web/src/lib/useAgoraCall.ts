import { useCallback, useEffect, useRef, useState } from 'react';
import { AgoraCallSession, CallPhase } from './agora-client';

export interface AgoraCallState {
  phase: CallPhase | 'idle';
  micEnabled: boolean;
  cameraEnabled: boolean;
  screenSharing: boolean;
  remotePresent: boolean;
  error: string;
  /** Sanitized stage/code line for support; never contains tokens or clinical data. */
  diagnostic: string;
}

/** One call session per `appointmentId`: switching to a different
 * appointment (or to none) tears down the previous session before anything
 * else happens, so a Doctor can never end up publishing into, or still
 * listening on, the wrong appointment's channel. */
export function useAgoraCall(appointmentId: string | null, opensAt?: string) {
  const [state, setState] = useState<AgoraCallState>({ phase: 'idle', micEnabled: true, cameraEnabled: true, screenSharing: false, remotePresent: false, error: '', diagnostic: '' });
  const opensAtRef = useRef<string | undefined>(opensAt);
  opensAtRef.current = opensAt;
  const sessionRef = useRef<AgoraCallSession | null>(null);
  const localVideoRef = useRef<HTMLDivElement | null>(null);
  const remoteVideoRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    setState({ phase: 'idle', micEnabled: true, cameraEnabled: true, screenSharing: false, remotePresent: false, error: '', diagnostic: '' });
    return () => { void sessionRef.current?.leave(); sessionRef.current = null; };
  }, [appointmentId]);

  const ensureSession = useCallback((): AgoraCallSession | null => {
    if (!appointmentId) return null;
    if (sessionRef.current) return sessionRef.current;
    const session = new AgoraCallSession(appointmentId, {
      onPhaseChange: phase => setState(previous => ({ ...previous, phase, error: phase === 'failed' ? previous.error : '' })),
      onRemoteUserChange: present => setState(previous => ({ ...previous, remotePresent: present })),
      onMicStateChange: enabled => setState(previous => ({ ...previous, micEnabled: enabled })),
      onCameraStateChange: enabled => setState(previous => ({ ...previous, cameraEnabled: enabled })),
      onScreenShareChange: active => setState(previous => ({ ...previous, screenSharing: active })),
      onError: message => setState(previous => ({ ...previous, error: message })),
      onDiagnostic: diagnostic => setState(previous => ({ ...previous, diagnostic })),
    });
    sessionRef.current = session;
    if (localVideoRef.current) session.setLocalVideoElement(localVideoRef.current);
    if (remoteVideoRef.current) session.setRemoteVideoElement(remoteVideoRef.current);
    return session;
  }, [appointmentId]);

  const join = useCallback(() => {
    const session = ensureSession();
    if (!session) return;
    session.opensAt = opensAtRef.current;
    setState(previous => ({ ...previous, phase: 'connecting', error: '', diagnostic: '' }));
    void session.join();
  }, [ensureSession]);

  const leave = useCallback(() => {
    const session = sessionRef.current;
    sessionRef.current = null;
    void session?.leave();
  }, []);

  const toggleMic = useCallback(() => { void sessionRef.current?.setMicEnabled(!state.micEnabled); }, [state.micEnabled]);
  const toggleCamera = useCallback(() => { void sessionRef.current?.setCameraEnabled(!state.cameraEnabled); }, [state.cameraEnabled]);
  const toggleScreenShare = useCallback(() => { void sessionRef.current?.toggleScreenShare(); }, []);

  const localVideoCallback = useCallback((el: HTMLDivElement | null) => {
    localVideoRef.current = el;
    sessionRef.current?.setLocalVideoElement(el);
  }, []);
  const remoteVideoCallback = useCallback((el: HTMLDivElement | null) => {
    remoteVideoRef.current = el;
    sessionRef.current?.setRemoteVideoElement(el);
  }, []);

  return { state, join, leave, toggleMic, toggleCamera, toggleScreenShare, localVideoRef: localVideoCallback, remoteVideoRef: remoteVideoCallback };
}
