import { h, btn, empty, uid, clear } from '../ui.js';
import { t } from '../i18n.js';
import * as store from '../store.js';

/** Credit-weighted and simple average over graded subjects (Bulgarian 2–6 scale). */
export function average(list) {
  const graded = list.filter((g) => Number(g.grade) >= 2 && Number(g.grade) <= 6);
  const credits = graded.reduce((a, g) => a + (Number(g.credits) || 0), 0);
  const weighted = credits ? graded.reduce((a, g) => a + Number(g.grade) * (Number(g.credits) || 0), 0) / credits : 0;
  const simple = graded.length ? graded.reduce((a, g) => a + Number(g.grade), 0) / graded.length : 0;
  return { count: graded.length, credits, weighted: credits ? weighted : simple, simple };
}

export function gradeWord(v) {
  if (!v) return t('not_graded');
  if (v >= 5.5) return t('gr_6');
  if (v >= 4.5) return t('gr_5');
  if (v >= 3.5) return t('gr_4');
  if (v >= 3) return t('gr_3');
  return t('gr_2');
}

export default function grades(view) {
  const all = () => store.load('grades', []);
  const put = (list) => { store.save('grades', list); summary(); };
  const tableBody = h('tbody');
  const sumBox = h('div', { class: 'cards three', style: { marginBottom: '1rem' } });

  function summary() {
    const a = average(all());
    clear(sumBox);
    const stat = (label, value, sub) => h('div', { class: 'card' }, h('small', { class: 'muted' }, label), h('div', { class: 'stat' }, value), sub ? h('small', { class: 'muted' }, sub) : null);
    sumBox.append(
      stat(t('weighted_avg'), a.count ? a.weighted.toFixed(2) : '—', a.count ? gradeWord(a.weighted) : ''),
      stat(t('simple_avg'), a.count ? a.simple.toFixed(2) : '—'),
      stat(t('total_credits'), String(a.credits), `${a.count} / ${all().length}`));
  }

  function row(g) {
    const update = (patch) => { const list = all(); const i = list.findIndex((x) => x.id === g.id); if (i < 0) return; Object.assign(list[i], patch); Object.assign(g, patch); put(list); };
    const gradeSel = h('select', { 'aria-label': t('grade'), onchange: () => update({ grade: gradeSel.value ? Number(gradeSel.value) : null }) },
      h('option', { value: '' }, '—'), [6, 5, 4, 3, 2].map((v) => h('option', { value: String(v) }, `${v} · ${t('gr_' + v)}`)));
    gradeSel.value = g.grade ? String(g.grade) : '';
    return h('tr', null,
      h('td', null, h('input', { value: g.subject, 'aria-label': t('subject'), maxlength: '120', onchange: (e) => update({ subject: e.target.value }) })),
      h('td', null, h('input', { type: 'number', min: '0', max: '30', step: '0.5', value: g.credits ?? '', 'aria-label': t('credits'), style: { width: '5.5rem' }, onchange: (e) => update({ credits: Number(e.target.value) || 0 }) })),
      h('td', null, gradeSel),
      h('td', null, h('input', { value: g.semester || '', 'aria-label': t('semester'), maxlength: '20', style: { width: '6rem' }, onchange: (e) => update({ semester: e.target.value }) })),
      h('td', null, btn(t('delete'), { icon: 'trash', iconOnly: true, class: 'ghost icon small', onclick: () => { put(all().filter((x) => x.id !== g.id)); draw(); } })));
  }

  function draw() {
    clear(tableBody);
    const list = all();
    if (!list.length) { tableBody.append(h('tr', null, h('td', { colspan: '5' }, empty('grades', t('no_grades'))))); return; }
    list.forEach((g) => tableBody.append(row(g)));
  }

  const add = () => { put([...all(), { id: uid(), subject: '', credits: 5, grade: null, semester: '' }]); draw(); const inputs = tableBody.querySelectorAll('tr:last-child input'); if (inputs[0]) inputs[0].focus(); };

  view.append(sumBox,
    h('section', { class: 'card' },
      h('header', null, h('h2', null, t('nav_grades')), btn(t('add_subject'), { class: 'primary', icon: 'plus', onclick: add })),
      h('div', { class: 'scroll-x' }, h('table', { class: 'tbl' },
        h('thead', null, h('tr', null, h('th', null, t('subject')), h('th', null, t('credits')), h('th', null, t('grade')), h('th', null, t('semester')), h('th'))),
        tableBody)),
      h('p', { class: 'faint', style: { marginTop: '.8rem' } }, h('small', null, t('grades_hint')))));
  summary();
  draw();
}
