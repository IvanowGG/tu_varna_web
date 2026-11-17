import { h, btn, seg, select, field, banner, empty, fmtDate, clear } from '../ui.js';
import { t } from '../i18n.js';
import { icon } from '../icons.js';
import { api } from '../api.js';
import * as store from '../store.js';
import * as tt from '../timetable.js';

function errorText(code) {
  if (code === 'host_not_allowed' || code === 'bad_url') return t('err_host_not_allowed');
  if (code === 'no_timetable_for_group') return t('err_no_timetable_for_group');
  if (['not_a_timetable', 'no_header', 'no_time_slots', 'no_days'].includes(code)) return t('err_parse');
  if (code === 'rate_limited') return t('ae_rate_limited');
  return t('err_upstream');
}

export default function schedule(view, ctx) {
  const saved = tt.saved();
  if (!saved || location.hash.includes('setup')) return setup(view, ctx, saved);
  return show(view, ctx, saved);
}

// ---------- Choose the group ----------
function setup(view, ctx, saved) {
  const m = new Date().getMonth();
  const src = saved?.source || {};
  let semester = src.semester || (m >= 1 && m <= 7 ? 'summer' : 'winter');
  const msg = h('div');
  const specSel = select([['', t('loading')]], '', null, { disabled: true, 'aria-label': t('specialty') });
  const kursSel = select([['', '—']], '', null, { disabled: true, 'aria-label': t('course') });
  const grupaSel = select([['', '—']], '', null, { disabled: true, 'aria-label': t('group') });
  const go = btn(t('load_schedule'), { class: 'primary', icon: 'calendar', onclick: () => resolve() });
  let opts = null;

  async function loadOptions() {
    clear(msg);
    [specSel, kursSel, grupaSel].forEach((s) => { s.disabled = true; });
    const r = await api('/api/schedule/options', { method: 'POST', body: { semester } });
    if (!r.ok) { msg.append(banner('danger', 'alert', errorText(r.data.error), ' ', btn(t('retry'), { class: 'small', onclick: loadOptions }))); return; }
    opts = r.data;
    const fill = (sel, list, cur) => {
      clear(sel);
      list.forEach((o) => sel.append(h('option', { value: o.value }, o.label)));
      if (cur && list.some((o) => o.value === cur)) sel.value = cur;
      sel.disabled = false;
    };
    fill(specSel, opts.spec, src.spec);
    fill(kursSel, opts.kurs.filter((o) => o.value !== '6'), src.kurs);
    fill(grupaSel, opts.grupa, src.grupa);
  }

  async function resolve() {
    clear(msg);
    go.disabled = true;
    go.replaceChildren(h('span', { class: 'spinner' }), t('loading'));
    const body = { semester, spec: specSel.value, kurs: kursSel.value, grupa: grupaSel.value };
    const r = await api('/api/schedule/resolve', { method: 'POST', body });
    go.disabled = false;
    go.replaceChildren(icon('calendar'), t('load_schedule'));
    if (!r.ok) { msg.append(banner('danger', 'alert', errorText(r.data.error))); return; }
    const specLabel = specSel.selectedOptions[0]?.textContent || '';
    store.save('timetable', { ...r.data, source: { ...body, specLabel } });
    location.hash = '#/schedule';
    ctx.render();
  }

  const urlInput = h('input', { type: 'url', placeholder: 'http://umo.tu-varna.bg/components/…htm', inputmode: 'url' });
  async function fromUrl() {
    clear(msg);
    const r = await api('/api/schedule/fetch', { method: 'POST', body: { url: urlInput.value.trim() } });
    if (!r.ok) { msg.append(banner('danger', 'alert', errorText(r.data.error))); return; }
    store.save('timetable', { ...r.data, source: { url: r.data.url } });
    location.hash = '#/schedule';
    ctx.render();
  }

  view.append(h('div', { class: 'cards' },
    h('section', { class: 'card' },
      h('header', null, h('h2', null, t('sch_setup')), saved ? h('a', { class: 'btn small ghost', href: '#/schedule' }, t('cancel')) : null),
      field(t('semester'), seg([['winter', t('winter')], ['summer', t('summer')]], semester, (v) => { semester = v; loadOptions(); }, t('semester'))),
      field(t('specialty'), specSel),
      h('div', { class: 'grid2' }, field(t('course'), kursSel), field(t('group'), grupaSel)),
      msg,
      h('div', { class: 'row end' }, go)),
    h('section', { class: 'card' },
      h('header', null, h('h3', null, t('or_paste_url'))),
      field(t('url'), urlInput),
      h('div', { class: 'row end' }, btn(t('load'), { onclick: fromUrl })),
      h('p', { class: 'faint', style: { marginTop: '1rem' } }, h('small', null, 'umo.tu-varna.bg → ', t('l_umo'))))));
  loadOptions();
}

// ---------- Show the week ----------
function show(view, ctx, saved) {
  const data = saved.timetable;
  const s = store.settings();
  let offset = 0; // weeks from now
  let allWeeks = false;
  const groups = tt.subgroups(saved);

  const meta = data.meta || {};
  const title = [meta.specialty || saved.source?.specLabel, meta.course && `${meta.course} ${t('course').toLowerCase()}`, meta.group && `${t('group').toLowerCase()} ${meta.group}`].filter(Boolean).join(' · ');
  const status = h('small', { class: 'muted' }, t('updated', new Date((saved.fetched_at || 0) * 1000).toLocaleString()));
  const msg = h('div');

  async function refresh() {
    clear(msg);
    const src = saved.source || {};
    const r = src.spec
      ? await api('/api/schedule/resolve', { method: 'POST', body: { semester: src.semester, spec: src.spec, kurs: src.kurs, grupa: src.grupa } })
      : await api('/api/schedule/fetch', { method: 'POST', body: { url: saved.url } });
    if (!r.ok) { msg.append(banner(r.status === 0 ? 'info' : 'danger', 'alert', r.status === 0 ? t('offline') : errorText(r.data.error))); return; }
    store.save('timetable', { ...r.data, source: saved.source });
    ctx.render();
  }

  const weekLabel = h('b');
  const grid = h('div', { class: 'week' });
  const draw = () => {
    clear(grid);
    const base = new Date();
    base.setDate(base.getDate() + offset * 7);
    const week = store.weekNumber(base);
    const par = tt.parityOf(week);
    weekLabel.textContent = par ? `${t('week_n', week)} · ${t(par === 'odd' ? 'odd_week' : 'even_week')}` : t('before_semester');
    const monday = new Date(base); monday.setDate(base.getDate() - ((base.getDay() + 6) % 7));
    const maxDay = data.lessons.some((l) => l.day === 6) ? (data.lessons.some((l) => l.day === 7) ? 7 : 6) : 5;
    const todayKey = new Date().toDateString();
    for (let d = 1; d <= maxDay; d++) {
      const date = new Date(monday); date.setDate(monday.getDate() + d - 1);
      const list = tt.lessonsFor(d, week, { subgroup: store.settings().subgroup, allParities: allWeeks }).sort((a, b) => tt.minutes(a.start) - tt.minutes(b.start));
      const isToday = date.toDateString() === todayKey;
      grid.append(h('div', { class: 'day' + (isToday ? ' today' : '') },
        h('h3', null, h('span', null, t('days')[d - 1]), h('small', { class: 'muted' }, fmtDate(date))),
        list.length ? list.map((l) => tt.lessonCard(l)) : h('p', { class: 'faint', style: { margin: '.3rem 0' } }, h('small', null, '—'))));
    }
  };

  const subSeg = groups.length
    ? seg([['', t('all')], ...groups.map((g) => [g, g])], s.subgroup && groups.includes(s.subgroup) ? s.subgroup : '', (v) => { store.setSetting('subgroup', v); draw(); }, t('subgroup'))
    : null;
  const weekSeg = seg([['this', t('this_week_only')], ['all', t('every_week')]], 'this', (v) => { allWeeks = v === 'all'; draw(); }, t('show'));

  view.append(
    h('div', { class: 'row between', style: { marginBottom: '1rem' } },
      h('div', null, h('h2', { style: { margin: 0 } }, title || t('nav_schedule')),
        h('small', { class: 'muted' }, [meta.semester, meta.year].filter(Boolean).join(' · '))),
      h('div', { class: 'row' },
        btn(t('refresh'), { icon: 'refresh', class: 'small', onclick: refresh }),
        h('a', { class: 'btn small', href: '#/schedule/setup' }, icon('edit'), t('change_group')))),
    msg,
    h('div', { class: 'card', style: { marginBottom: '1rem' } },
      h('div', { class: 'row between' },
        h('div', { class: 'row' },
          btn(t('week_prev'), { icon: 'left', iconOnly: true, class: 'icon small', onclick: () => { offset--; draw(); } }),
          weekLabel,
          btn(t('week_next'), { icon: 'right', iconOnly: true, class: 'icon small', onclick: () => { offset++; draw(); } }),
          btn(t('this_week'), { class: 'ghost small', onclick: () => { offset = 0; draw(); } })),
        h('div', { class: 'row' }, subSeg ? h('span', { class: 'muted' }, h('small', null, t('subgroup'))) : null, subSeg, weekSeg)),
      h('p', { class: 'faint', style: { margin: '.6rem 0 0' } }, h('small', null, t('sem_start_hint', fmtDate(s.semesterStart, { day: 'numeric', month: 'long', year: 'numeric' }))))),
    grid,
    h('div', { class: 'legend', style: { margin: '1rem 0' } },
      ['lecture', 'lab', 'seminar', 'exercise', 'project'].map((k) => h('span', { class: 'k-' + k }, h('i', { class: 'kdot' }), tt.kindLabel(k)))),
    data.notes && data.notes.length ? h('section', { class: 'card flat', style: { marginTop: '1rem' } },
      h('h3', null, t('sch_notes')), h('ul', { class: 'muted' }, data.notes.map((n) => h('li', null, n)))) : null,
    h('p', { class: 'faint', style: { marginTop: '1rem' } }, h('small', null, t('source'), ': ',
      h('a', { href: saved.url, target: '_blank', rel: 'noopener noreferrer' }, saved.url), ' · ', status)));

  if (!data.lessons.length) view.append(empty('calendar', t('no_lessons')));
  draw();

  // Teams/Outlook calendar, if the student granted it at sign-in.
  const granted = (ctx.state.me.user.granted || []).map((g) => g.toLowerCase());
  if (granted.includes('calendars.read')) view.append(msCalendar());
}

function msCalendar() {
  const box = h('section', { class: 'card', style: { marginTop: '1rem' } }, h('header', null, h('h2', null, t('ms_calendar'))), h('div', { class: 'skeleton' }));
  api('/api/ms/calendar?days=14').then((r) => {
    box.lastChild.remove();
    if (!r.ok) { box.append(h('p', { class: 'muted' }, t('ms_calendar_none'))); return; }
    const ev = r.data.events || [];
    if (!ev.length) { box.append(h('p', { class: 'muted' }, '—')); return; }
    box.append(h('div', { class: 'list' }, ev.map((e) => h('div', { class: 'item' }, icon(e.online ? 'users' : 'calendar'),
      h('div', { class: 'body' }, h('b', null, e.subject || '—'),
        h('small', { class: 'muted' }, `${fmtDate(new Date(e.start), { weekday: 'short', day: 'numeric', month: 'short', hour: '2-digit', minute: '2-digit' })}${e.location ? ' · ' + e.location : ''}`)),
      e.join_url && String(e.join_url).startsWith('https://teams.microsoft.com/') ? h('a', { class: 'btn small', href: e.join_url, target: '_blank', rel: 'noopener noreferrer' }, 'Teams') : null))));
  });
  return box;
}
