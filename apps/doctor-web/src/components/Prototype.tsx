'use client';
import React from 'react';
import templates from '../generated/templates.json';

export interface PrototypeNode { tag: string; props: Record<string, unknown>; children: (PrototypeNode | string)[]; line: number }
export interface Patch { props?: Record<string, unknown>; children?: React.ReactNode; before?: React.ReactNode; tag?: string; omit?: boolean }
export type Override = (node: PrototypeNode) => Patch | undefined;
export type Action = (handler: string, event: React.SyntheticEvent) => void;
export function hasClass(n: PrototypeNode, cls: string) { return String(n.props.className ?? '').split(' ').includes(cls); }
export function id(n: PrototypeNode) { return String(n.props.id ?? ''); }
export function text(n: PrototypeNode): string { return n.children.map(child => typeof child === 'string' ? child : text(child)).join('').trim(); }
export function Prototype({ name, override, action }: { name: keyof typeof templates; override?: Override; action?: Action }) {
  function render(node: PrototypeNode | string, key: number): React.ReactNode {
    if (typeof node === 'string') return node;
    const patch = override?.(node);
    if (patch?.omit) return null;
    const props: Record<string, unknown> = { ...node.props, ...patch?.props, key };
    for (const prop of Object.keys(props)) {
      if (prop.startsWith('data-on')) {
        const handler = props[prop];
        delete props[prop];
        if (typeof handler !== 'string') continue;
        const eventName = ({ 'data-onclick':'onClick', 'data-oninput':'onChange', 'data-onchange':'onChange', 'data-onkeypress':'onKeyDown' } as Record<string, string>)[prop];
        if (eventName && !props[eventName]) props[eventName] = (event: React.SyntheticEvent) => action?.(handler, event);
      }
    }
    let tag = patch?.tag ?? node.tag;
    const clickable = props.onClick && !['button','input','select','textarea','a'].includes(tag);
    if (clickable) {
      props.role = props.role ?? 'button';
      props.tabIndex = props.tabIndex ?? 0;
      const click = props.onClick as (event: React.SyntheticEvent) => void;
      props.onKeyDown = (event: React.KeyboardEvent) => { if (event.key === 'Enter' || event.key === ' ') { event.preventDefault(); click(event); } };
    }
    if (tag === 'button') props.type = props.type ?? 'button';
    if (tag === 'svg') { props['aria-hidden'] = true; props.focusable = false; }
    if (tag === 'input' && !props['aria-label']) props['aria-label'] = props.placeholder ?? props.id ?? 'Field';
    if (tag === 'textarea' && !props['aria-label']) props['aria-label'] = props.placeholder ?? props.id ?? 'Notes';
    if (props.title && !props['aria-label'] && (tag === 'button' || clickable)) props['aria-label'] = props.title;
    const children = patch && 'children' in patch ? patch.children : node.children.map(render);
    const voidTag = ['input','img','br','hr','source','wbr'].includes(tag);
    const element=React.createElement(tag, props, ...(voidTag ? [] : [children]));
    return patch?.before?<React.Fragment key={key}>{patch.before}{element}</React.Fragment>:element;
  }
  return render(templates[name] as unknown as PrototypeNode, 0);
}
