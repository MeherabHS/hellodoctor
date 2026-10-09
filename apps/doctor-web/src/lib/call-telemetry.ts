// Adapter boundary for real Agora SDK callbacks. Never infer a joined call from
// opening the Telehealth page or token issuance; no SDK is integrated yet.
export type CallEvent = 'CALL_INITIATED'|'DOCTOR_JOINED'|'DOCTOR_RECONNECTING'|'DOCTOR_RECONNECTED'|'DOCTOR_DISCONNECTED'|'NETWORK_DEGRADED'|'NETWORK_RESTORED'|'CALL_ENDED';
export async function deliverCallTelemetry(appointment: string, event: CallEvent,
  submit: (path:string,body:unknown)=>Promise<unknown>, measurements?:{rtt_ms?:number;packet_loss_percent?:number}):Promise<boolean> {
  if (!/^[0-9a-f-]{36}$/i.test(appointment) || !['CALL_INITIATED','DOCTOR_JOINED','DOCTOR_RECONNECTING','DOCTOR_RECONNECTED','DOCTOR_DISCONNECTED','NETWORK_DEGRADED','NETWORK_RESTORED','CALL_ENDED'].includes(event)) return false;
  const {rtt_ms,packet_loss_percent}=measurements??{};
  if ((rtt_ms!==undefined&&(!Number.isInteger(rtt_ms)||rtt_ms<0)) || (packet_loss_percent!==undefined&&(!Number.isFinite(packet_loss_percent)||packet_loss_percent<0||packet_loss_percent>100))) return false;
  try {
    await submit(`consultations/${appointment}/telemetry/events`,{events:[{event_type:event,rtt_ms,packet_loss_percent,occurred_at:new Date().toISOString()}]});
    return true;
  } catch { return false; }
}
