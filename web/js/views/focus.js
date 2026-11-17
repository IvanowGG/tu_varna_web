import { h, btn, seg, todayISO } from '../ui.js';
import { t } from '../i18n.js';
import * as store from '../store.js';

// One timer for the whole app, so it keeps running when you switch pages.
const T = { phase: 'focus', running: false, endAt: 0, left: 0, total: 0, tick: null, listeners: new Set() };

function beep() {
  try {
    const ctx = new AudioContext();
    [0, 0.25, 0.5].forEach((d) => {
      const o = ctx.createOscillator(); const g = ctx.createGain();
      o.frequency.value = 880; o.connect(g); g.connect(ctx.destination);
      g.gain.setValueAtTime(0.0001, ctx.currentTime + d);
      g.gain.exponentialRampToValueAtTime(0.25, ctx.currentTime + d + 0.02);
      g.gain.exponentialRampToValueAtTime(0.0001, ctx.currentTime + d + 0.2);
      o.start(ctx.currentTime + d); o.stop(ctx.currentTime + d + 0.22);
    });
  } catch { /* no audio */ }
}

function lengthOf(phase) { const s = store.settings(); return (phase === 'focus' ? s.focusMin : s.breakMin) * 60; }

function emit() { T.listeners.forEach((fn) => fn()); }

function step() {
  T.left = Math.max(0, Math.round((T.endAt - Date.now()) / 1000));
  if (T.left === 0) finish();
  emit();
}

function finish() {
  clearInterval(T.tick); T.running = false;
  if (T.phase === 'focus') {
    const f = store.load('focus', { sessions: [] });
    f.sessions.push({ date: todayISO(), minutes: Math.round(T.total / 60), at: Date.now() });
    f.sessions = f.sessions.slice(-2000);
    store.save('focus', f);
  }
  beep();
  T.message = T.phase === 'focus' ? t('focus_done') : t('break_done');
  T.phase = T.phase === 'focus' ? 'break' : 'focus';
  T.total = T.left = lengthOf(T.phase);
  document.title = t('nav_focus');
}

function start() {
  if (T.running) return;
  if (!T.left) T.total = T.left = lengthOf(T.phase);
  T.endAt = Date.now() + T.left * 1000; T.running = true; T.message = '';
  T.tick = setInterval(step, 500);
  emit();
}
function pause() { if (!T.running) return; clearInterval(T.tick); T.running = false; step(); }
function reset() { clearInterval(T.tick); T.running = false; T.total = T.left = lengthOf(T.phase); emit(); }

export default function focus(view) {
  if (!T.total) T.total = T.left = lengthOf(T.phase);
  const R = 120, C = 2 * Math.PI * R;
  const NS = 'http://www.w3.org/2000/svg';
  const svg = document.createElementNS(NS, 'svg'); svg.setAttribute('viewBox', '0 0 260 260');
  const bg = document.createElementNS(NS, 'circle'); const fg = document.createElementNS(NS, 'circle');
  for (const c of [bg, fg]) { c.setAttribute('cx', '130'); c.setAttribute('cy', '130'); c.setAttribute('r', String(R)); }
  bg.setAttribute('class', 'bg'); fg.setAttribute('class', 'fg'); fg.setAttribute('stroke-dasharray', String(C));
  svg.append(bg, fg);
  const clock = h('b'); const phase = h('small', { class: 'muted' });
  const ring = h('div', { class: 'ring' }, svg, h('div', { class: 'label' }, h('div', null, clock, phase)));
  const toggle = btn(t('start'), { class: 'primary', icon: 'play' });
  toggle.addEventListener('click', () => (T.running ? pause() : start()));
  const message = h('p', { class: 'muted', role: 'status' });
  const s = store.settings();

  const draw = () => {
    const mm = String(Math.floor(T.left / 60)).padStart(2, '0'), ss = String(T.left % 60).padStart(2, '0');
    clock.textContent = `${mm}:${ss}`;
    phase.textContent = T.phase === 'focus' ? t('focus') : t('break');
    fg.setAttribute('stroke-dashoffset', String(C * (1 - (T.total ? T.left / T.total : 0))));
    fg.style.stroke = T.phase === 'focus' ? 'var(--primary)' : 'var(--ok)';
    toggle.replaceChildren(document.createTextNode(T.running ? t('pause') : t('start')));
    message.textContent = T.message || '';
    if (T.running) document.title = `${mm}:${ss} · ${phase.textContent}`;
    drawStats();
  };

  const statsBox = h('div', { class: 'bars' });
  const todayBox = h('div', { class: 'stat' });
  function drawStats() {
    const f = store.load('focus', { sessions: [] });
    const days = [];
    for (let i = 6; i >= 0; i--) { const d = new Date(); d.setDate(d.getDate() - i); days.push(todayISO(d)); }
    const per = days.map((d) => f.sessions.filter((x) => x.date === d).reduce((a, x) => a + x.minutes, 0));
    const max = Math.max(30, ...per);
    statsBox.replaceChildren(...days.map((d, i) => h('div', { title: `${per[i]} ${t('minutes_short')}` },
      h('i', { style: { height: Math.round((per[i] / max) * 90) + 'px' } }),
      t('days_short')[(new Date(d + 'T00:00:00').getDay() + 6) % 7])));
    todayBox.textContent = String(f.sessions.filter((x) => x.date === todayISO()).length);
  }

  view.append(h('div', { class: 'cards' },
    h('section', { class: 'card' },
      h('div', { class: 'timer' },
        seg([['25', '25 / 5'], ['50', '50 / 10'], ['15', '15 / 3']], String(s.focusMin), (v) => {
          const map = { 25: [25, 5], 50: [50, 10], 15: [15, 3] }[v];
          store.setSetting('focusMin', map[0]); store.setSetting('breakMin', map[1]); reset();
        }),
        ring,
        h('div', { class: 'row' }, toggle, btn(t('reset'), { icon: 'refresh', onclick: reset }), btn(t('skip'), { class: 'ghost', onclick: () => { T.endAt = Date.now(); step(); } })),
        message)),
    h('section', { class: 'card' },
      h('header', null, h('h2', null, t('this_week_focus'))),
      statsBox,
      h('div', { style: { marginTop: '1rem' } }, h('small', { class: 'muted' }, t('sessions_today')), todayBox))));

  T.listeners.add(draw);
  draw();
  return () => T.listeners.delete(draw);
}
