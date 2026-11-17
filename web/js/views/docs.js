import { h, btn, seg, select, field, banner, confirmBox, debounce, uid, clear, fmtDate } from '../ui.js';
import { t, lang } from '../i18n.js';
import { icon } from '../icons.js';
import { stream, download, saveBlob } from '../api.js';
import * as store from '../store.js';
import { renderMarkdown, wordCount } from '../md.js';

const KINDS = ['essay', 'report', 'coursework', 'lab', 'summary', 'email', 'cv', 'presentation', 'letter', 'notes', 'other'];
const TEXT_ACTIONS = ['improve', 'continue', 'summarize', 'questions', 'translate'];

export default function docs(view, ctx) {
  const ai = ctx.state.me.ai || {};
  const all = () => store.load('docs', []);
  let doc = null;
  let mode = 'write';
  let controller = null;
  let undo = null;

  // ---------- Generator form ----------
  const form = { kind: 'essay', topic: '', details: '', language: lang(), length: 'medium', tone: 'academic', include_profile: true };
  const topic = h('input', { maxlength: '300', placeholder: t('topic'), oninput: () => { form.topic = topic.value; } });
  const details = h('textarea', { rows: '4', maxlength: '6000', placeholder: t('details_ph'), oninput: () => { form.details = details.value; } });
  const kindSel = select(KINDS.map((k) => [k, t('d_' + k)]), form.kind, (v) => { form.kind = v; });
  const profileCb = h('input', { type: 'checkbox', checked: true, onchange: () => { form.include_profile = profileCb.checked; } });
  const msg = h('div');
  const genBtn = btn(t('generate'), { class: 'primary', icon: 'sparkle', onclick: () => run('write') });
  const outlineBtn = btn(t('a_outline'), { icon: 'tasks', onclick: () => run('outline') });
  const stopBtn = btn(t('stop'), { icon: 'stop', class: 'danger', onclick: () => controller && controller.abort() });
  stopBtn.hidden = true;

  const aiState = h('span', { class: 'pill ' + (ai.enabled ? 'ok' : '') }, ai.enabled ? t('ai_on', ai.model) : t('ai_off'));
  const left = h('div', null,
    h('section', { class: 'card' },
      h('header', null, h('h2', null, t('doc_assistant')), aiState),
      field(t('what'), kindSel),
      field(t('topic'), topic),
      field(t('details'), details),
      field(t('language'), seg([['bg', 'Български'], ['en', 'English']], form.language, (v) => { form.language = v; })),
      h('div', { class: 'grid2' },
        field(t('length'), seg([['short', t('short')], ['medium', t('medium')], ['long', t('long')]], form.length, (v) => { form.length = v; })),
        field(t('tone'), select([['academic', t('academic')], ['neutral', t('neutral')], ['friendly', t('friendly')]], form.tone, (v) => { form.tone = v; }))),
      h('label', { class: 'row', style: { marginBottom: '.8rem' } }, profileCb, h('small', null, t('include_profile'))),
      msg,
      h('div', { class: 'row end' }, stopBtn, outlineBtn, genBtn),
      h('p', { class: 'faint', style: { margin: '.8rem 0 0' } }, h('small', null, t('ai_note')))),
    h('section', { class: 'card', style: { marginTop: '1rem' } },
      h('header', null, h('h3', null, t('my_docs')), btn(t('new_doc'), { icon: 'plus', iconOnly: true, class: 'icon small', onclick: () => openDoc(newDoc()) })),
      h('div', { class: 'list', id: 'doc-list' })));

  // ---------- Editor ----------
  const title = h('input', { class: 'doc-title', placeholder: t('untitled'), 'aria-label': t('title'), maxlength: '200', oninput: () => { if (doc) { doc.title = title.value; persist(); } } });
  const ta = h('textarea', { class: 'editor', 'aria-label': t('nav_docs'), oninput: () => { if (doc) { doc.body = ta.value; persist(); updateCount(); } } });
  const paper = h('div', { class: 'paper prose' });
  const body = h('div');
  const status = h('div', { class: 'status' });
  const count = h('small', { class: 'muted' });
  const undoBtn = btn('↶', { class: 'ghost small', title: 'Undo', onclick: () => { if (undo != null && doc) { ta.value = doc.body = undo; undo = null; undoBtn.hidden = true; persist(); showMode(); } } });
  undoBtn.hidden = true;
  const actionBtns = TEXT_ACTIONS.map((a) => btn(t('a_' + a), { class: 'small', onclick: () => run(a) }));

  const updateCount = () => { count.textContent = t('words', wordCount(ta.value)); };
  const showMode = () => {
    clear(body);
    if (mode === 'write') body.append(ta);
    else { renderMarkdown(ta.value, paper); body.append(paper); }
  };

  const persist = debounce(() => {
    if (!doc) return;
    doc.updated = Date.now();
    const list = all();
    const i = list.findIndex((d) => d.id === doc.id);
    if (i >= 0) list[i] = { ...doc }; else list.unshift({ ...doc });
    store.save('docs', list);
    status.textContent = `${t('saved_locally')} · ${new Date().toLocaleTimeString()}`;
    drawList();
  }, 400);

  const right = h('section', { class: 'card' },
    h('div', { class: 'row between' }, title,
      h('div', { class: 'row' },
        seg([['write', t('write')], ['preview', t('preview')]], mode, (v) => { mode = v; showMode(); }),
        btn(t('export_docx'), { icon: 'download', class: 'small', onclick: exportDocx }),
        btn(t('export_md'), { icon: 'file', class: 'small', onclick: exportMd }),
        btn(t('print'), { icon: 'printer', iconOnly: true, class: 'icon small', onclick: printDoc }))),
    h('div', { class: 'row', style: { margin: '.6rem 0' } }, h('small', { class: 'faint' }, t('apply_to_doc')), actionBtns, undoBtn),
    body,
    h('div', { class: 'row between', style: { marginTop: '.4rem' } }, status, count));

  view.append(h('div', { class: 'split wide-left' }, left, right));

  function newDoc() { return { id: uid(), title: '', kind: form.kind, body: '', updated: Date.now() }; }

  function openDoc(d) {
    if (controller) return;
    doc = { ...d };
    title.value = doc.title;
    ta.value = doc.body;
    undo = null; undoBtn.hidden = true;
    status.textContent = '';
    updateCount();
    showMode();
    drawList();
  }

  function drawList() {
    const box = left.querySelector('#doc-list');
    clear(box);
    const list = all().sort((a, b) => b.updated - a.updated);
    if (!list.length) { box.append(h('p', { class: 'muted' }, t('no_docs'))); return; }
    for (const d of list) {
      const active = doc && doc.id === d.id;
      box.append(h('div', { class: 'item', style: active ? { borderColor: 'var(--primary)' } : null },
        icon('file'),
        h('div', { class: 'body', role: 'button', tabindex: '0', onclick: () => openDoc(d), onkeydown: (e) => { if (e.key === 'Enter') openDoc(d); } },
          h('b', null, d.title || t('untitled')), h('small', { class: 'muted' }, `${t('d_' + (d.kind || 'other'))} · ${fmtDate(new Date(d.updated))}`)),
        btn(t('delete'), { icon: 'trash', iconOnly: true, class: 'ghost icon small', onclick: async () => {
          if (!(await confirmBox(d.title || t('untitled')))) return;
          store.save('docs', all().filter((x) => x.id !== d.id));
          if (doc && doc.id === d.id) openDoc(newDoc()); else drawList();
        } })));
    }
  }

  function busy(on, label) {
    [genBtn, outlineBtn, ...actionBtns].forEach((b) => { b.disabled = on; });
    stopBtn.hidden = !on;
    ta.readOnly = on;
    if (on) { status.replaceChildren(h('span', { class: 'spinner' }), ' ', label); status.className = 'status row'; }
    else status.className = 'status';
  }

  async function run(action) {
    clear(msg);
    const textAction = TEXT_ACTIONS.includes(action);
    if (!textAction && !form.topic.trim()) { msg.append(banner('danger', 'alert', t('ai_errors_no_topic'))); topic.focus(); return; }
    if (textAction && !ta.value.trim()) { msg.append(banner('danger', 'alert', t('ai_errors_no_text'))); return; }

    const source = ta.value;
    // Where the output goes: a new document, appended, or replacing the text (with undo).
    if (action === 'write' || action === 'outline' || action === 'summarize' || action === 'questions') {
      const d = newDoc();
      d.kind = action === 'summarize' || action === 'questions' ? 'summary' : form.kind;
      d.title = textAction ? `${t('a_' + action)}: ${doc?.title || t('untitled')}` : form.topic.trim();
      openDoc(d);
    } else if (action === 'improve' || action === 'translate') {
      undo = source; undoBtn.hidden = false;
      ta.value = '';
    } else if (action === 'continue') {
      ta.value = source.replace(/\s*$/, '') + '\n\n';
    }
    mode = 'preview'; showMode();
    paper.classList.add('typing');

    controller = new AbortController();
    busy(true, ai.enabled ? t('thinking') : t('template_mode'));
    let got = '';
    const flush = () => { doc.body = ta.value; renderMarkdown(ta.value, paper); updateCount(); };
    try {
      await stream('/api/ai/generate', {
        action, kind: form.kind, topic: form.topic, details: form.details, language: form.language,
        length: form.length, tone: form.tone, include_profile: form.include_profile, text: textAction ? source : '',
      }, (ev, data) => {
        if (ev === 'status' && data && data.mode === 'thinking') busy(true, t('thinking'));
        if (ev === 'text') {
          if (!got) busy(true, ai.enabled ? t('writing') : t('template_mode'));
          got += data; ta.value += data; flush();
        }
        if (ev === 'error') {
          const code = data && data.code;
          const key = 'ai_errors_' + code;
          msg.append(banner('danger', 'alert', t(key) !== key ? t(key) : t('ai_errors_generic')));
          if (!got && (action === 'improve' || action === 'translate') && undo != null) { ta.value = undo; undo = null; undoBtn.hidden = true; }
        }
      }, controller.signal);
    } catch (e) {
      if (e.name !== 'AbortError') msg.append(banner('danger', 'alert', t('ai_errors_generic')));
    } finally {
      controller = null;
      paper.classList.remove('typing');
      busy(false);
      flush();
      persist();
    }
  }

  async function exportDocx() {
    if (!doc || !ta.value.trim()) return;
    const name = (title.value.trim() || t('untitled'));
    const r = await download('/api/export/docx', { title: name, markdown: ta.value });
    if (r.ok) saveBlob(r.blob, name.replace(/[\\/:*?"<>|]/g, '_') + '.docx');
    else msg.replaceChildren(banner('danger', 'alert', t('ai_errors_generic')));
  }

  function exportMd() {
    if (!doc) return;
    const name = (title.value.trim() || t('untitled')).replace(/[\\/:*?"<>|]/g, '_');
    saveBlob(new Blob([ta.value], { type: 'text/markdown;charset=utf-8' }), name + '.md');
  }

  function printDoc() {
    const root = document.getElementById('print-root');
    renderMarkdown(ta.value, root);
    document.body.classList.add('printing');
    const done = () => { document.body.classList.remove('printing'); clear(root); window.removeEventListener('afterprint', done); };
    window.addEventListener('afterprint', done);
    window.print();
  }

  const first = all().sort((a, b) => b.updated - a.updated)[0];
  openDoc(first || newDoc());
  return () => { if (controller) controller.abort(); };
}
