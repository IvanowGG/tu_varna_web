// Tiny DOM helpers. Everything is built with createElement + textContent:
// student data is never parsed as HTML, so it cannot inject script.

import { icon } from './icons.js';
import { t, lang } from './i18n.js';

export function h(tag, attrs, ...kids) {
  const el = document.createElement(tag);
  if (attrs) {
    for (const [k, v] of Object.entries(attrs)) {
      if (v == null || v === false) continue;
      if (k === 'class') el.className = v;
      else if (k === 'text') el.textContent = v;
      else if (k.startsWith('on') && typeof v === 'function') el.addEventListener(k.slice(2), v);
      else if (k === 'value') el.value = v;
      else if (k === 'checked') el.checked = !!v;
      else if (k === 'dataset') Object.assign(el.dataset, v);
      else if (k === 'style' && typeof v === 'object') Object.assign(el.style, v);
      else el.setAttribute(k, v === true ? '' : String(v));
    }
  }
  add(el, kids);
  return el;
}

function add(el, kids) {
  for (const k of kids) {
    if (k == null || k === false) continue;
    if (Array.isArray(k)) add(el, k);
    else if (k instanceof Node) el.append(k);
    else el.append(document.createTextNode(String(k)));
  }
}

export function clear(el) { while (el.firstChild) el.firstChild.remove(); return el; }

export function btn(label, opts = {}) {
  const b = h('button', { type: opts.type || 'button', class: 'btn ' + (opts.class || ''), onclick: opts.onclick, title: opts.title, 'aria-label': opts.aria || (opts.iconOnly ? label : null), disabled: opts.disabled });
  if (opts.icon) b.append(icon(opts.icon));
  if (!opts.iconOnly) b.append(document.createTextNode(label));
  return b;
}

export function field(label, control, hint) {
  return h('label', { class: 'field' }, h('span', null, label), control, hint ? h('small', { class: 'faint' }, hint) : null);
}

/** A segmented control: the buttons *are* the value. */
export function seg(options, value, onchange, aria) {
  const wrap = h('div', { class: 'seg', role: 'group', 'aria-label': aria || '' });
  const render = (v) => {
    clear(wrap);
    for (const [val, label] of options) {
      wrap.append(h('button', {
        type: 'button', 'aria-pressed': String(val === v),
        onclick: () => { render(val); onchange(val); },
      }, label));
    }
  };
  render(value);
  return wrap;
}

export function select(options, value, onchange, attrs = {}) {
  const s = h('select', { ...attrs, onchange: (e) => onchange && onchange(e.target.value) });
  for (const [val, label] of options) s.append(h('option', { value: val, selected: String(val) === String(value) || null }, label));
  s.value = value ?? '';
  return s;
}

export function empty(iconName, text, action) {
  return h('div', { class: 'empty' }, icon(iconName), h('p', null, text), action || null);
}

export function banner(kind, iconName, ...content) {
  return h('div', { class: 'banner ' + (kind || '') }, icon(iconName), h('div', null, ...content));
}

// ---------- Modal ----------
let lastFocus = null;
export function modal(title, body, { wide = false, onclose } = {}) {
  const root = document.getElementById('modal-root');
  lastFocus = document.activeElement;
  const close = () => { back.remove(); document.removeEventListener('keydown', onKey); if (onclose) onclose(); if (lastFocus && lastFocus.focus) lastFocus.focus(); };
  const onKey = (e) => { if (e.key === 'Escape') close(); };
  const box = h('div', { class: 'modal' + (wide ? ' wide' : ''), role: 'dialog', 'aria-modal': 'true', 'aria-label': title },
    h('header', null, h('h2', null, title), btn(t('close'), { icon: 'x', iconOnly: true, class: 'ghost icon small', onclick: () => close() })),
    body);
  const back = h('div', { class: 'modal-back', onmousedown: (e) => { if (e.target === back) close(); } }, box);
  root.append(back);
  document.addEventListener('keydown', onKey);
  const first = box.querySelector('input, select, textarea, button:not(.ghost)');
  if (first) setTimeout(() => first.focus(), 30);
  return { close, box };
}

export function confirmBox(message, okLabel) {
  return new Promise((resolve) => {
    let done = false;
    const finish = (v) => { if (!done) { done = true; resolve(v); m.close(); } };
    const m = modal(t('are_you_sure'), h('div', null,
      h('p', null, message),
      h('div', { class: 'row end' },
        btn(t('cancel'), { onclick: () => finish(false) }),
        btn(okLabel || t('delete'), { class: 'primary', onclick: () => finish(true) }))), { onclose: () => { if (!done) { done = true; resolve(false); } } });
  });
}

// ---------- Formatting ----------
export function locale() { return lang() === 'en' ? 'en-GB' : 'bg-BG'; }
export function fmtDate(d, opts = { day: 'numeric', month: 'short' }) {
  if (!d) return '';
  const x = d instanceof Date ? d : new Date(d + (String(d).length === 10 ? 'T00:00:00' : ''));
  return x.toLocaleDateString(locale(), opts);
}
export function fmtTime(d) { return d.toLocaleTimeString(locale(), { hour: '2-digit', minute: '2-digit' }); }
export function todayISO(d = new Date()) {
  const p = (n) => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`;
}
export function daysBetween(aISO, bISO) {
  const a = new Date(aISO + 'T00:00:00'), b = new Date(bISO + 'T00:00:00');
  return Math.round((b - a) / 86400000);
}
export function relDays(iso) {
  const n = daysBetween(todayISO(), iso);
  if (n === 0) return t('today');
  if (n === 1) return t('tomorrow');
  if (n === -1) return t('yesterday');
  return n > 0 ? t('in_days', n) : t('days_ago', -n);
}
export function uid() {
  const a = new Uint8Array(9); crypto.getRandomValues(a);
  return Array.from(a, (b) => b.toString(16).padStart(2, '0')).join('');
}
export function debounce(fn, ms) {
  let id; return (...args) => { clearTimeout(id); id = setTimeout(() => fn(...args), ms); };
}
export function initials(name) {
  return (name || '?').split(/\s+/).filter(Boolean).slice(0, 2).map((w) => w[0].toUpperCase()).join('');
}
export function avatar(user, cls = '') {
  const a = h('div', { class: 'avatar ' + cls });
  if (user && user.photo && user.photo.startsWith('data:image/')) a.append(h('img', { src: user.photo, alt: '' }));
  else a.textContent = initials(user && user.name);
  return a;
}

