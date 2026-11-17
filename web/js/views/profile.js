import { h, btn, avatar } from '../ui.js';
import { t } from '../i18n.js';
import { icon } from '../icons.js';
import { signOut } from '../main.js';

export default function profile(view, { state }) {
  const u = state.me.user;
  const row = (label, value) => value ? h('tr', null, h('th', null, label), h('td', null, value)) : null;
  view.append(h('div', { class: 'cards' },
    h('section', { class: 'card span2' },
      h('div', { class: 'row' }, avatar(u, 'lg'),
        h('div', { class: 'grow' },
          h('h2', { style: { margin: 0 } }, u.name),
          h('div', { class: 'muted' }, u.email),
          h('div', { class: 'row', style: { marginTop: '.4rem' } },
            h('span', { class: 'pill ' + (u.source === 'demo' ? 'warn' : 'primary') }, u.source === 'demo' ? t('demo_account') : 'Microsoft'),
            u.job_title ? h('span', { class: 'pill' }, u.job_title) : null)),
        btn(t('sign_out'), { icon: 'logout', onclick: () => signOut() }))),
    h('section', { class: 'card' },
      h('header', null, h('h2', null, t('from_microsoft'))),
      h('table', { class: 'tbl' }, h('tbody', null,
        row(t('email'), u.email),
        row('UPN', u.upn !== u.email ? u.upn : ''),
        row(t('department'), u.department),
        row(t('job_title'), u.job_title),
        row(t('faculty_number'), u.employee_id),
        row(t('office'), u.office),
        row(t('organization'), u.company),
        row('Tenant', u.tenant_id))))));
}
