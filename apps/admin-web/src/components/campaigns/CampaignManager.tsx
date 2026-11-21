'use client';
import { tr } from '@/lib/i18n';
import { apiErrorMessage } from '@/lib/i18n/apiErrors';
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
import { ArrowLeft, Megaphone, Pencil, Plus, Send } from 'lucide-react';
import { Card, Chip, EmptyState, PageHead, SearchField, eatDate, type ChipTone } from '../ui/afya';
import { CAMPAIGN_BODY_MAX, CAMPAIGN_TITLE_MAX, campaignFieldErrors } from '@/lib/campaignValidation';
import { isUnpublished, stepsBeforePublish } from '@/lib/publishFlow';

// Kenya has a single fixed UTC+3 offset year-round (no DST), so an
// Africa/Nairobi wall-clock time entered in the schedule picker converts to
// an authoritative UTC instant by appending this literal offset — no
// timezone library, and no dependency on the Admin's own browser timezone.
const NAIROBI_OFFSET = '+03:00';

const STATUS_LABEL: Record<CampaignStatus, string> = {
  get DRAFT(){return tr('ui2.draft')},
  get PENDING_REVIEW(){return tr('ui2.draft')},
  get APPROVED(){return tr('ui2.draft')},
  get SCHEDULED(){return tr('ui2.scheduled')},
  get SENDING(){return tr('ui2.sending2')},
  get PAUSED(){return tr('ui2.paused')},
  get COMPLETED(){return tr('ui2.completed')},
  get CANCELLED(){return tr('ui2.cancelled')},
};
const STATUS_TONE: Record<CampaignStatus, ChipTone> = {
  DRAFT: 'warn',
  PENDING_REVIEW: 'warn',
  APPROVED: 'warn',
  SCHEDULED: 'info',
  SENDING: 'warn',
  PAUSED: 'neutral',
  COMPLETED: 'success',
  CANCELLED: 'danger',
};
const AUDIENCE_LABEL: Record<CampaignAudience, string> = { get PATIENTS(){return tr('ui2.patients')}, get DOCTORS(){return tr('ui2.doctors')}, get BOTH(){return tr('ui2.patientsAndDoctors')} };

function emptyContent(): CampaignContent {
  return { title_en: '', title_sw: '', body_en: '', body_sw: '', audience: 'PATIENTS' };
}

function errorMessage(e: unknown): string {
  return apiErrorMessage(e, tr('ui.somethingWentWrongPleaseRetry'));
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

  /// The Admin publishes directly; there is no review step in the UI
  /// (lib/publishFlow). Runs the backend's remaining internal steps from the
  /// campaign's current state; each step uses the freshest id/version the
  /// previous one returned (every transition is version-guarded server-side).
  async function readyToSchedule(campaign: Campaign): Promise<Campaign> {
    let updated = campaign;
    for (const step of stepsBeforePublish(updated.status)) {
      updated = step === 'submit'
        ? await submitCampaign(updated.id, updated.version)
        : await approveCampaign(updated.id, updated.version);
      setSelected(updated);
    }
    return updated;
  }

  async function publishCampaignNow() {
    if (!selected) return;
    setBusy(true);
    setActionError('');
    try {
      let updated = await readyToSchedule(selected);
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
      setActionError(tr('ui2.chooseADateAndTimeOr'));
      return;
    }
    setBusy(true);
    setActionError('');
    try {
      const ready = await readyToSchedule(selected);
      const updated = await scheduleCampaign(ready.id, ready.version, scheduledForUtc);
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
    if (!window.confirm(tr('ui.cancelThisCampaignAnyNotYet'))) {
      return;
    }
    await runTransition(cancelCampaign);
  }

  const filteredAuditLogs = useMemo(
    () => (selected ? (auditLogs ?? []).filter((l) => l.resource_type === 'CAMPAIGN' && l.resource_id === selected.id) : []),
    [auditLogs, selected]
  );

  const [search, setSearch] = useState('');
  const [audienceFilter, setAudienceFilter] = useState<CampaignAudience | 'ALL'>('ALL');
  const [lang, setLang] = useState<'en' | 'sw'>('en');
  const visibleCampaigns = campaigns.filter((c) =>
    (audienceFilter === 'ALL' || c.audience === audienceFilter) &&
    (!search.trim() || `${c.title_en} ${c.title_sw} ${c.body_en}`.toLowerCase().includes(search.trim().toLowerCase())));
  const closeEditor = () => { setSelectedId(null); setSelected(null); };
  const titleKey = lang === 'en' ? 'title_en' : 'title_sw';
  const bodyKey = lang === 'en' ? 'body_en' : 'body_sw';

  if (!selectedId) {
    return (
      <div className="afya-stack">
        <PageHead title={tr('ui.campaigns')} subtitle={tr('ui4.createAndManagePushNotificationCampaigns')}
          actions={<button type="button" className="afya-btn-primary" onClick={startNewCampaign}><Plus size={16} aria-hidden />{tr('ui.newCampaign')}</button>} />
        <section className="admin-card">
          <div className="afya-filter-bar">
            <SearchField label={tr('ui.searchCampaigns')} placeholder={tr('ui.searchCampaignsByTitleOrMessage')} value={search} onChange={setSearch} />
            <select className="afya-select" aria-label={tr('ui.audience')} value={audienceFilter} onChange={(e) => setAudienceFilter(e.target.value as CampaignAudience | 'ALL')}>
              <option value="ALL">{tr('ui.allAudiences')}</option><option value="PATIENTS">{tr('ui.patients')}</option><option value="DOCTORS">{tr('ui.doctors')}</option><option value="BOTH">{tr('ui.patientsAndDoctors')}</option>
            </select>
            <select className="afya-select" aria-label={tr('ui.status')} value={statusFilter} onChange={(e) => setStatusFilter(e.target.value as CampaignStatus | 'ALL')}>
              <option value="ALL">{tr('ui.allStatuses')}</option>
              {(Object.keys(STATUS_LABEL) as CampaignStatus[]).map((s) => <option key={s} value={s}>{STATUS_LABEL[s]}</option>)}
            </select>
          </div>
          {listError && <div className="afya-login-error" role="alert" style={{ marginBottom: 12 }}>{listError} <button type="button" className="afya-btn-secondary" onClick={() => void loadList()}>{tr('ui.retry')}</button></div>}
          <div className="afya-table-wrap">
            <table className="admin-table">
              <thead><tr><th>{tr('ui.campaignTitle')}</th><th>{tr('ui.channel')}</th><th>{tr('ui.audience')}</th><th>{tr('ui.schedule')}</th><th>{tr('ui.status')}</th><th>{tr('ui.updated')}</th><th style={{ textAlign: 'right' }}>{tr('ui.actions')}</th></tr></thead>
              <tbody>
                {visibleCampaigns.map((c) => (
                  <tr key={c.id} className="admin-clickable-row" onClick={() => void openCampaign(c.id)}>
                    <td><div style={{ display: 'flex', alignItems: 'center', gap: 12 }}><span className="afya-stat-icon afya-tone-green" style={{ width: 40, height: 40, borderRadius: 10 }} aria-hidden><Megaphone size={18} /></span>
                      <div><div style={{ fontWeight: 700 }}>{c.title_en}</div><div className="afya-stat-note" style={{ maxWidth: 360, whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>{c.body_en}</div></div></div></td>
                    <td><Chip tone="info">{tr('ui.push')}</Chip></td>
                    <td>{AUDIENCE_LABEL[c.audience]}</td>
                    <td style={{ whiteSpace: 'nowrap' }}>{c.scheduled_for ? new Date(c.scheduled_for).toLocaleString('en-KE', { timeZone: 'Africa/Nairobi', dateStyle: 'medium', timeStyle: 'short' }) : '—'}</td>
                    <td><Chip tone={STATUS_TONE[c.status]}>{STATUS_LABEL[c.status]}</Chip></td>
                    <td style={{ whiteSpace: 'nowrap' }}>{eatDate(c.updated_at)}</td>
                    <td style={{ textAlign: 'right' }}><button type="button" className="afya-icon-btn" aria-label={tr('tpl.openNamed', { name: c.title_en })} onClick={(e) => { e.stopPropagation(); void openCampaign(c.id); }}><Pencil size={16} /></button></td>
                  </tr>
                ))}
              </tbody>
            </table>
            {listLoading && !campaigns.length && <EmptyState title={tr('ui.loadingCampaigns')} />}
            {!listLoading && !listError && !visibleCampaigns.length && <EmptyState title={campaigns.length ? tr('ui2.noCampaignsMatchTheseFilters') : tr('ui2.noCampaignsYet')}>{campaigns.length ? tr('ui2.tryAnotherSearchAudienceOrStatus') : tr('ui2.createOneToSendAPush')}</EmptyState>}
          </div>
        </section>
      </div>
    );
  }

  return (
    <div className="afya-stack">
      <div className="afya-page-head">
        <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
          <button type="button" className="afya-icon-btn" aria-label={tr('ui.backToCampaigns')} onClick={closeEditor}><ArrowLeft size={20} /></button>
          <div>
            <h2>{selectedId === 'new' ? tr('ui2.createCampaign') : tr('ui2.editCampaign')}</h2>
            <p style={{ display: 'flex', gap: 8, alignItems: 'center', flexWrap: 'wrap' }}>
              {selected ? <Chip tone={STATUS_TONE[selected.status]}>{STATUS_LABEL[selected.status]}</Chip> : <Chip tone="neutral">{tr('ui.notSavedYet')}</Chip>}
              {selected && tr('tpl.createdOn', { date: eatDate(selected.created_at) })}
              {selected?.scheduled_for && tr('tpl.scheduledFor', { date: new Date(selected.scheduled_for).toLocaleString('en-KE', { timeZone: 'Africa/Nairobi' }) })}
            </p>
          </div>
        </div>
      </div>
      {detailLoading && <p role="status" className="afya-stat-note">{tr('ui.loadingCampaign')}</p>}
      {detailError && <div className="afya-login-error" role="alert">{detailError}</div>}
      {actionError && <div className="afya-login-error" role="alert">{actionError}</div>}
      {!detailLoading && (
        <div className="afya-two-col">
          <div className="afya-stack">
            <section className="admin-card">
              <fieldset disabled={!canEditContent || busy} className="afya-form" style={{ border: 0, padding: 0, margin: 0 }}>
                <div className="afya-lang-switch" role="group" aria-label={tr('ui.campaignLanguage')}>
                  <button type="button" aria-pressed={lang === 'en'} onClick={() => setLang('en')}>{tr('ui.englishEn')}</button>
                  <button type="button" aria-pressed={lang === 'sw'} onClick={() => setLang('sw')}>{tr('ui.kiswahiliSw')}</button>
                </div>
                <label htmlFor="cmp-title">{tr('mix.campaignTitleLang', { lang: lang === 'en' ? tr('ui2.english') : tr('ui2.kiswahili') })}</label>
                <input id="cmp-title" value={content[titleKey]} maxLength={CAMPAIGN_TITLE_MAX} onChange={(e) => updateField(titleKey, e.target.value)} />
                <span className="afya-stat-note" style={{ textAlign: 'right' }}>{content[titleKey].length} / {CAMPAIGN_TITLE_MAX}</span>
                <label htmlFor="cmp-body">{tr('mix.messageLang', { lang: lang === 'en' ? tr('ui2.english') : tr('ui2.kiswahili') })}</label>
                <textarea id="cmp-body" rows={4} value={content[bodyKey]} maxLength={CAMPAIGN_BODY_MAX} onChange={(e) => updateField(bodyKey, e.target.value)} />
                <span className="afya-stat-note" style={{ textAlign: 'right' }}>{content[bodyKey].length} / {CAMPAIGN_BODY_MAX}</span>
                <label htmlFor="cmp-audience">{tr('ui.audience2')}</label>
                <select id="cmp-audience" value={content.audience} onChange={(e) => updateField('audience', e.target.value as CampaignAudience)}>
                  <option value="PATIENTS">{tr('ui.patients')}</option><option value="DOCTORS">{tr('ui.doctors')}</option><option value="BOTH">{tr('ui.patientsAndDoctors')}</option>
                </select>
                <span className="afya-stat-note">{audiencePreviewLoading ? tr('tpl.countingRecipients') : audiencePreview !== null ? tr(audiencePreview === 1 ? 'tpl.recipientsOne' : 'tpl.recipientsMany', { count: audiencePreview }) : ''}</span>
                <span className="afya-field-label">{tr('ui.deliveryChannel')}</span>
                <span className="afya-check"><input type="checkbox" checked readOnly disabled />{tr('ui.inAppPushNotification')}</span>
              </fieldset>
              {touched && validationErrors.length > 0 && (
                <ul role="alert" className="afya-stat-note" style={{ color: 'var(--afya-danger-fg)', margin: '12px 0 0', paddingLeft: 18 }}>
                  {validationErrors.map((err) => <li key={err}>{err}</li>)}
                </ul>
              )}
            </section>

            {selected && (isUnpublished(selected.status) || selected.status === 'SCHEDULED') && (
              <Card title={tr('ui.schedule')} subtitle={tr('ui4.africaNairobiTimeUtc3')}>
                <div style={{ display: 'flex', gap: 10, alignItems: 'center', flexWrap: 'wrap' }}>
                  <input className="afya-input" style={{ width: 230 }} type="datetime-local" aria-label={tr('ui.sendAt')} value={scheduleLocal} onChange={(e) => setScheduleLocal(e.target.value)} />
                  <button type="button" className="afya-btn-secondary" disabled={busy} onClick={() => void doSchedule(false)}>{selected.status === 'SCHEDULED' ? tr('ui2.reschedule') : tr('ui2.scheduleForLater')}</button>
                  {selected.status === 'SCHEDULED' && <button type="button" className="afya-btn-primary" disabled={busy} onClick={() => void doSchedule(true)}><Send size={15} aria-hidden />{tr('ui.sendNow')}</button>}
                </div>
              </Card>
            )}

            <div style={{ display: 'flex', gap: 10, flexWrap: 'wrap' }}>
              {canEditContent && <button type="button" className="afya-btn-outline-green" disabled={busy} onClick={() => void saveDraft()}>{selectedId === 'new' ? tr('ui2.saveDraft') : tr('ui2.saveChanges')}</button>}
              {selected && isUnpublished(selected.status) && <button type="button" className="afya-btn-primary" disabled={busy} onClick={() => void publishCampaignNow()}><Send size={15} aria-hidden />{tr('ui.publishNow')}</button>}
              {selected && selected.status === 'PAUSED' && <button type="button" className="afya-btn-secondary" disabled={busy} onClick={() => void runTransition(resumeCampaign)}>{tr('ui.resume')}</button>}
              {selected && ['SCHEDULED', 'SENDING'].includes(selected.status) && <button type="button" className="afya-btn-secondary" disabled={busy} onClick={() => void runTransition(pauseCampaign)}>{tr('ui.pause')}</button>}
              {selected && !['COMPLETED', 'CANCELLED'].includes(selected.status) && <button type="button" className="afya-btn-danger" disabled={busy} onClick={() => void doCancel()}>{tr('ui.cancelCampaign')}</button>}
            </div>

            {selected && (
              <Card title={tr('ui.auditHistory')} actions={<button type="button" className="afya-btn-secondary" onClick={() => void loadAudit(selected.id)}>{auditOpen ? tr('ui2.refresh') : tr('ui2.show')}</button>}>
                {!auditOpen ? <p className="afya-stat-note" style={{ margin: 0 }}>{tr('ui.everyChangeToThisCampaignIs')}</p> : (
                  <>
                    {auditError && <div className="afya-login-error" role="alert">{auditError}</div>}
                    {!auditError && auditLogs === null && <p role="status" className="afya-stat-note">{tr('ui.loadingAuditHistory')}</p>}
                    {auditLogs !== null && filteredAuditLogs.length === 0 && <p className="afya-stat-note">{tr('ui.noAuditEventsRecordedForThis')}</p>}
                    {filteredAuditLogs.length > 0 && (
                      <div className="afya-table-wrap"><table className="admin-table">
                        <thead><tr><th>{tr('ui.action')}</th><th>{tr('ui.actor')}</th><th>{tr('ui.result')}</th><th>{tr('ui.when')}</th></tr></thead>
                        <tbody>{filteredAuditLogs.map((l) => <tr key={l.id}><td>{l.action}</td><td>{l.actor_role}</td><td>{l.result}</td><td>{new Date(l.created_at).toLocaleString('en-KE', { timeZone: 'Africa/Nairobi' })}</td></tr>)}</tbody>
                      </table></div>
                    )}
                  </>
                )}
              </Card>
            )}
          </div>

          <div className="afya-stack">
            {selected && selected.status !== 'DRAFT' && (
              <Card title={tr('ui.deliveryMonitoring')} actions={<button type="button" className="afya-btn-secondary" disabled={statsLoading} onClick={() => void loadStats(selected.id)}>{statsLoading ? tr('ui2.refreshing') : tr('ui2.refresh')}</button>}>
                {statsError && <div className="afya-login-error" role="alert">{statsError}</div>}
                {stats && (
                  <div className="afya-mini-stats">
                    <div><span>{stats.audience_count}</span>{tr('ui.eligibleAudience')}</div>
                    <div><span>{stats.queued}</span>{tr('ui.queued')}</div>
                    <div><span style={{ color: 'var(--afya-success-fg)' }}>{stats.sent_to_provider}</span>{tr('ui.sentToFcm')}</div>
                    <div><span style={{ color: 'var(--afya-warn-fg)' }}>{stats.deferred_by_preference_or_quiet_hours}</span>{tr('ui.deferredQuietHours')}</div>
                    <div><span style={{ color: 'var(--afya-warn-fg)' }}>{stats.retry_pending}</span>{tr('ui.retryPending')}</div>
                    <div><span style={{ color: 'var(--afya-danger-fg)' }}>{stats.permanently_failed}</span>{tr('ui.failed')}</div>
                  </div>
                )}
                <p className="afya-stat-note" style={{ margin: '10px 0 0' }}>{tr('ui.sentToFcmMeansGoogleAccepted')}</p>
              </Card>
            )}
            <Card title={tr('ui.liveNotificationPreview')}>
              <CampaignNotificationPreview titleEn={content.title_en} bodyEn={content.body_en} titleSw={content.title_sw} bodySw={content.body_sw} />
            </Card>
          </div>
        </div>
      )}
    </div>
  );
}