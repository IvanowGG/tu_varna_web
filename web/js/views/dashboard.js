import { h, btn, empty, fmtDate, todayISO, relDays, daysBetween, uid } from '../ui.js';
import { t } from '../i18n.js';
import { icon } from '../icons.js';
import * as store from '../store.js';
import * as tt from '../timetable.js';
import { average } from './grades.js';

export default function dashboard(view, { state }) {
  const me = state.me.user;
  const now = new Date();
  const hour = now.getHours();
  const greet = hour < 11 ? t('good_morning') : hour < 18 ? t('good_day') : t('good_evening');
  const week = store.weekNumber(now);
  const par = tt.parityOf(week);
  const first = me.given_name || (me.name || '').split(' ')[0];

  view.append(h('div', { class: 'row between', style: { marginBottom: '1rem' } },
    h('div', null,
      h('h2', { style: { fontSize: '1.5rem', margin: 0 } }, `${greet}, ${first}!`),
      h('p', { class: 'muted', style: { margin: '.2rem 0 0' } },
        fmtDate(now, { weekday: 'long', day: 'numeric', month: 'long' }), ' · ',
        par ? `${t('week_n', week)} (${t(par === 'odd' ? 'odd_week' : 'even_week')})` : t('before_semester'),
        me.department ? ' · ' + me.department : ''))));

  const grid = h('div', { class: 'cards three' });
  view.append(grid);

  // Today's classes + next.
  const todayCard = h('section', { class: 'card span2' });
  const fillToday = () => {
    todayCard.replaceChildren();
    const saved = tt.saved();
    todayCard.append(h('header', null, h('h2', null, t('today_classes')), h('a', { class: 'btn small', href: '#/schedule' }, t('open'))));
    if (!saved) {
      todayCard.append(empty('calendar', t('set_up_schedule'), h('a', { class: 'btn primary', href: '#/schedule' }, t('set_up'))));
      return;
    }
    const day = ((now.getDay() + 6) % 7) + 1;
    const list = tt.lessonsFor(day, week).sort((a, b) => tt.minutes(a.start) - tt.minutes(b.start));
    const nn = tt.nowNext(new Date());
    if (nn) {
      todayCard.append(h('div', { class: 'next', style: { marginBottom: '.8rem' } },
        h('div', { class: 'clock' }, tt.fmtCountdown(nn.minutesLeft)),
        h('div', null,
          h('small', { class: 'muted' }, nn.state === 'now' ? `${t('now_in')} · ${t('ends_in')}` : `${t('next_class')} · ${t('starts_in')}`),
          h('div', null, h('b', null, nn.lesson.subject), nn.lesson.room ? ' · ' + nn.lesson.room : ''))));
    } else if (list.length) {
      todayCard.append(h('p', { class: 'muted' }, t('no_more_today')));
    }
    if (!list.length) { todayCard.append(empty('coffee', t('no_classes_today'))); return; }
    const row = h('div', { class: 'week', style: { gridTemplateColumns: 'repeat(auto-fill, minmax(220px, 1fr))' } });
    const nowM = now.getHours() * 60 + now.getMinutes();
    list.forEach((l) => row.append(tt.lessonCard(l, { now: nn && nn.lesson === l && nn.state === 'now', dim: tt.minutes(l.end) <= nowM })));
    todayCard.append(row);
  };
  fillToday();
  grid.append(todayCard);

  // Tasks due soon: tick a task off right here; overdue dates in red.
  const tasks = store.load('tasks', []);
  const open = tasks.filter((x) => !x.done);
  const dueCard = h('section', { class: 'card' });
  const quick = h('input', { placeholder: t('quick_add'), 'aria-label': t('quick_add'), onkeydown: (e) => {
    if (e.key === 'Enter' && quick.value.trim()) {
      const list = store.load('tasks', []);
      const id = uid();
      list.push({ id, title: quick.value.trim(), subject: '', due: todayISO(), type: 'task', priority: 'normal', done: false, created: Date.now() });
      store.save('tasks', list); quick.value = '';
      drawDue(id);
      quick.focus();
    }
  } });
  const MAX_DUE = 6;
  // justAdded: id of a task added here, always shown so a quick add never seems to vanish.
  function drawDue(justAdded) {
    const openNow = store.load('tasks', []).filter((x) => !x.done);
    const overdue = openNow.filter((x) => x.due && x.due < todayISO());
    const allSoon = openNow.filter((x) => x.due && x.type !== 'exam' && daysBetween(todayISO(), x.due) <= 7)
      .sort((a, b) => a.due.localeCompare(b.due));
    let soon = allSoon.slice(0, MAX_DUE);
    const added = justAdded && allSoon.find((x) => x.id === justAdded);
    if (added && !soon.includes(added)) soon = [...soon.slice(0, MAX_DUE - 1), added];
    const hidden = allSoon.length - soon.length;
    const row = (x) => {
      const late = x.due < todayISO();
      const cb = h('input', { type: 'checkbox', 'aria-label': t('mark_done') + ': ' + x.title, onchange: () => {
        const list = store.load('tasks', []);
        const it = list.find((y) => y.id === x.id);
        if (it) { it.done = true; it.doneAt = Date.now(); store.save('tasks', list); }
        drawDue();
      } });
      return h('div', { class: 'item' }, cb,
        h('a', { class: 'body', href: '#/tasks' },
          h('b', null, x.title),
          h('small', { class: late ? 'late' : 'muted' }, [x.subject, relDays(x.due)].filter(Boolean).join(' · '))),
        x.type === 'homework' ? icon('book', 'faint') : null);
    };
    dueCard.replaceChildren(
      h('header', null, h('h2', null, t('due_soon')),
        overdue.length ? h('a', { class: 'pill danger', href: '#/tasks' }, t('overdue_n', overdue.length)) : null),
      soon.length ? h('div', { class: 'list' }, soon.map((x) => {
        const el = row(x);
        if (x.id === justAdded) el.classList.add('fresh');
        return el;
      })) : h('p', { class: 'muted' }, t('nothing_due')),
      hidden > 0 ? h('a', { class: 'more-link', href: '#/tasks' }, t('more_n', hidden)) : null,
      h('div', { style: { marginTop: '.8rem' } }, quick));
  }
  drawDue();
  grid.append(dueCard);

  // Exams countdown.
  const exams = open.filter((x) => x.type === 'exam' && x.due && x.due >= todayISO()).sort((a, b) => a.due.localeCompare(b.due)).slice(0, 4);
  grid.append(h('section', { class: 'card' },
    h('header', null, h('h2', null, t('exams')), h('a', { class: 'btn small', href: '#/tasks' }, t('open'))),
    exams.length ? h('div', { class: 'list' }, exams.map((x) => {
      const d = daysBetween(todayISO(), x.due);
      return h('div', { class: 'item' }, h('div', { class: 'stat', style: { fontSize: '1.4rem', minWidth: '3.2rem', color: d <= 3 ? 'var(--danger)' : 'var(--primary)' } }, d + t('d_short')),
        h('div', { class: 'body' }, h('b', null, x.title), h('small', { class: 'muted' }, fmtDate(x.due, { weekday: 'short', day: 'numeric', month: 'short' }))));
    })) : h('p', { class: 'muted' }, t('no_exams'))));

  // Grades.
  const avg = average(store.load('grades', []));
  grid.append(h('section', { class: 'card' },
    h('header', null, h('h2', null, t('avg_grade')), h('a', { class: 'btn small', href: '#/grades' }, t('open'))),
    avg.count ? h('div', null, h('div', { class: 'stat' }, avg.weighted.toFixed(2)), h('small', { class: 'muted' }, `${avg.count} · ${t('total_credits')}: ${avg.credits}`)) : h('p', { class: 'muted' }, t('no_grades'))));

  // Focus today.
  const focus = store.load('focus', { sessions: [] });
  const mins = focus.sessions.filter((s) => s.date === todayISO()).reduce((a, s) => a + s.minutes, 0);
  grid.append(h('section', { class: 'card' },
    h('header', null, h('h2', null, t('focus_today')), h('a', { class: 'btn small', href: '#/focus' }, t('start'))),
    h('div', { class: 'stat' }, String(mins), h('small', { class: 'muted', style: { fontSize: '1rem', marginLeft: '.3rem' } }, t('minutes_short')))));

  // Keep the countdown live.
  const timer = setInterval(fillToday, 30_000);
  return () => clearInterval(timer);
}
