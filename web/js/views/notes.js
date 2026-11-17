import { h, btn, seg, empty, confirmBox, debounce, uid, clear, fmtDate } from '../ui.js';
import { t } from '../i18n.js';
import { icon } from '../icons.js';
import * as store from '../store.js';
import { renderMarkdown } from '../md.js';

export default function notes(view) {
  let current = null;
  let mode = 'write';
  let query = '';
  const all = () => store.load('notes', []);
  const listBox = h('div', { class: 'list' });
  const pane = h('div');

  const persist = debounce((note, statusEl) => {
    const list = all();
    const i = list.findIndex((n) => n.id === note.id);
    note.updated = Date.now();
    if (i >= 0) list[i] = note; else list.unshift(note);
    store.save('notes', list);
    statusEl.textContent = `${t('saved_locally')} · ${new Date().toLocaleTimeString()}`;
    drawList();
  }, 400);

  function drawList() {
    clear(listBox);
    const q = query.toLowerCase();
    const list = all().filter((n) => !q || (n.title + ' ' + n.body + ' ' + (n.subject || '')).toLowerCase().includes(q)).sort((a, b) => b.updated - a.updated);
    if (!list.length) { listBox.append(h('p', { class: 'muted' }, t('no_notes'))); return; }
    for (const n of list) {
      listBox.append(h('a', { class: 'item', href: '#/notes', 'aria-current': current && current.id === n.id ? 'true' : null,
        style: current && current.id === n.id ? { borderColor: 'var(--primary)' } : null,
        onclick: (e) => { e.preventDefault(); open(n); } },
        icon('notes'), h('div', { class: 'body' }, h('b', null, n.title || t('untitled')), h('small', { class: 'muted' }, [n.subject, fmtDate(new Date(n.updated))].filter(Boolean).join(' · ')))));
    }
  }

  function open(n) {
    current = { ...n };
    clear(pane);
    const status = h('div', { class: 'status' });
    const title = h('input', { class: 'doc-title', value: current.title, placeholder: t('untitled'), 'aria-label': t('title'),
      oninput: () => { current.title = title.value; persist(current, status); } });
    const subj = h('input', { value: current.subject || '', placeholder: t('subject'), 'aria-label': t('subject'), maxlength: '120',
      oninput: () => { current.subject = subj.value; persist(current, status); } });
    const ta = h('textarea', { class: 'editor', 'aria-label': t('nav_notes'), oninput: () => { current.body = ta.value; persist(current, status); } });
    ta.value = current.body || '';
    const prev = h('div', { class: 'paper prose' });
    const body = h('div');
    const showMode = () => { clear(body); if (mode === 'write') body.append(ta); else body.append(renderMarkdown(current.body, prev)); };
    pane.append(h('div', { class: 'card' },
      h('div', { class: 'row between' }, title,
        h('div', { class: 'row' },
          seg([['write', t('write')], ['preview', t('preview')]], mode, (v) => { mode = v; showMode(); }),
          btn(t('delete'), { icon: 'trash', iconOnly: true, class: 'ghost icon small', onclick: async () => {
            if (!(await confirmBox(current.title || t('untitled')))) return;
            store.save('notes', all().filter((x) => x.id !== current.id));
            current = null; clear(pane); pane.append(empty('notes', t('pick_note'))); drawList();
          } }))),
      h('div', { style: { margin: '.5rem 0 .8rem' } }, subj),
      body, status));
    showMode();
    drawList();
    if (!current.body) ta.focus();
  }

  const newNote = () => { const n = { id: uid(), title: '', subject: '', body: '', updated: Date.now() }; store.save('notes', [n, ...all()]); open(n); };
  const search = h('input', { type: 'search', placeholder: t('search'), 'aria-label': t('search'), oninput: () => { query = search.value; drawList(); } });

  view.append(h('div', { class: 'split' },
    h('div', null,
      h('div', { class: 'row', style: { marginBottom: '.7rem' } }, h('div', { class: 'grow' }, search), btn(t('new_note'), { icon: 'plus', iconOnly: true, class: 'primary icon', onclick: newNote })),
      listBox),
    pane));
  drawList();
  const first = all().sort((a, b) => b.updated - a.updated)[0];
  if (first) open(first); else pane.append(empty('notes', t('pick_note'), btn(t('new_note'), { class: 'primary', icon: 'plus', onclick: newNote })));
}
