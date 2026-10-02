'use client';

import React from 'react';
import { Search, PhoneCall, Bell, UserCheck } from 'lucide-react';

interface TopbarProps {
  title: string;
  subtitle: string;
  activeDoctorsCount: number;
}

export default function Topbar({
  title,
  subtitle,
  activeDoctorsCount,
}: TopbarProps) {
  return (
    <header className="h-16 bg-white border-b border-slate-200 px-6 flex items-center justify-between flex-shrink-0 z-10">
      <div>
        <h2 className="text-base font-bold text-slate-900 tracking-tight leading-none">
          {title}
        </h2>
        <p className="text-[11px] text-slate-500 font-medium mt-0.5">
          {subtitle}
        </p>
      </div>

      <div className="flex items-center gap-4">
        {/* Search Input */}
        <div className="relative">
          <Search className="w-4 h-4 text-slate-400 absolute left-3 top-1/2 -translate-y-1/2 pointer-events-none" />
          <input
            type="text"
            placeholder="Search doctors, patient IDs, transactions..."
            className="w-72 bg-slate-50 border border-slate-200 rounded-xl pl-9 pr-4 py-1.5 text-xs text-slate-900 placeholder:text-slate-400 focus:outline-none focus:bg-white focus:border-blue-500 focus:ring-2 focus:ring-blue-100 transition-all"
          />
        </div>

        {/* 16263 Emergency Hotline Indicator */}
        <div className="flex items-center gap-2 px-3 py-1.5 bg-rose-50 border border-rose-200 rounded-xl text-rose-600 text-xs font-bold">
          <PhoneCall className="w-3.5 h-3.5 animate-pulse text-rose-500" />
          <span>Emergency: 16263</span>
        </div>

        {/* Active Doctors Counter */}
        <div className="flex items-center gap-1.5 px-3 py-1.5 bg-emerald-50 border border-emerald-200 rounded-xl text-emerald-700 text-xs font-bold">
          <UserCheck className="w-3.5 h-3.5" />
          <span>{activeDoctorsCount} Doctors On-Duty</span>
        </div>

        {/* Notification Bell */}
        <button className="w-8 h-8 rounded-xl bg-slate-50 border border-slate-200 flex items-center justify-center text-slate-600 hover:bg-slate-100 transition-colors">
          <Bell className="w-4 h-4" />
        </button>

        {/* Admin User Avatar */}
        <div className="flex items-center gap-2 pl-2 border-l border-slate-200">
          <div className="w-8 h-8 rounded-xl bg-slate-900 text-white flex items-center justify-center text-xs font-bold">
            AD
          </div>
          <div className="text-left">
            <div className="text-xs font-bold text-slate-900 leading-none">
              Super Admin
            </div>
            <div className="text-[10px] text-slate-500 font-medium">
              Medical Board
            </div>
          </div>
        </div>
      </div>
    </header>
  );
}
