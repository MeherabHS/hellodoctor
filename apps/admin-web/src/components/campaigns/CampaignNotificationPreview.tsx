'use client';
import { useState } from 'react';
import { PREVIEW_BODY_LIMIT, PREVIEW_TITLE_LIMIT, truncateForPreview } from '@/lib/notificationPreviewText';

interface Props {
  titleEn: string;
  bodyEn: string;
  titleSw: string;
  bodySw: string;
}

/// Live, client-side-only notification preview — re-renders from the same
/// draft state the campaign editor already holds on every keystroke, with no
/// network round trip and no way to trigger a real send (it never calls any
/// campaign API). Always visibly labeled as a simulation.
export function CampaignNotificationPreview({ titleEn, bodyEn, titleSw, bodySw }: Props) {
  const [lang, setLang] = useState<'en' | 'sw'>('en');
  const [platform, setPlatform] = useState<'android' | 'ios'>('android');

  const rawTitle = (lang === 'sw' ? titleSw : titleEn).trim() || 'Notification title';
  const rawBody = (lang === 'sw' ? bodySw : bodyEn).trim() || 'Notification message body appears here as you type.';
  const title = truncateForPreview(rawTitle, PREVIEW_TITLE_LIMIT);
  const body = truncateForPreview(rawBody, PREVIEW_BODY_LIMIT);

  return (
    <div className="campaign-preview-panel">
      <div className="campaign-preview-toolbar">
        <div>
          <div className="campaign-preview-label">Live notification preview</div>
          <div className="campaign-preview-sim-badge">SIMULATED — no notification is sent by this preview</div>
        </div>
        <div className="campaign-preview-toggles">
          <div className="campaign-preview-toggle-group" role="group" aria-label="Preview language">
            <button type="button" className={lang === 'en' ? 'active' : ''} onClick={() => setLang('en')}>EN</button>
            <button type="button" className={lang === 'sw' ? 'active' : ''} onClick={() => setLang('sw')}>SW</button>
          </div>
          <div className="campaign-preview-toggle-group" role="group" aria-label="Preview platform">
            <button type="button" className={platform === 'android' ? 'active' : ''} onClick={() => setPlatform('android')}>Android</button>
            <button type="button" className={platform === 'ios' ? 'active' : ''} onClick={() => setPlatform('ios')}>iOS</button>
          </div>
        </div>
      </div>

      <div className="campaign-preview-phone">
        <div className="campaign-preview-phone-notch" />
        <div className="campaign-preview-lockscreen">
          {platform === 'android' ? (
            <div className="campaign-preview-card campaign-preview-card-android">
              <div className="campaign-preview-card-row">
                <span className="campaign-preview-app-icon">A</span>
                <span className="campaign-preview-app-name">AfyaMkononi · now</span>
              </div>
              <div className="campaign-preview-title">{title.shown}</div>
              <div className="campaign-preview-body">{body.shown}</div>
            </div>
          ) : (
            <div className="campaign-preview-card campaign-preview-card-ios">
              <div className="campaign-preview-card-row">
                <span className="campaign-preview-app-icon campaign-preview-app-icon-ios">A</span>
                <span className="campaign-preview-app-name">AFYAMKONONI</span>
                <span className="campaign-preview-time">now</span>
              </div>
              <div className="campaign-preview-title">{title.shown}</div>
              <div className="campaign-preview-body">{body.shown}</div>
            </div>
          )}
        </div>
      </div>

      {(title.truncated || body.truncated) && (
        <p className="campaign-preview-truncation-note">
          {title.truncated && 'Title is truncated on most lock screens. '}
          {body.truncated && 'Message body is truncated on most lock screens.'}
        </p>
      )}
      <p className="campaign-preview-privacy-note">
        This is exactly what the recipient sees on their lock screen — never include a patient name, diagnosis, or other identifying clinical detail here.
      </p>

      <style jsx>{`
        .campaign-preview-panel {
          display: flex;
          flex-direction: column;
          gap: 12px;
          min-width: 260px;
          flex: 1 1 320px;
        }
        .campaign-preview-toolbar {
          display: flex;
          justify-content: space-between;
          align-items: flex-start;
          gap: 10px;
          flex-wrap: wrap;
        }
        .campaign-preview-label {
          font-size: 12px;
          font-weight: 800;
          color: #0f172a;
        }
        .campaign-preview-sim-badge {
          margin-top: 3px;
          font-size: 10px;
          font-weight: 800;
          letter-spacing: 0.3px;
          color: #b45309;
          background: #fef3c7;
          border: 1px solid #fde68a;
          border-radius: 6px;
          padding: 2px 6px;
          display: inline-block;
        }
        .campaign-preview-toggles {
          display: flex;
          gap: 8px;
          flex-wrap: wrap;
        }
        .campaign-preview-toggle-group {
          display: inline-flex;
          border: 1px solid #cbd5e1;
          border-radius: 8px;
          overflow: hidden;
        }
        .campaign-preview-toggle-group button {
          background: #fff;
          border: none;
          padding: 5px 10px;
          font-size: 11px;
          font-weight: 700;
          color: #334155;
          cursor: pointer;
        }
        .campaign-preview-toggle-group button.active {
          background: #0f172a;
          color: #fff;
        }
        .campaign-preview-phone {
          width: 100%;
          max-width: 300px;
          margin: 0 auto;
          background: linear-gradient(180deg, #1e293b, #0f172a);
          border-radius: 28px;
          padding: 16px 12px 24px;
          box-shadow: 0 8px 24px rgba(15, 23, 42, 0.25);
        }
        .campaign-preview-phone-notch {
          width: 60px;
          height: 6px;
          border-radius: 4px;
          background: rgba(255, 255, 255, 0.25);
          margin: 0 auto 14px;
        }
        .campaign-preview-lockscreen {
          min-height: 120px;
          display: flex;
          align-items: flex-start;
        }
        .campaign-preview-card {
          width: 100%;
          background: rgba(255, 255, 255, 0.96);
          border-radius: 14px;
          padding: 10px 12px;
        }
        .campaign-preview-card-row {
          display: flex;
          align-items: center;
          gap: 6px;
          margin-bottom: 4px;
        }
        .campaign-preview-app-icon {
          width: 16px;
          height: 16px;
          border-radius: 4px;
          background: #0f766e;
          color: #fff;
          font-size: 10px;
          font-weight: 800;
          display: inline-flex;
          align-items: center;
          justify-content: center;
        }
        .campaign-preview-app-icon-ios {
          border-radius: 50%;
        }
        .campaign-preview-app-name {
          font-size: 10px;
          font-weight: 700;
          color: #64748b;
          text-transform: uppercase;
          letter-spacing: 0.3px;
        }
        .campaign-preview-time {
          margin-left: auto;
          font-size: 10px;
          color: #94a3b8;
        }
        .campaign-preview-title {
          font-size: 12.5px;
          font-weight: 800;
          color: #0f172a;
          overflow-wrap: anywhere;
        }
        .campaign-preview-body {
          font-size: 11.5px;
          color: #334155;
          margin-top: 2px;
          overflow-wrap: anywhere;
        }
        .campaign-preview-truncation-note,
        .campaign-preview-privacy-note {
          font-size: 10.5px;
          color: #64748b;
          margin: 0;
        }
        .campaign-preview-truncation-note {
          color: #b45309;
        }
        @media (max-width: 420px) {
          .campaign-preview-phone {
            max-width: 220px;
          }
        }
      `}</style>
    </div>
  );
}
