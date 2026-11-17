// Shared timetable logic: which lessons apply on a given date for this student.

import * as store from './store.js';
import { t } from './i18n.js';
import { h } from './ui.js';
import { icon } from './icons.js';

export function saved() { return store.load('timetable', null); }

export function subgroups(tt) {
  const s = new Set();
  (tt?.timetable?.lessons || []).forEach((l) => { if (l.subgroup) s.add(l.subgroup); });
  return [...s].sort();
}

/** Parity of a teaching week: 'odd' | 'even' | null (outside the semester). */
export function parityOf(week) {
  if (week < 1) return null;
  return week % 2 === 1 ? 'odd' : 'even';
}

/** Lessons for a weekday (1–7) in a given teaching week, filtered by the student's sub-group. */
export function lessonsFor(day, week, { subgroup = store.settings().subgroup, allParities = false } = {}) {
  const tt = saved();
  const p = parityOf(week);
  return (tt?.timetable?.lessons || []).filter((l) =>
    l.day === day &&
    (!subgroup || !l.subgroup || l.subgroup === subgroup) &&
    (allParities || !p || l.parity === 'all' || l.parity === p));
}

export function minutes(hhmm) { const [a, b] = hhmm.split(':').map(Number); return a * 60 + b; }

/** The current or next lesson today: { lesson, state: 'now' | 'next', minutesLeft }. */
export function nowNext(date = new Date()) {
  const day = ((date.getDay() + 6) % 7) + 1;
  const week = store.weekNumber(date);
  const nowM = date.getHours() * 60 + date.getMinutes();
  const list = lessonsFor(day, week).slice().sort((a, b) => minutes(a.start) - minutes(b.start));
  for (const l of list) {
    const s = minutes(l.start), e = minutes(l.end);
    if (nowM >= s && nowM < e) return { lesson: l, state: 'now', minutesLeft: e - nowM };
    if (nowM < s) return { lesson: l, state: 'next', minutesLeft: s - nowM };
  }
  return null;
}

export function kindLabel(k) { return t('k_' + (k || 'other')); }

export function lessonCard(l, { now = false, dim = false } = {}) {
  const meta = [];
  if (l.room) meta.push(h('span', null, icon('pin', 'i14'), ' ', l.room));
  if (l.lecturer) meta.push(h('span', null, l.lecturer));
  const tags = [h('span', { class: 'pill' }, h('i', { class: 'kdot' }), ' ', kindLabel(l.kind))];
  if (l.subgroup) tags.push(h('span', { class: 'pill primary' }, l.subgroup));
  if (l.parity === 'odd') tags.push(h('span', { class: 'pill warn' }, t('odd_only')));
  if (l.parity === 'even') tags.push(h('span', { class: 'pill warn' }, t('even_only')));
  if (l.hours) tags.push(h('span', { class: 'pill' }, l.hours));
  return h('article', { class: `lesson k-${l.kind}${now ? ' now' : ''}${dim ? ' dim' : ''}`, title: l.raw },
    h('span', { class: 't' }, `${l.start} – ${l.end}`),
    h('span', { class: 's' }, l.subject || l.raw),
    meta.length ? h('span', { class: 'm' }, meta) : null,
    l.extra && l.extra.length ? h('span', { class: 'm' }, l.extra.join(' · ')) : null,
    h('div', { class: 'tags' }, tags));
}

export function fmtCountdown(mins) {
  const hh = Math.floor(mins / 60), mm = mins % 60;
  return hh ? `${hh}:${String(mm).padStart(2, '0')} h` : `${mm} ${t('minutes_short')}`;
}
