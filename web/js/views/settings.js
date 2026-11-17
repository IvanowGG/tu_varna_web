import { h, btn, seg, field, banner, confirmBox, clear } from '../ui.js';
import { t, lang } from '../i18n.js';
import * as store from '../store.js';
import * as tt from '../timetable.js';
import { saveBlob } from '../api.js';
import { setTheme, setLanguage, signOut } from '../main.js';

export default function settings(view, ctx) {
  const s = store.settings();
  const msg = h('div');
  const start = h('input', { type: 'date', value: s.semesterStart, onchange: () => { if (start.value) store.setSetting('semesterStart', start.value); weekInfo(); } });
  const info = h('small', { class: 'muted' });
  const weekInfo = () => {
    const w = store.weekNumber(new Date());
    const p = tt.parityOf(w);
    info.textContent = p ? `${t('today')}: ${t('week_n', w)} (${t(p === 'odd' ? 'odd_week' : 'even_week')})` : t('before_semester');
  };
  weekInfo();
  const groups = tt.subgroups(tt.saved());
  const file = h('input', { type: 'file', accept: 'application/json,.json', hidden: true, onchange: async () => {
    clear(msg);
    try {
      const f = file.files[0];
      if (!f || f.size > 20_000_000) throw new Error('size');
      store.importAll(JSON.parse(await f.text()));
      msg.append(banner('info', 'check', t('imported')));
    } catch { msg.append(banner('danger', 'alert', t('import_failed'))); }
    file.value = '';
  } });

  view.append(h('div', { class: 'cards' },
    h('section', { class: 'card' }, h('header', null, h('h2', null, t('appearance'))),
      field(t('theme'), seg([['auto', t('theme_auto')], ['light', t('theme_light')], ['dark', t('theme_dark')]], store.prefs().theme, (v) => setTheme(v))),
      field(t('language'), seg([['bg', 'Български'], ['en', 'English']], lang(), (v) => setLanguage(v)))),
    h('section', { class: 'card' }, h('header', null, h('h2', null, t('nav_schedule'))),
      field(t('semester_start'), start, t('semester_start_hint')), info,
      groups.length ? field(t('my_subgroup'), seg([['', t('all')], ...groups.map((g) => [g, g])], s.subgroup, (v) => store.setSetting('subgroup', v))) : null),
    h('section', { class: 'card' }, h('header', null, h('h2', null, t('data'))),
      h('p', { class: 'muted' }, t('data_hint')),
      msg,
      h('div', { class: 'row' },
        btn(t('export_data'), { icon: 'download', onclick: () => saveBlob(new Blob([JSON.stringify(store.exportAll(), null, 2)], { type: 'application/json' }), `tuv-hub-${new Date().toISOString().slice(0, 10)}.json`) }),
        btn(t('import_data'), { icon: 'upload', onclick: () => file.click() }), file),
      h('div', { class: 'row', style: { marginTop: '1rem' } },
        btn(t('clear_data'), { icon: 'trash', class: 'danger', onclick: async () => {
          if (await confirmBox(t('clear_data'))) { store.clearAll(); clear(msg); msg.append(banner('info', 'check', t('cleared'))); }
        } }))),
    h('section', { class: 'card' }, h('header', null, h('h2', null, t('privacy'))),
      h('p', { class: 'muted' }, t('privacy_text')),
      btn(t('sign_out'), { icon: 'logout', onclick: () => signOut() }))));
  void ctx;
}
