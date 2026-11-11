// Real Agora Web SDK call lifecycle, filling the adapter boundary that
// call-telemetry.ts's own comment names ("no SDK is integrated yet"). Mirrors
// apps/mobile's CallController/AgoraEngineAdapter one-to-one: same backend
// endpoint (`consultations/:id/rtc-token`), same server-assigned channel
// name/uid/token (never derived client-side), same token-renewal-on-expiry
// and connection-state-driven reconnection handling, same CallEvent
// telemetry vocabulary. The Agora SDK itself is dynamically imported only
// when a call actually starts — it touches browser-only APIs
// (getUserMedia/RTCPeerConnection) that must never run during Next.js SSR,
// and it is a large bundle not worth loading for every Doctor Web page.
import type {
  IAgoraRTCClient,
  IAgoraRTCRemoteUser,
  ICameraVideoTrack,
  ILocalVideoTrack,
  IMicrophoneAudioTrack,
} from 'agora-rtc-sdk-ng';
import { ApiError, api, errorText, recordCallTelemetry } from './client';
import { classifyAgoraErrorCode } from './agora-errors';
import type { RtcTokenData } from './contracts';

export type CallPhase = 'connecting' | 'waiting-for-remote' | 'connected' | 'reconnecting' | 'failed' | 'ended';

export interface AgoraCallHandlers {
  onPhaseChange: (phase: CallPhase) => void;
  onRemoteUserChange: (present: boolean) => void;
  onMicStateChange: (enabled: boolean) => void;
  onCameraStateChange: (enabled: boolean) => void;
  onScreenShareChange: (active: boolean) => void;
  onError: (message: string) => void;
}

function describeError(error: unknown): string {
  const code = error && typeof error === 'object' && 'code' in error ? String((error as { code: unknown }).code) : '';
  const classified = classifyAgoraErrorCode(code);
  if (classified) return classified;
  if (error instanceof ApiError) return errorText(error);
  return 'Unable to connect. Check your network and retry.';
}

/** One call's full lifecycle for one appointment. Never reused across a
 * different appointment — the owning hook constructs a fresh instance per
 * `appointment.id`, which is what prevents a duplicate/crossed session when
 * the Doctor switches appointments while already on the Telehealth page. */
export class AgoraCallSession {
  private client: IAgoraRTCClient | null = null;
  private micTrack: IMicrophoneAudioTrack | null = null;
  private cameraTrack: ICameraVideoTrack | null = null;
  private screenTrack: ILocalVideoTrack | null = null;
  private screenSharing = false;
  private remoteUser: IAgoraRTCRemoteUser | null = null;
  private localVideoEl: HTMLElement | null = null;
  private remoteVideoEl: HTMLElement | null = null;
  private joining = false;
  private joined = false;
  private destroyed = false;
  private renewing = false;

  constructor(private readonly appointmentId: string, private readonly handlers: AgoraCallHandlers) {}

  setLocalVideoElement(el: HTMLElement | null): void {
    this.localVideoEl = el;
    if (el && this.cameraTrack) this.cameraTrack.play(el);
  }
  setRemoteVideoElement(el: HTMLElement | null): void {
    this.remoteVideoEl = el;
    if (el && this.remoteUser?.videoTrack) this.remoteUser.videoTrack.play(el);
  }

  private async mintToken(): Promise<RtcTokenData> {
    return api<RtcTokenData>(`consultations/${this.appointmentId}/rtc-token`, { method: 'POST' });
  }

  async join(): Promise<void> {
    if (this.joining || this.joined || this.destroyed) return;
    this.joining = true;
    try {
      const { default: AgoraRTC } = await import('agora-rtc-sdk-ng');
      const session = await this.mintToken();
      if (this.destroyed) return;
      const client = this.client ?? AgoraRTC.createClient({ mode: 'rtc', codec: 'vp8' });
      this.client = client;
      this.registerClientHandlers(client);
      await client.join(session.app_id, session.channel_name, session.token, session.uid);
      if (this.destroyed) { await client.leave().catch(() => undefined); return; }
      const [micTrack, cameraTrack] = await AgoraRTC.createMicrophoneAndCameraTracks();
      if (this.destroyed) { micTrack.close(); cameraTrack.close(); await client.leave().catch(() => undefined); return; }
      this.micTrack = micTrack;
      this.cameraTrack = cameraTrack;
      if (this.localVideoEl) cameraTrack.play(this.localVideoEl);
      await client.publish([micTrack, cameraTrack]);
      this.joined = true;
      this.handlers.onMicStateChange(true);
      this.handlers.onCameraStateChange(true);
      this.handlers.onPhaseChange(this.remoteUser ? 'connected' : 'waiting-for-remote');
      void recordCallTelemetry(this.appointmentId, 'CALL_INITIATED');
    } catch (error) {
      await this.releaseLocalTracks();
      this.handlers.onPhaseChange('failed');
      this.handlers.onError(describeError(error));
    } finally {
      this.joining = false;
    }
  }

  private registerClientHandlers(client: IAgoraRTCClient): void {
    client.on('connection-state-change', curState => {
      if (this.destroyed) return;
      if (curState === 'RECONNECTING') {
        this.handlers.onPhaseChange('reconnecting');
        void recordCallTelemetry(this.appointmentId, 'DOCTOR_RECONNECTING');
      } else if (curState === 'CONNECTED') {
        if (this.joined) {
          this.handlers.onPhaseChange(this.remoteUser ? 'connected' : 'waiting-for-remote');
          void recordCallTelemetry(this.appointmentId, 'DOCTOR_RECONNECTED');
        }
      } else if (curState === 'DISCONNECTED' && this.joined) {
        this.handlers.onPhaseChange('failed');
        void recordCallTelemetry(this.appointmentId, 'DOCTOR_DISCONNECTED');
      }
    });
    client.on('user-published', async (user, mediaType) => {
      if (this.destroyed) return;
      try { await client.subscribe(user, mediaType); } catch { return; }
      this.remoteUser = user;
      if (mediaType === 'video' && user.videoTrack && this.remoteVideoEl) user.videoTrack.play(this.remoteVideoEl);
      if (mediaType === 'audio') user.audioTrack?.play();
      this.handlers.onRemoteUserChange(true);
      if (this.joined) this.handlers.onPhaseChange('connected');
    });
    client.on('user-unpublished', (user, mediaType) => {
      if (mediaType === 'video' && this.remoteUser?.uid === user.uid && this.remoteVideoEl) {
        this.remoteVideoEl.replaceChildren();
      }
    });
    client.on('user-left', user => {
      if (this.remoteUser?.uid !== user.uid) return;
      this.remoteUser = null;
      this.handlers.onRemoteUserChange(false);
      if (this.joined) this.handlers.onPhaseChange('waiting-for-remote');
    });
    client.on('token-privilege-will-expire', () => {
      if (this.renewing || this.destroyed) return;
      this.renewing = true;
      this.mintToken()
        .then(fresh => client.renewToken(fresh.token))
        .catch(() => { /* swallowed by design — a genuine failure surfaces honestly through connection-state-change, same as apps/mobile's CallController */ })
        .finally(() => { this.renewing = false; });
    });
  }

  async setMicEnabled(enabled: boolean): Promise<void> {
    if (!this.micTrack) return;
    await this.micTrack.setEnabled(enabled);
    this.handlers.onMicStateChange(enabled);
  }

  async setCameraEnabled(enabled: boolean): Promise<void> {
    if (!this.cameraTrack) return;
    await this.cameraTrack.setEnabled(enabled);
    this.handlers.onCameraStateChange(enabled);
  }

  async toggleScreenShare(): Promise<void> {
    if (!this.client || !this.cameraTrack) return;
    const { default: AgoraRTC } = await import('agora-rtc-sdk-ng');
    if (this.screenSharing) {
      if (this.screenTrack) {
        await this.client.unpublish(this.screenTrack);
        this.screenTrack.close();
        this.screenTrack = null;
      }
      await this.client.publish(this.cameraTrack);
      if (this.localVideoEl) this.cameraTrack.play(this.localVideoEl);
      this.screenSharing = false;
      this.handlers.onScreenShareChange(false);
      return;
    }
    try {
      const track = await AgoraRTC.createScreenVideoTrack({ encoderConfig: '1080p_1' }, 'disable');
      track.on('track-ended', () => { void this.toggleScreenShare(); });
      await this.client.unpublish(this.cameraTrack);
      await this.client.publish(track);
      if (this.localVideoEl) track.play(this.localVideoEl);
      this.screenTrack = track;
      this.screenSharing = true;
      this.handlers.onScreenShareChange(true);
    } catch (error) {
      this.handlers.onError(describeError(error));
    }
  }

  private async releaseLocalTracks(): Promise<void> {
    this.cameraTrack?.close();
    this.micTrack?.close();
    this.screenTrack?.close();
    this.cameraTrack = null;
    this.micTrack = null;
    this.screenTrack = null;
    this.screenSharing = false;
  }

  /** Idempotent: safe to call more than once (unmount + explicit "End Call"
   * both call this), matching apps/mobile's `CallController.leave()`. */
  async leave(): Promise<void> {
    if (this.destroyed) return;
    this.destroyed = true;
    const wasJoined = this.joined;
    this.joined = false;
    const client = this.client;
    if (client) {
      try { await client.unpublish(); } catch { /* best-effort cleanup */ }
    }
    await this.releaseLocalTracks();
    if (client) {
      try { await client.leave(); } catch { /* best-effort cleanup */ }
    }
    if (wasJoined) void recordCallTelemetry(this.appointmentId, 'CALL_ENDED');
    this.handlers.onPhaseChange('ended');
  }
}
