/**
 * Dictionary lookup shared by server and client code (no 'use client', so
 * server components such as the root layout can call it).
 */
import { en } from './messages/en.ts';
import { sw } from './messages/sw.ts';
import type { Locale } from './locale.ts';

type Widen<T> = { [K in keyof T]: T[K] extends string ? string : Widen<T[K]> };
/** Every language must provide every English key (checked by `tsc`). */
export type Messages = Widen<typeof en>;
type Paths<T, P extends string = ''> = {
  [K in keyof T & string]: T[K] extends string ? `${P}${K}` : Paths<T[K], `${P}${K}.`>;
}[keyof T & string];
export type MessageKey = Paths<typeof en>;
export type Params = Record<string, string | number>;

const dictionaries: Record<Locale, Messages> = { en, sw };

// Kept in step with the screen's language by the provider, so helpers
// called while a component renders use the same language as the screen.
let current: Locale = 'en';

export function setCurrentLocale(locale: Locale) {
  current = locale;
}

export function currentLocale(): Locale {
  return current;
}

function lookup(dictionary: Messages, key: string): string | undefined {
  let node: unknown = dictionary;
  for (const part of key.split('.')) {
    if (node && typeof node === 'object' && part in node) node = (node as Record<string, unknown>)[part];
    else return undefined;
  }
  return typeof node === 'string' ? node : undefined;
}

function interpolate(text: string, params?: Params): string {
  if (!params) return text;
  return text.replace(/\{(\w+)\}/g, (match, name: string) =>
    name in params ? String(params[name]) : match,
  );
}

/** Translates a key in the current (or given) language. */
export function tr(key: MessageKey, params?: Params, locale: Locale = current): string {
  const text = lookup(dictionaries[locale], key) ?? lookup(dictionaries.en, key) ?? key;
  return interpolate(text, params);
}
