'use client';

import React from 'react';
import {
  LayoutDashboard,
  Wallet,
  Receipt,
  AlertTriangle,
  FileCheck,
  Calendar,
  Users,
  Activity,
  ShieldCheck,
} from 'lucide-react';

export type AdminTab =
  | 'command'
  | 'settlements'
  | 'payments'
  | 'grievances'
  | 'bmdc'
  | 'slots'
  | 'patients'
  | 'logs';

interface SidebarProps {
  activeTab: AdminTab;
  onTabChange: (tab: AdminTab) => void;
  pendingGrievancesCount: number;
}

export default function Sidebar({
  activeTab,
  onTabChange,
  pendingGrievancesCount,
}: SidebarProps) {
  const navItems = [
    { id: 'command' as AdminTab, label: 'Command Center', icon: LayoutDashboard },
    { id: 'settlements' as AdminTab, label: 'Settlements & Payouts', icon: Wallet },
    { id: 'payments' as AdminTab, label: 'All Payments & Holds', icon: Receipt },
    {
      id: 'grievances' as AdminTab,
      label: 'Consultation Grievances',
      icon: AlertTriangle,
      badge: pendingGrievancesCount > 0 ? pendingGrievancesCount : undefined,
      badgeColor: 'amber',
    },
    { id: 'bmdc' as AdminTab, label: 'BMDC Credentialing', icon: FileCheck },
    { id: 'slots' as AdminTab, label: 'Slot Matrix & Locks', icon: Calendar },
    { id: 'patients' as AdminTab, label: 'Patients Directory', icon: Users },
    { id: 'logs' as AdminTab, label: 'Incident & System Logs', icon: Activity },
  ];

  return (
    <aside className="w-64 bg-white border-r border-slate-200 flex flex-col flex-shrink-0 select-none">
      {/* Header / Brand */}
      <div className="h-16 flex items-center gap-3 px-5 border-b border-slate-100">
        <div className="w-8 h-8 rounded-lg bg-gradient-to-br from-sky-600 to-sky-700 text-white flex items-center justify-center font-bold text-base shadow-sm">
          +
        </div>
        <div>
          <h1 className="text-sm font-bold text-slate-900 tracking-tight leading-none">
            HelloDoctor
          </h1>
          <span className="text-[11px] font-medium text-slate-500">
            Operations & Medical Admin
          </span>
        </div>
      </div>

      {/* Navigation */}
      <nav className="flex-1 py-4 px-3 space-y-1 overflow-y-auto">
        {navItems.map((item) => {
          const Icon = item.icon;
          const isActive = activeTab === item.id;
          return (
            <button
              key={item.id}
              onClick={() => onTabChange(item.id)}
              className={`w-full flex items-center gap-3 px-3 py-2.5 rounded-xl text-xs font-semibold transition-all ${
                isActive
                  ? 'bg-blue-50 text-blue-600 font-bold shadow-xs'
                  : 'text-slate-600 hover:bg-slate-50 hover:text-slate-900'
              }`}
            >
              <Icon
                className={`w-4 h-4 transition-colors ${
                  isActive ? 'text-blue-600' : 'text-slate-400'
                }`}
              />
              <span className="flex-1 text-left">{item.label}</span>
              {item.badge !== undefined && (
                <span
                  className={`text-[10px] font-bold px-2 py-0.5 rounded-full ${
                    item.badgeColor === 'amber'
                      ? 'bg-amber-100 text-amber-800'
                      : 'bg-blue-100 text-blue-800'
                  }`}
                >
                  {item.badge}
                </span>
              )}
            </button>
          );
        })}
      </nav>

      {/* Footer */}
      <div className="p-4 border-t border-slate-100 flex items-center justify-between text-[11px] text-slate-400">
        <div className="flex items-center gap-1.5 font-medium">
          <ShieldCheck className="w-3.5 h-3.5 text-emerald-500" />
          <span>v2.4.0 (Enterprise)</span>
        </div>
        <span className="px-1.5 py-0.5 bg-emerald-50 text-emerald-700 font-bold rounded text-[10px]">
          ONLINE
        </span>
      </div>
    </aside>
  );
}
