'use client';
import { useCallback, useEffect, useMemo, useState } from 'react';
import {
  Campaign,
  CampaignAudience,
  CampaignContent,
  CampaignStats,
  CampaignStatus,
  AdminLog,
  approveCampaign,
  cancelCampaign,
  createCampaign,
  getAdminLogs,
  getCampaign,
  getCampaignAudiencePreview,
  getCampaignStats,
  listCampaigns,
  pauseCampaign,
  resumeCampaign,
  scheduleCampaign,
  submitCampaign,
  updateCampaign,
} from '@/lib/api';
import { CampaignNotificationPreview } from './CampaignNotificationPreview';
import { CAMPAIGN_BODY_MAX, CAMPAIGN_TITLE_MAX, campaignFieldErrors } from '@/lib/campaignValidation';

// Kenya has a single fixed UTC+3 offset year-round (no DST), so an
// Africa/Nairobi wall-clock time entered in the schedule picker converts to
// an authoritative UTC instant by appending this literal offset — no
// timezone library, and no dependency on the Admin's own browser timezone.
const NAIROBI_OFFSET = '+03:00';

const STATUS_LABEL: Record<CampaignStatus, string> = {
  DRAFT: 'Draft',
  PENDING_REVIEW: 'Pending review',
  APPROVED: 'Approved',
  SCHEDULED: 'Scheduled',
  SENDING: 'Sending',
  PAUSED: 'Paused',
  COMPLETED: 'Completed',
  CANCELLED: 'Cancelled',
};
const STATUS_BADGE: Record<CampaignStatus, string> = {
  DRAFT: 'badge-slate',
  PENDING_REVIEW: 'badge-amber',
  APPROVED: 'badge-blue',
  SCHEDULED: 'badge-blue',
  SENDING: 'badge-amber',
  PAUSED: 'badge-amber',
  COMPLETED: 'badge-green',
  CANCELLED: 'badge-rose',
};

function emptyContent(): CampaignContent {
  return { title_en: '', title_sw: '', body_en: '', body_sw: '', audience: 'PATIENTS' };
}

function errorMessage(e: unknown): string {
  return e instanceof Error ? e.message : 'Something went wrong. Please retry.';
}


/// Admin Campaign Manager — Milestones 6B + 6C. Dashboard/list, create/edit
/// draft, English/Kiswahili content, audience selector with a PII-safe
/// eligible-count preview, schedule now/later, submit/approve, pause/resume/
/// cancel, delivery statistics, and campaign-scoped audit history. Dispatch
/// itself is entirely backend (the scheduler's fan-out pass); this component
/// never sends anything — only ever changes a campaign's own lifecycle state.
export function CampaignManager() {
  const [campaigns, setCampaigns] = useState<Campaign[]>([]);
  const [listLoading, setListLoading] = useState(true);
  const [listError, setListError] = useState('');
  const [statusFilter, setStatusFilter] = useState<CampaignStatus | 'ALL'>('ALL');

  const [selectedId, setSelectedId] = useState<string | 'new' | null>(null);
  const [selected, setSelected] = useState<Campaign | null>(null);
  const [detailLoading, setDetailLoading] = useState(false);
  const [detailError, setDetailError] = useState('');
  const [content, setContent] = useState<CampaignContent>(emptyContent());
  const [creationKey, setCreationKey] = useState('');
  const [touched, setTouched] = useState(false);

  const [audiencePreview, setAudiencePreview] = useState<number | null>(null);
  const [audiencePreviewLoading, setAudiencePreviewLoading] = useState(false);

  const [busy, setBusy] = useState(false);
  const [actionError, setActionError] = useState('');
  const [scheduleLocal, setScheduleLocal] = useState('');

  const [stats, setStats] = useState<CampaignStats | null>(null);
  const [statsLoading, setStatsLoading] = useState(false);
  const [statsError, setStatsError] = useState('');

  const [auditLogs, setAuditLogs] = useState<AdminLog[] | null>(null);
  const [auditError, setAuditError] = useState('');
  const [auditOpen, setAuditOpen] = useState(false);

  const loadList = useCallback(async () => {
    setListLoading(true);
    setListError('');
    try {
      const rows = await listCampaigns(statusFilter === 'ALL' ? undefined : statusFilter);
      setCampaigns(rows);
    } catch (e) {
      setListError(errorMessage(e));
    } finally {
      setListLoading(false);
    }
  }, [statusFilter]);

  useEffect(() => {
    void loadList();
  }, [loadList]);

  const loadAudiencePreview = useCallback(async (audience: CampaignAudience) => {
    setAudiencePreviewLoading(true);
    try {
      const result = await getCampaignAudiencePreview(audience);
      setAudiencePreview(result.eligible_count);
    } catch {
      setAudiencePreview(null);
    } finally {
      setAudiencePreviewLoading(false);
    }
  }, []);

  function startNewCampaign() {
    setSelectedId('new');
    setSelected(null);
    setContent(emptyContent());
    setCreationKey(crypto.randomUUID());
    setTouched(false);
    setActionError('');
    setDetailError('');
    setStats(null);
    setAuditLogs(null);
    setAuditOpen(false);
    void loadAudiencePreview('PATIENTS');
  }

  const openCampaign = useCallback(
    async (id: string) => {
      setSelectedId(id);
      setDetailLoading(true);
      setDetailError('');
      setActionError('');
      setStats(null);
      setAuditLogs(null);
      setAuditOpen(false);
      try {
        const campaign = await getCampaign(id);
        setSelected(campaign);
        setContent({
          title_en: campaign.title_en,
          title_sw: campaign.title_sw,
          body_en: campaign.body_en,
          body_sw: campaign.body_sw,
          audience: campaign.audience,
        });
        setTouched(false);
        void loadAudiencePreview(campaign.audience);
        if (campaign.status !== 'DRAFT') void loadStats(id);
      } catch (e) {
        setDetailError(errorMessage(e));
      } finally {
        setDetailLoading(false);
      }
    },
    [loadAudiencePreview]
  );

  async function loadStats(id: string) {
    setStatsLoading(true);
    setStatsError('');
    try {
      setStats(await getCampaignStats(id));
    } catch (e) {
      setStatsError(errorMessage(e));
    } finally {
      setStatsLoading(false);
    }
  }

  async function loadAudit(id: string) {
    setAuditOpen(true);
    if (auditLogs) return;
    setAuditError('');
    try {
      setAuditLogs(await getAdminLogs());
    } catch (e) {
      setAuditError(errorMessage(e));
    }
  }

  function updateField<K extends keyof CampaignContent>(key: K, value: CampaignContent[K]) {
    setTouched(true);
    setContent((previous) => ({ ...previous, [key]: value }));
    if (key === 'audience') void loadAudiencePreview(value as CampaignAudience);
  }

  const validationErrors = useMemo(() => campaignFieldErrors(content), [content]);
  const canEditContent =
    selectedId === 'new' ||
    (selected !== null && ['DRAFT', 'PENDING_REVIEW', 'APPROVED', 'SCHEDULED'].includes(selected.status));

  async function saveDraft() {
    if (validationErrors.length > 0) {
      setTouched(true);
      return;
    }
    setBusy(true);
    setActionError('');
    try {
      if (selectedId === 'new') {
        const created = await createCampaign(creationKey, content);
        setSelected(created);
        setSelectedId(created.id);
      } else if (selected) {
        const updated = await updateCampaign(selected.id, selected.version, content);
        setSelected(updated);
      }
      await loadList();
    } catch (e) {
      setActionError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  }

  async function runTransition(action: (id: string, version: number) => Promise<Campaign>) {
    if (!selected) return;
    setBusy(true);
    setActionError('');
    try {
      const updated = await action(selected.id, selected.version);
      setSelected(updated);
      setContent({
        title_en: updated.title_en,
        title_sw: updated.title_sw,
        body_en: updated.body_en,
        body_sw: updated.body_sw,
        audience: updated.audience,
      });
      await loadList();
      if (updated.status !== 'DRAFT') void loadStats(updated.id);
    } catch (e) {
      setActionError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  }

  /// Combined submit→approve→send-now path for an authorized Admin who does
  /// not need another person's separate approval — campaigns have no
  /// author-based approval restriction on the backend (unlike sensitive
  /// articles), so any Admin holding ManageCampaigns may already approve
  /// their own submission one step at a time; this only collapses that into
  /// one click instead of three separate round trips. Each step re-reads
  /// the freshest id/version the previous step returned, since every
  /// transition is optimistic-concurrency-guarded server-side.
  async function publishCampaignNow() {
    if (!selected) return;
    setBusy(true);
    setActionError('');
    try {
      let updated = await submitCampaign(selected.id, selected.version);
      updated = await approveCampaign(updated.id, updated.version);
      updated = await scheduleCampaign(updated.id, updated.version, new Date().toISOString());
      setSelected(updated);
      setContent({
        title_en: updated.title_en,
        title_sw: updated.title_sw,
        body_en: updated.body_en,
        body_sw: updated.body_sw,
        audience: updated.audience,
      });
      await loadList();
      void loadStats(updated.id);
    } catch (e) {
      setActionError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  }

  async function doSchedule(sendImmediately: boolean) {
    if (!selected) return;
    const scheduledForUtc = sendImmediately
      ? new Date().toISOString()
      : scheduleLocal
        ? new Date(`${scheduleLocal}:00${NAIROBI_OFFSET}`).toISOString()
        : null;
    if (!scheduledForUtc) {
      setActionError('Choose a date and time, or use Send Immediately.');
      return;
    }
    setBusy(true);
    setActionError('');
    try {
      const updated = await scheduleCampaign(selected.id, selected.version, scheduledForUtc);
      setSelected(updated);
      await loadList();
    } catch (e) {
      setActionError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  }

  async function doCancel() {
    if (!selected) return;
    if (!window.confirm('Cancel this campaign? Any not-yet-sent notifications will be removed. This cannot be undone.')) {
      return;
    }
    await runTransition(cancelCampaign);
  }

  const filteredAuditLogs = useMemo(
    () => (selected ? (auditLogs ?? []).filter((l) => l.resource_type === 'CAMPAIGN' && l.resource_id === selected.id) : []),
    [auditLogs, selected]
  );

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 20 }}>
      <div className="admin-card">
        <div className="admin-card-header">
          <div>
            <div className="admin-card-title">Campaign dashboard</div>
            <div className="admin-card-subtitle">Admin-authored push notification campaigns to Patients, Doctors, or both.</div>
          </div>
          <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
            <select
              value={statusFilter}
              onChange={(e) => setStatusFilter(e.target.value as CampaignStatus | 'ALL')}
              style={{ padding: '7px 10px', border: '1px solid #CBD5E1', borderRadius: 8, fontSize: 11.5, fontWeight: 600 }}
            >
              <option value="ALL">All statuses</option>
              {(Object.keys(STATUS_LABEL) as CampaignStatus[]).map((s) => (
                <option key={s} value={s}>{STATUS_LABEL[s]}</option>
              ))}
            </select>
            <button className="admin-btn-pri" onClick={startNewCampaign}>+ New Campaign</button>
          </div>
        </div>

        {listLoading && <p role="status">Loading campaigns…</p>}
        {listError && (
          <div role="alert" style={{ color: '#B91C1C', marginBottom: 10 }}>
            {listError} <button className="admin-btn-sec" onClick={() => void loadList()}>Retry</button>
          </div>
        )}
        {!listLoading && !listError && campaigns.length === 0 && (
          <p style={{ color: '#64748B', fontSize: 12.5 }}>No campaigns yet. Create one to get started.</p>
        )}
        {!listLoading && campaigns.length > 0 && (
          <div style={{ overflowX: 'auto' }}>
            <table className="admin-table">
              <thead>
                <tr>
                  <th>Title (English)</th>
                  <th>Audience</th>
                  <th>Status</th>
                  <th>Updated</th>
                </tr>
              </thead>
              <tbody>
                {campaigns.map((c) => (
                  <tr key={c.id} className="admin-clickable-row" onClick={() => void openCampaign(c.id)}>
                    <td>{c.title_en}</td>
                    <td>{c.audience}</td>
                    <td><span className={`badge ${STATUS_BADGE[c.status]}`}>{STATUS_LABEL[c.status]}</span></td>
                    <td>{new Date(c.updated_at).toLocaleString('en-KE', { timeZone: 'Africa/Nairobi' })}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>

      {selectedId && (
        <div className="admin-card">
          {detailLoading && <p role="status">Loading campaign…</p>}
          {detailError && <p role="alert" style={{ color: '#B91C1C' }}>{detailError}</p>}
          {!detailLoading && (
            <>
              <div className="admin-card-header">
                <div>
                  <div className="admin-card-title">
                    {selectedId === 'new' ? 'New campaign' : selected ? STATUS_LABEL[selected.status] : ''}
                  </div>
                  {selected && (
                    <div className="admin-card-subtitle">
                      Created {new Date(selected.created_at).toLocaleString('en-KE', { timeZone: 'Africa/Nairobi' })}
                      {selected.scheduled_for && ` · Scheduled for ${new Date(selected.scheduled_for).toLocaleString('en-KE', { timeZone: 'Africa/Nairobi' })} (Africa/Nairobi)`}
                    </div>
                  )}
                </div>
                <button className="admin-btn-sec" onClick={() => { setSelectedId(null); setSelected(null); }}>Close</button>
              </div>

              {actionError && <p role="alert" style={{ color: '#B91C1C' }}>{actionError}</p>}

              <div style={{ display: 'flex', gap: 24, flexWrap: 'wrap' }}>
                <div style={{ flex: '2 1 420px', minWidth: 320, display: 'flex', flexDirection: 'column', gap: 12 }}>
                  <label>
                    <div style={{ fontSize: 11.5, fontWeight: 700, marginBottom: 4 }}>Title (English)</div>
                    <input
                      disabled={!canEditContent}
                      value={content.title_en}
                      maxLength={CAMPAIGN_TITLE_MAX}
                      onChange={(e) => updateField('title_en', e.target.value)}
                      style={{ width: '100%', padding: '8px 10px', border: '1px solid #CBD5E1', borderRadius: 8, fontSize: 12.5 }}
                    />
                  </label>
                  <label>
                    <div style={{ fontSize: 11.5, fontWeight: 700, marginBottom: 4 }}>Title (Kiswahili)</div>
                    <input
                      disabled={!canEditContent}
                      value={content.title_sw}
                      maxLength={CAMPAIGN_TITLE_MAX}
                      onChange={(e) => updateField('title_sw', e.target.value)}
                      style={{ width: '100%', padding: '8px 10px', border: '1px solid #CBD5E1', borderRadius: 8, fontSize: 12.5 }}
                    />
                  </label>
                  <label>
                    <div style={{ fontSize: 11.5, fontWeight: 700, marginBottom: 4 }}>Message (English)</div>
                    <textarea
                      disabled={!canEditContent}
                      value={content.body_en}
                      maxLength={CAMPAIGN_BODY_MAX}
                      rows={3}
                      onChange={(e) => updateField('body_en', e.target.value)}
                      style={{ width: '100%', padding: '8px 10px', border: '1px solid #CBD5E1', borderRadius: 8, fontSize: 12.5, resize: 'vertical' }}
                    />
                  </label>
                  <label>
                    <div style={{ fontSize: 11.5, fontWeight: 700, marginBottom: 4 }}>Message (Kiswahili)</div>
                    <textarea
                      disabled={!canEditContent}
                      value={content.body_sw}
                      maxLength={CAMPAIGN_BODY_MAX}
                      rows={3}
                      onChange={(e) => updateField('body_sw', e.target.value)}
                      style={{ width: '100%', padding: '8px 10px', border: '1px solid #CBD5E1', borderRadius: 8, fontSize: 12.5, resize: 'vertical' }}
                    />
                  </label>

                  <label>
                    <div style={{ fontSize: 11.5, fontWeight: 700, marginBottom: 4 }}>Audience</div>
                    <select
                      disabled={!canEditContent}
                      value={content.audience}
                      onChange={(e) => updateField('audience', e.target.value as CampaignAudience)}
                      style={{ padding: '7px 10px', border: '1px solid #CBD5E1', borderRadius: 8, fontSize: 12.5 }}
                    >
                      <option value="PATIENTS">Patients</option>
                      <option value="DOCTORS">Doctors</option>
                      <option value="BOTH">Both</option>
                    </select>
                    <span style={{ marginLeft: 10, fontSize: 11.5, color: '#64748B' }}>
                      {audiencePreviewLoading
                        ? 'Counting eligible recipients…'
                        : audiencePreview !== null
                          ? `${audiencePreview} eligible recipient${audiencePreview === 1 ? '' : 's'} (no personal data shown)`
                          : ''}
                    </span>
                  </label>

                  {touched && validationErrors.length > 0 && (
                    <ul role="alert" style={{ color: '#B91C1C', fontSize: 11.5, margin: 0, paddingLeft: 18 }}>
                      {validationErrors.map((err) => <li key={err}>{err}</li>)}
                    </ul>
                  )}

                  <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap', marginTop: 4 }}>
                    {canEditContent && (
                      <button className="admin-btn-pri" disabled={busy} onClick={() => void saveDraft()}>
                        {selectedId === 'new' ? 'Create draft' : 'Save changes'}
                      </button>
                    )}
                    {selected && selected.status === 'DRAFT' && (
                      <button className="admin-btn-sec" disabled={busy} onClick={() => void runTransition(submitCampaign)}>
                        Submit for review
                      </button>
                    )}
                    {selected && selected.status === 'DRAFT' && (
                      <button className="admin-btn-pri" disabled={busy} onClick={() => void publishCampaignNow()}>
                        Publish now
                      </button>
                    )}
                    {selected && selected.status === 'PENDING_REVIEW' && (
                      <button className="admin-btn-sec" disabled={busy} onClick={() => void runTransition(approveCampaign)}>
                        Approve
                      </button>
                    )}
                    {selected && selected.status === 'PAUSED' && (
                      <button className="admin-btn-sec" disabled={busy} onClick={() => void runTransition(resumeCampaign)}>
                        Resume
                      </button>
                    )}
                    {selected && ['SCHEDULED', 'SENDING'].includes(selected.status) && (
                      <button className="admin-btn-sec" disabled={busy} onClick={() => void runTransition(pauseCampaign)}>
                        Pause
                      </button>
                    )}
                    {selected && !['COMPLETED', 'CANCELLED'].includes(selected.status) && (
                      <button className="admin-btn-sec" disabled={busy} style={{ color: '#B91C1C', borderColor: '#FCA5A5' }} onClick={() => void doCancel()}>
                        Cancel campaign
                      </button>
                    )}
                  </div>

                  {selected && ['APPROVED', 'SCHEDULED'].includes(selected.status) && (
                    <div style={{ border: '1px solid #E2E8F0', borderRadius: 10, padding: 12, marginTop: 4 }}>
                      <div style={{ fontSize: 11.5, fontWeight: 700, marginBottom: 6 }}>Schedule delivery</div>
                      <div style={{ display: 'flex', gap: 8, alignItems: 'center', flexWrap: 'wrap' }}>
                        <input
                          type="datetime-local"
                          value={scheduleLocal}
                          onChange={(e) => setScheduleLocal(e.target.value)}
                          style={{ padding: '7px 10px', border: '1px solid #CBD5E1', borderRadius: 8, fontSize: 12 }}
                        />
                        <span style={{ fontSize: 10.5, color: '#64748B' }}>Africa/Nairobi time</span>
                        <button className="admin-btn-sec" disabled={busy} onClick={() => void doSchedule(false)}>
                          {selected.status === 'SCHEDULED' ? 'Reschedule' : 'Schedule'}
                        </button>
                        <button className="admin-btn-pri" disabled={busy} onClick={() => void doSchedule(true)}>
                          Send immediately
                        </button>
                      </div>
                    </div>
                  )}
                </div>

                <CampaignNotificationPreview
                  titleEn={content.title_en}
                  bodyEn={content.body_en}
                  titleSw={content.title_sw}
                  bodySw={content.body_sw}
                />
              </div>

              {selected && selected.status !== 'DRAFT' && (
                <div style={{ marginTop: 20, borderTop: '1px solid #E2E8F0', paddingTop: 16 }}>
                  <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 10 }}>
                    <div className="admin-card-title" style={{ fontSize: 13.5 }}>Delivery monitoring</div>
                    <button className="admin-btn-sec" disabled={statsLoading} onClick={() => void loadStats(selected.id)}>
                      {statsLoading ? 'Refreshing…' : 'Refresh'}
                    </button>
                  </div>
                  {statsError && <p role="alert" style={{ color: '#B91C1C' }}>{statsError}</p>}
                  {stats && (
                    <div className="admin-kpi-grid" style={{ gridTemplateColumns: 'repeat(3, 1fr)' }}>
                      <div className="admin-kpi-card">
                        <span className="admin-kpi-sub">Eligible audience (live)</span>
                        <span className="admin-kpi-val">{stats.audience_count}</span>
                      </div>
                      <div className="admin-kpi-card">
                        <span className="admin-kpi-sub">Queued</span>
                        <span className="admin-kpi-val">{stats.queued}</span>
                      </div>
                      <div className="admin-kpi-card">
                        <span className="admin-kpi-sub">Sent to FCM provider</span>
                        <span className="admin-kpi-val" style={{ color: '#059669' }}>{stats.sent_to_provider}</span>
                      </div>
                      <div className="admin-kpi-card">
                        <span className="admin-kpi-sub">Deferred (preference / quiet hours)</span>
                        <span className="admin-kpi-val" style={{ color: '#B45309' }}>{stats.deferred_by_preference_or_quiet_hours}</span>
                      </div>
                      <div className="admin-kpi-card">
                        <span className="admin-kpi-sub">Retry pending</span>
                        <span className="admin-kpi-val" style={{ color: '#B45309' }}>{stats.retry_pending}</span>
                      </div>
                      <div className="admin-kpi-card">
                        <span className="admin-kpi-sub">Permanently failed</span>
                        <span className="admin-kpi-val" style={{ color: '#DC2626' }}>{stats.permanently_failed}</span>
                      </div>
                    </div>
                  )}
                  <p style={{ fontSize: 10.5, color: '#94A3B8', marginTop: 8 }}>
                    &ldquo;Sent to FCM provider&rdquo; means Google accepted the message for delivery — it is not a device-delivery or read receipt, which this platform does not track for push notifications.
                  </p>
                </div>
              )}

              {selected && (
                <div style={{ marginTop: 16 }}>
                  <button className="admin-btn-sec" onClick={() => void loadAudit(selected.id)}>
                    {auditOpen ? 'Audit history' : 'Show audit history'}
                  </button>
                  {auditOpen && (
                    <div style={{ marginTop: 10 }}>
                      {auditError && <p role="alert" style={{ color: '#B91C1C' }}>{auditError}</p>}
                      {!auditError && auditLogs === null && <p role="status">Loading audit history…</p>}
                      {auditLogs !== null && filteredAuditLogs.length === 0 && (
                        <p style={{ color: '#64748B', fontSize: 12 }}>No audit events recorded for this campaign.</p>
                      )}
                      {filteredAuditLogs.length > 0 && (
                        <table className="admin-table">
                          <thead>
                            <tr><th>Action</th><th>Actor role</th><th>Result</th><th>When</th></tr>
                          </thead>
                          <tbody>
                            {filteredAuditLogs.map((l) => (
                              <tr key={l.id}>
                                <td>{l.action}</td>
                                <td>{l.actor_role}</td>
                                <td>{l.result}</td>
                                <td>{new Date(l.created_at).toLocaleString('en-KE', { timeZone: 'Africa/Nairobi' })}</td>
                              </tr>
                            ))}
                          </tbody>
                        </table>
                      )}
                    </div>
                  )}
                </div>
              )}
            </>
          )}
        </div>
      )}
    </div>
  );
}
