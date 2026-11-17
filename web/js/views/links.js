import { h, btn, field, empty, uid, clear } from '../ui.js';
import { t } from '../i18n.js';
import { icon } from '../icons.js';
import * as store from '../store.js';

// Checked 2026-11-12: these hosts answer (see .fway checks).
const UNI = [
  ['l_site', 'https://www1.tu-varna.bg/tu-varna/', 'globe'],
  ['l_umo', 'http://umo.tu-varna.bg/', 'calendar'],
  ['l_elearning', 'https://elearning.tu-varna.bg/', 'graduation'],
  ['l_sis', 'http://www2.tu-varna.bg/student/', 'user'],
  ['l_mail', 'https://mail.tu-varna.bg/', 'mail'],
  ['l_teams', 'https://teams.microsoft.com/', 'users'],
  ['l_office', 'https://www.office.com/', 'file'],
];

function safeUrl(u) {
  try { const x = new URL(u); return x.protocol === 'https:' || x.protocol === 'http:' ? x.href : null; } catch { return null; }
}

export default function links(view) {
  const box = h('div', { class: 'list' });
  const all = () => store.load('links', []);
  const name = h('input', { maxlength: '80', placeholder: t('name') });
  const url = h('input', { type: 'url', placeholder: 'https://', maxlength: '500' });

  const card = (label, href, ic, extra) => h('div', { class: 'item' }, icon(ic),
    h('a', { class: 'body', href, target: '_blank', rel: 'noopener noreferrer' }, h('b', null, label), h('small', { class: 'muted' }, new URL(href).host)),
    extra || icon('external'));

  function draw() {
    clear(box);
    const list = all();
    if (!list.length) { box.append(empty('link', t('no_links'))); return; }
    for (const l of list) {
      const href = safeUrl(l.url);
      if (!href) continue;
      box.append(card(l.name || href, href, 'link', btn(t('delete'), { icon: 'trash', iconOnly: true, class: 'ghost icon small', onclick: () => { store.save('links', all().filter((x) => x.id !== l.id)); draw(); } })));
    }
  }

  view.append(h('div', { class: 'cards' },
    h('section', { class: 'card' }, h('header', null, h('h2', null, t('uni_links'))),
      h('div', { class: 'list' }, UNI.map(([k, u, ic]) => card(t(k), u, ic)))),
    h('section', { class: 'card' }, h('header', null, h('h2', null, t('my_links'))),
      h('form', { class: 'row', style: { marginBottom: '.8rem' }, onsubmit: (e) => {
        e.preventDefault();
        const href = safeUrl(url.value.trim());
        if (!href) { url.focus(); return; }
        store.save('links', [...all(), { id: uid(), name: name.value.trim(), url: href }]);
        name.value = ''; url.value = ''; draw();
      } }, h('div', { class: 'grow' }, field(t('name'), name)), h('div', { class: 'grow' }, field(t('url'), url)), btn(t('add'), { type: 'submit', class: 'primary', icon: 'plus' })),
      box)));
  draw();
}
