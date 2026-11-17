import { h, btn, seg, select, field, empty, modal, confirmBox, todayISO, daysBetween, relDays, fmtDate, uid, clear } from '../ui.js';
import { t } from '../i18n.js';
import { icon } from '../icons.js';
import * as store from '../store.js';
import * as tt from '../timetable.js';

const TYPES = ['task', 'homework', 'exam', 'project'];
const PRIOS = ['low', 'normal', 'high'];

function subjects() {
  const s = new Set();
  (tt.saved()?.timetable?.lessons || []).forEach((l) => l.subject && s.add(l.subject));
  store.load('grades', []).forEach((g) => g.subject && s.add(g.subject));
  return [...s].sort((a, b) => a.localeCompare(b));
}

function editor(task, onSave) {
  const x = { id: uid(), title: '', subject: '', due: todayISO(), type: 'task', priority: 'normal', done: false, notes: '', created: Date.now(), ...task };
  const title = h('input', { value: x.title, required: true, maxlength: '200' });
  const subj = h('input', { value: x.subject, list: 'subjects-list', maxlength: '120' });
  const due = h('input', { type: 'date', value: x.due || '' });
  const notes = h('textarea', { rows: '3', maxlength: '4000' }); notes.value = x.notes || '';
  let type = x.type, prio = x.priority;
  const form = h('form', { onsubmit: (e) => {
    e.preventDefault();
    if (!title.value.trim()) { title.focus(); return; }
    onSave({ ...x, title: title.value.trim(), subject: subj.value.trim(), due: due.value || '', type, priority: prio, notes: notes.value });
    m.close();
  } },
    h('datalist', { id: 'subjects-list' }, subjects().map((s) => h('option', { value: s }))),
    field(t('title'), title),
    h('div', { class: 'grid2' }, field(t('subject'), subj), field(t('due'), due)),
    field(t('type'), seg(TYPES.map((k) => [k, t('t_' + k)]), type, (v) => { type = v; }, t('type'))),
    field(t('priority'), seg(PRIOS.map((k) => [k, t('p_' + k)]), prio, (v) => { prio = v; }, t('priority'))),
    field(t('details'), notes),
    h('div', { class: 'row end' }, btn(t('cancel'), { onclick: () => m.close() }), btn(t('save'), { type: 'submit', class: 'primary' })));
  const m = modal(task ? t('edit') : t('new_task'), form);
}

export default function tasks(view) {
  let filter = 'open';
  const listBox = h('div');
  const all = () => store.load('tasks', []);
  const put = (list) => { store.save('tasks', list); draw(); };
  const upsert = (x) => { const list = all(); const i = list.findIndex((y) => y.id === x.id); if (i >= 0) list[i] = x; else list.push(x); put(list); };

  const quick = h('input', { placeholder: t('quick_add'), 'aria-label': t('quick_add'), onkeydown: (e) => {
    if (e.key === 'Enter' && quick.value.trim()) {
      upsert({ id: uid(), title: quick.value.trim(), subject: '', due: '', type: 'task', priority: 'normal', done: false, created: Date.now() });
      quick.value = '';
    }
  } });

  function row(x) {
    const cb = h('input', { type: 'checkbox', checked: x.done, 'aria-label': x.title, onchange: () => upsert({ ...x, done: cb.checked, doneAt: cb.checked ? Date.now() : null }) });
    const overdue = !x.done && x.due && x.due < todayISO();
    const meta = [x.subject, x.due ? `${fmtDate(x.due, { weekday: 'short', day: 'numeric', month: 'short' })} (${relDays(x.due)})` : ''].filter(Boolean).join(' · ');
    return h('div', { class: 'item' + (x.done ? ' done' : '') },
      cb,
      h('div', { class: 'body', role: 'button', tabindex: '0', onclick: () => editor(x, upsert), onkeydown: (e) => { if (e.key === 'Enter') editor(x, upsert); } },
        h('b', null, x.title), meta ? h('small', { class: overdue ? '' : 'muted', style: overdue ? { color: 'var(--danger)' } : null }, meta) : null),
      x.type !== 'task' ? h('span', { class: 'pill ' + (x.type === 'exam' ? 'primary' : '') }, t('t_' + x.type)) : null,
      x.priority === 'high' && !x.done ? h('span', { class: 'pill danger' }, '!') : null,
      btn(t('delete'), { icon: 'trash', iconOnly: true, class: 'ghost icon small', onclick: async () => { if (await confirmBox(x.title)) put(all().filter((y) => y.id !== x.id)); } }));
  }

  function draw() {
    clear(listBox);
    let list = all();
    if (filter === 'open') list = list.filter((x) => !x.done);
    if (filter === 'exams') list = list.filter((x) => x.type === 'exam');
    if (!list.length) { listBox.append(empty('tasks', t('no_tasks'))); return; }
    const today = todayISO();
    const groups = { overdue: [], today: [], week: [], later: [], nodate: [], done: [] };
    for (const x of list) {
      if (x.done) groups.done.push(x);
      else if (!x.due) groups.nodate.push(x);
      else if (x.due < today) groups.overdue.push(x);
      else if (x.due === today) groups.today.push(x);
      else if (daysBetween(today, x.due) <= 7) groups.week.push(x);
      else groups.later.push(x);
    }
    const prio = { high: 0, normal: 1, low: 2 };
    for (const [g, items] of Object.entries(groups)) {
      if (!items.length) continue;
      items.sort((a, b) => (a.due || '9').localeCompare(b.due || '9') || prio[a.priority] - prio[b.priority]);
      listBox.append(h('div', { class: 'group-title' }, `${t('g_' + g)} · ${items.length}`), h('div', { class: 'list' }, items.map(row)));
    }
    if (groups.done.length) listBox.append(h('div', { class: 'row end', style: { marginTop: '.6rem' } },
      btn(t('clear_done'), { class: 'ghost small', icon: 'trash', onclick: () => put(all().filter((x) => !x.done)) })));
  }

  view.append(
    h('div', { class: 'row between', style: { marginBottom: '1rem' } },
      seg([['open', t('filter_open')], ['all', t('filter_all')], ['exams', t('filter_exams')]], filter, (v) => { filter = v; draw(); }),
      btn(t('new_task'), { class: 'primary', icon: 'plus', onclick: () => editor(null, upsert) })),
    h('div', { class: 'card' }, quick, listBox));
  draw();
}
