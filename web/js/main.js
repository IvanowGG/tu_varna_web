// App start, shell (sidebar / tab bar), routing and theme.

import { h, clear, btn, avatar, banner } from './ui.js';
import { t, setLang, lang } from './i18n.js';
import { icon, msLogo } from './icons.js';
import { api } from './api.js';
import * as store from './store.js';
import { runCaptcha } from './captcha.js';

import dashboard from './views/dashboard.js';
import schedule from './views/schedule.js';
import tasks from './views/tasks.js';
import notes from './views/notes.js';
import grades from './views/grades.js';
import docs from './views/docs.js';
import focus from './views/focus.js';
import links from './views/links.js';
import settings from './views/settings.js';
import profile from './views/profile.js';

// authNotice: a sign-in error code for someone who stays signed in (shown once as a banner).
export const state = { me: null, authNotice: null };

const ROUTES = {
  dashboard: { view: dashboard, icon: 'home', label: 'nav_dashboard' },
  schedule: { view: schedule, icon: 'calendar', label: 'nav_schedule' },
  tasks: { view: tasks, icon: 'tasks', label: 'nav_tasks' },
  notes: { view: notes, icon: 'notes', label: 'nav_notes' },
  grades: { view: grades, icon: 'grades', label: 'nav_grades' },
  docs: { view: docs, icon: 'sparkle', label: 'nav_docs' },
  focus: { view: focus, icon: 'timer', label: 'nav_focus' },
  links: { view: links, icon: 'link', label: 'nav_links' },
  settings: { view: settings, icon: 'settings', label: 'nav_settings' },
  profile: { view: profile, icon: 'user', label: 'nav_profile' },
};
const SIDE = ['dashboard', 'schedule', 'tasks', 'notes', 'grades', 'docs', 'focus', '|', 'links', 'settings'];
const TABS = ['dashboard', 'schedule', 'tasks', 'docs', 'more'];

// ---------- Theme & language ----------
export function applyPrefs(p = store.prefs()) {
  document.documentElement.dataset.theme = p.theme;
  setLang(p.lang);
}
export function setTheme(theme) { const p = { ...store.prefs(), theme }; store.setPrefs(p); applyPrefs(p); }
export function setLanguage(l) { const p = { ...store.prefs(), lang: l }; store.setPrefs(p); applyPrefs(p); render(); }

function themeButton() {
  const cur = store.prefs().theme;
  const dark = cur === 'dark' || (cur === 'auto' && matchMedia('(prefers-color-scheme: dark)').matches);
  return btn(t('theme'), { icon: dark ? 'sun' : 'moon', iconOnly: true, class: 'ghost icon', title: t('theme'), onclick: () => { setTheme(dark ? 'light' : 'dark'); render(); } });
}
function langButton() {
  const b = btn(lang() === 'bg' ? 'EN' : 'БГ', { class: 'ghost small', title: t('language'), onclick: () => setLanguage(lang() === 'bg' ? 'en' : 'bg') });
  b.setAttribute('lang', lang() === 'bg' ? 'en' : 'bg');
  return b;
}

// ---------- Routing ----------
function routeName() {
  const r = (location.hash.replace(/^#\/?/, '').split(/[/?]/)[0]) || 'dashboard';
  return ROUTES[r] || r === 'more' ? r : 'dashboard';
}

let cleanup = null;
export async function render() {
  const app = document.getElementById('app');
  if (cleanup) { try { cleanup(); } catch { /* view already gone */ } cleanup = null; }
  clear(app);
  if (!state.me || !state.me.signed_in) { app.append(landing()); return; }
  const name = routeName();
  app.append(shell(name));
}

function navLink(name, cur, cls) {
  const r = ROUTES[name];
  return h('a', { href: '#/' + name, 'aria-current': name === cur ? 'page' : null, class: cls },
    icon(r.icon), h('span', null, t(r.label)));
}

function shell(cur) {
  const me = state.me.user;
  const side = h('aside', { class: 'sidebar' },
    h('a', { class: 'brand', href: '#/dashboard' }, h('div', { class: 'logo' }, 'ТУ'), h('div', null, t('app_name'), h('small', null, t('app_sub')))),
    h('nav', { class: 'nav', 'aria-label': 'main' }, SIDE.map((n) => n === '|' ? h('div', { class: 'sep' }) : navLink(n, cur))),
    h('a', { class: 'side-user', href: '#/profile' }, avatar(me), h('div', { class: 'who' }, h('b', null, me.name), h('small', { class: 'muted' }, me.email))));

  const tabbar = h('nav', { class: 'tabbar', 'aria-label': 'main' }, TABS.map((n) => n === 'more'
    ? h('a', { href: '#/more', 'aria-current': cur === 'more' || ['notes', 'grades', 'focus', 'links', 'settings', 'profile'].includes(cur) ? 'page' : null }, icon('more'), h('span', null, t('nav_more')))
    : navLink(n, cur)));

  const view = h('section', { class: 'view' });
  const title = cur === 'more' ? t('nav_more') : t(ROUTES[cur].label);
  const actions = h('div', { class: 'actions' }, langButton(), themeButton(),
    h('a', { href: '#/profile', class: 'only-mobile', 'aria-label': t('nav_profile') }, avatar(me)));
  const top = h('header', { class: 'topbar' }, h('h1', null, title), actions);
  const main = h('main', { class: 'main' }, top, view);

  // A failed re-sign-in must be explained, not swallowed.
  if (state.authNotice) view.append(authErrorBanner(state.authNotice, t('perm_error_title')));
  if (cur === 'more') view.append(moreMenu());
  else {
    try {
      const r = ROUTES[cur].view(view, { state, render, actions });
      if (typeof r === 'function') cleanup = r;
    } catch (e) {
      console.error(e);
      view.append(banner('danger', 'alert', String(e && e.message || e)));
    }
  }
  document.title = `${title} · ${t('app_name')} ${t('app_sub')}`;
  return h('div', { class: 'shell' }, side, main, tabbar);
}

function moreMenu() {
  return h('div', { class: 'list' }, ['notes', 'grades', 'focus', 'links', 'settings', 'profile'].map((n) =>
    h('a', { class: 'item', href: '#/' + n }, icon(ROUTES[n].icon), h('div', { class: 'body' }, h('b', null, t(ROUTES[n].label))), icon('right'))));
}

// ---------- Landing (signed out) ----------
function authErrorBanner(code, title) {
  const known = ['captcha', 'session', 'expired', 'cancelled', 'consent', 'domain', 'not_configured', 'rate_limited'];
  return banner('danger', 'alert', h('b', null, title + ': '), t(known.includes(code) ? 'ae_' + code : 'ae_generic'));
}
function authError() {
  const code = new URLSearchParams(location.search).get('auth_error');
  return code ? authErrorBanner(code, t('auth_error_title')) : null;
}

async function signIn(kind) {
  const ok = await runCaptcha();
  if (!ok) return;
  if (kind === 'demo') {
    const r = await api('/auth/demo', { method: 'POST' });
    if (r.ok) { await boot(); location.hash = '#/dashboard'; }
  } else {
    location.href = '/auth/login';
  }
}

function landing() {
  const auth = state.me ? state.me.auth : { microsoft: false, demo: false };
  const feature = (ic, title, text) => h('div', { class: 'feature' }, h('div', { class: 'ic' }, icon(ic)), h('h3', null, title), h('p', null, text));
  const msBtn = h('button', { class: 'btn ms', type: 'button', disabled: !auth.microsoft || null, onclick: () => signIn('ms') }, msLogo(), t('sign_in_ms'));
  const mock = h('div', { class: 'mock', 'aria-hidden': 'true' },
    h('div', { class: 'bar' }, h('i'), h('i'), h('i')),
    h('div', { class: 'list' },
      mockLesson('lecture', '09:15 – 11:00', 'Web дизайн', 'доц. Г. Маринова · 201М'),
      mockLesson('lab', '11:15 – 12:45', 'Операционни системи', '1а · 513ІІЕ'),
      mockLesson('seminar', '13:15 – 15:00', 'Дискретни структури', 'НУК 316'),
      h('div', { class: 'item' }, icon('flag'), h('div', { class: 'body' }, h('b', null, 'Курсов проект — ООП'), h('small', { class: 'muted' }, 'след 3 дни')), h('span', { class: 'pill warn' }, '3д'))));
  return h('div', { class: 'landing' },
    h('div', { class: 'top' },
      h('div', { class: 'brand' }, h('div', { class: 'logo' }, 'ТУ'), h('div', null, t('app_name'), h('small', null, t('app_sub')))),
      h('div', { class: 'row' }, langButton(), themeButton())),
    h('section', { class: 'hero' },
      h('div', { class: 'hero-inner' },
        h('div', null,
          authError(),
          h('h1', null, t('hero_title_1'), h('br'), h('em', null, t('hero_title_2'))),
          h('p', { class: 'lead' }, t('hero_lead')),
          h('div', { class: 'cta' }, msBtn, auth.demo ? btn(t('try_demo'), { class: 'small', icon: 'play', onclick: () => signIn('demo') }) : null),
          !auth.microsoft ? h('p', { class: 'faint' }, h('small', null, t('ms_not_configured'))) : null,
          h('div', { class: 'trust' },
            h('span', null, icon('lock'), t('trust_local')), h('span', null, icon('shield'), t('trust_no_pw')), h('span', null, icon('check'), t('trust_captcha')))),
        mock)),
    h('section', { class: 'features' },
      feature('calendar', t('f_schedule'), t('f_schedule_d')),
      feature('sparkle', t('f_docs'), t('f_docs_d')),
      feature('tasks', t('f_tasks'), t('f_tasks_d')),
      feature('grades', t('f_grades'), t('f_grades_d')),
      feature('timer', t('f_focus'), t('f_focus_d')),
      feature('lock', t('f_private'), t('f_private_d'))),
    h('footer', { class: 'foot' }, '© ', String(new Date().getFullYear()), ' · ', t('app_name'), ' · ', t('app_sub')));
}

function mockLesson(kind, time, subj, meta) {
  return h('div', { class: 'lesson k-' + kind }, h('span', { class: 't' }, time), h('span', { class: 's' }, subj), h('span', { class: 'm' }, meta));
}

// ---------- Session ----------
export async function signOut() {
  const r = await api('/auth/logout', { method: 'POST' });
  state.me = null;
  history.replaceState(null, '', '/');
  await boot();
  // Also end the Microsoft session, so the next person on a shared PC is not signed in.
  if (r.ok && r.data.microsoft_logout && state.me && state.me.auth.microsoft) location.href = r.data.microsoft_logout;
}

async function boot() {
  const r = await api('/api/me');
  state.me = r.ok ? r.data : { signed_in: false, auth: { microsoft: false, demo: false } };
  store.setOwner(state.me.signed_in ? state.me.storage_key : 'guest');
  const failed = new URLSearchParams(location.search).get('auth_error');
  if (state.me.signed_in && failed) {
    // Still signed in (the old session survives a failed re-sign-in): keep the error to show it.
    state.authNotice = failed;
    history.replaceState(null, '', '/' + location.hash);
  }
  render();
}

applyPrefs();
window.addEventListener('hashchange', () => { state.authNotice = null; render(); });
matchMedia('(prefers-color-scheme: dark)').addEventListener('change', () => { if (store.prefs().theme === 'auto') render(); });
boot();

if ('serviceWorker' in navigator && location.protocol === 'https:') {
  navigator.serviceWorker.register('/sw.js').catch(() => { /* optional */ });
}
