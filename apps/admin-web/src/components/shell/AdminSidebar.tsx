'use client';
import {useState} from 'react';
import type {LucideIcon} from 'lucide-react';
import {ChevronDown, Banknote, FileText, LayoutDashboard, Megaphone, ShieldCheck, UserCog, Users} from 'lucide-react';

/** Every page the Admin shell can show. Ids are unchanged from the old shell. */
export type AdminPageId =
  | 'doctors'
  | 'finance'
  | 'command'
  | 'slots'
  | 'patients'
  | 'verification'
  | 'peers'
  | 'peerDirectory'
  | 'compliance'
  | 'logs'
  | 'grievances'
  | 'campaigns'
  | 'blogs'
  | 'facilities'
  | 'quizzes'
  | 'topics'
  | 'team'
  | 'activity'
  | 'settings';

export type NavBadgeTone = 'amber' | 'blue' | 'red' | 'green' | 'muted';
export interface NavBadge { label: string; tone: NavBadgeTone; title?: string }

interface NavLeaf { id: AdminPageId; label: string }
interface NavGroup { key: string; label: string; icon: LucideIcon; pages: NavLeaf[] }

/**
 * Merged navigation from the five Admin boards (docs/admin-ui/ADMIN_REDESIGN_PLAN.md).
 * Users, Reports and Resources are not listed: no pages exist behind them.
 */
export const ADMIN_NAV: NavGroup[] = [
  {key: 'dashboard', label: 'Dashboard', icon: LayoutDashboard, pages: [{id: 'command', label: 'Dashboard'}]},
  {
    key: 'people',
    label: 'People & Care',
    icon: Users,
    pages: [
      {id: 'patients', label: 'Patients Directory'},
      {id: 'verification', label: 'Doctor Verification'},
      {id: 'peers', label: 'Peer Navigators'},
      {id: 'peerDirectory', label: 'Peer Directory'},
    ],
  },
  {
    key: 'operations',
    label: 'Operations & Safety',
    icon: ShieldCheck,
    pages: [
      {id: 'slots', label: 'Slot Matrix & Locks'},
      {id: 'logs', label: 'App Errors & Incidents'},
      {id: 'grievances', label: 'Consultation Grievances'},
      {id: 'compliance', label: 'Rx & Q&A Audit'},
    ],
  },
  {
    key: 'content',
    label: 'Content',
    icon: FileText,
    pages: [
      {id: 'blogs', label: 'Articles (Blog)'},
      {id: 'topics', label: 'Topics & Categories'},
      {id: 'quizzes', label: 'Quizzes'},
      {id: 'facilities', label: 'Facilities'},
    ],
  },
  {key: 'campaigns', label: 'Campaigns', icon: Megaphone, pages: [{id: 'campaigns', label: 'Campaigns'}]},
  {
    key: 'finance',
    label: 'Finance',
    icon: Banknote,
    pages: [
      {id: 'finance', label: 'All Payments & Escrow'},
      {id: 'doctors', label: 'Settlements & Payouts'},
      {id: 'settings', label: 'Payment Settings'},
    ],
  },
  {key: 'team', label: 'Admin Team', icon: UserCog, pages: [{id: 'team', label: 'Admins & Invitations'}, {id: 'activity', label: 'Activity Log'}]},
];

/** Breadcrumb for the top bar: [group, page], or [page] for single-page groups. */
export function pageTrail(page: AdminPageId): string[] {
  const group = ADMIN_NAV.find(g => g.pages.some(p => p.id === page));
  const leaf = group?.pages.find(p => p.id === page);
  if (!group || !leaf) return [];
  return group.pages.length === 1 ? [group.label] : [group.label, leaf.label];
}

export function groupOf(page: AdminPageId): string {
  return ADMIN_NAV.find(g => g.pages.some(p => p.id === page))?.key ?? 'dashboard';
}

export function AdminSidebar({
  active,
  onNavigate,
  badges,
  groupBadges,
}: {
  active: AdminPageId;
  onNavigate: (page: AdminPageId) => void;
  badges?: Partial<Record<AdminPageId, NavBadge>>;
  groupBadges?: Partial<Record<string, NavBadge>>;
}) {
  const activeGroup = groupOf(active);
  // The active page's group is always open; others open on demand.
  const [open, setOpen] = useState<Record<string, boolean>>({});
  const isOpen = (key: string) => key === activeGroup || open[key] === true;

  return (
    <aside className="admin-sidebar" aria-label="Admin navigation">
      <div className="admin-sidebar-header">
        {/* eslint-disable-next-line @next/next/no-img-element -- static same-origin asset */}
        <img src="/brand/afya_logo_white.png" alt="" />
        <div style={{display: 'flex', flexDirection: 'column'}}>
          <span className="admin-brand-name">AfyaMkononi</span>
          <span className="admin-brand-role">Admin</span>
        </div>
      </div>

      <nav className="admin-sidebar-nav">
        {ADMIN_NAV.map(group => {
          const Icon = group.icon;
          if (group.pages.length === 1) {
            const page = group.pages[0];
            const badge = badges?.[page.id];
            return (
              <button
                key={group.key}
                type="button"
                id={`adminNav-${page.id}`}
                className={`admin-nav-item ${active === page.id ? 'active' : ''}`}
                aria-current={active === page.id ? 'page' : undefined}
                onClick={() => onNavigate(page.id)}
              >
                <Icon size={18} aria-hidden />
                <span>{group.label}</span>
                {badge && <span className={`admin-nav-badge ${badge.tone}`} title={badge.title}>{badge.label}</span>}
              </button>
            );
          }
          const expanded = isOpen(group.key);
          const groupBadge = groupBadges?.[group.key];
          return (
            <div key={group.key}>
              <button
                type="button"
                className={`admin-nav-item ${activeGroup === group.key && !expanded ? 'active' : ''} ${activeGroup === group.key ? 'group-active' : ''}`}
                aria-expanded={expanded}
                aria-controls={`adminNavGroup-${group.key}`}
                onClick={() => setOpen(prev => ({...prev, [group.key]: !isOpen(group.key)}))}
              >
                <Icon size={18} aria-hidden />
                <span>{group.label}</span>
                {groupBadge && !expanded && <span className={`admin-nav-badge ${groupBadge.tone}`} title={groupBadge.title}>{groupBadge.label}</span>}
                <ChevronDown size={16} className="admin-nav-chevron" aria-hidden style={groupBadge && !expanded ? {marginLeft: 6} : undefined} />
              </button>
              {expanded && (
                <div className="admin-nav-sub" id={`adminNavGroup-${group.key}`}>
                  {group.pages.map(page => {
                    const badge = badges?.[page.id];
                    return (
                      <button
                        key={page.id}
                        type="button"
                        id={`adminNav-${page.id}`}
                        className={`admin-nav-subitem ${active === page.id ? 'active' : ''}`}
                        aria-current={active === page.id ? 'page' : undefined}
                        onClick={() => onNavigate(page.id)}
                      >
                        <span>{page.label}</span>
                        {badge && <span className={`admin-nav-badge ${badge.tone}`} title={badge.title}>{badge.label}</span>}
                      </button>
                    );
                  })}
                </div>
              )}
            </div>
          );
        })}
      </nav>
    </aside>
  );
}
