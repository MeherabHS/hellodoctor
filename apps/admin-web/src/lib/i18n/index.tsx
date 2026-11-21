'use client';
import React, { createContext, useCallback, useContext, useMemo, useState } from 'react';
import { currentLocale, setCurrentLocale, tr, type MessageKey, type Params } from './core';
import { intlTag, type Locale, writeLocaleCookie } from './locale';

export type { Locale } from './locale';
export type { MessageKey, Messages, Params } from './core';
export { currentLocale, tr };

interface I18nValue {
  locale: Locale;
  setLocale: (locale: Locale) => void;
  t: (key: MessageKey, params?: Params) => string;
  formatDate: (value: Date | string | number, options?: Intl.DateTimeFormatOptions) => string;
  formatNumber: (value: number, options?: Intl.NumberFormatOptions) => string;
}

const I18nContext = createContext<I18nValue | null>(null);

export function I18nProvider({ initialLocale, children }: { initialLocale: Locale; children: React.ReactNode }) {
  const [locale, setState] = useState<Locale>(initialLocale);
  setCurrentLocale(locale);
  const setLocale = useCallback((next: Locale) => {
    writeLocaleCookie(next);
    setCurrentLocale(next);
    if (typeof document !== 'undefined') document.documentElement.lang = next;
    setState(next);
  }, []);
  const value = useMemo<I18nValue>(() => {
    const tag = intlTag(locale);
    return {
      locale,
      setLocale,
      t: (key, params) => tr(key, params, locale),
      formatDate: (input, options) => {
        const date = input instanceof Date ? input : new Date(input);
        return Number.isNaN(date.getTime()) ? '' : date.toLocaleString(tag, options);
      },
      formatNumber: (input, options) => input.toLocaleString(tag, options),
    };
  }, [locale, setLocale]);
  // Each page's root component calls useT(), so a switch re-renders the
  // screen in place (no remount: the open page and unsaved edits survive).
  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}

/** The screen's language. Outside a provider (unit tests) it is English. */
export function useT(): I18nValue {
  const value = useContext(I18nContext);
  if (value) return value;
  return {
    locale: 'en',
    setLocale: () => {},
    t: (key, params) => tr(key, params, 'en'),
    formatDate: (input, options) => new Date(input).toLocaleString('en-KE', options),
    formatNumber: (input, options) => input.toLocaleString('en-KE', options),
  };
}

/** EN | SW switch. */
export function LanguageSwitch({ className = '' }: { className?: string }) {
  const { locale, setLocale, t } = useT();
  return (
    <div role="group" aria-label={t('common.language')} className={`afya-lang-switch ${className}`}>
      {(['en', 'sw'] as const).map((code) => (
        <button
          key={code}
          type="button"
          aria-pressed={locale === code}
          onClick={() => setLocale(code)}
          className={locale === code ? 'active' : undefined}
        >
          {code.toUpperCase()}
        </button>
      ))}
    </div>
  );
}

// Re-exported for helper modules that need to know the screen's language.
export function isSwahili(): boolean {
  return currentLocale() === 'sw';
}
