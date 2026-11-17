// The captcha dialog. Resolves true once the server has confirmed the answer.

import { h, modal, btn, clear } from './ui.js';
import { t } from './i18n.js';
import { api } from './api.js';
import { icon } from './icons.js';

export function runCaptcha() {
  return new Promise((resolve) => {
    let finished = false;
    let worker = null;
    const status = h('div', { class: 'cap-status', role: 'status' });
    const bar = h('i');
    const progress = h('div', { class: 'cap-progress', 'aria-hidden': 'true' }, bar);
    const img = h('img', { alt: t('cap_hint'), draggable: 'false' });
    const knob = h('div', { class: 'cap-knob', role: 'slider', tabindex: '0', 'aria-label': t('cap_hint'), 'aria-valuemin': '0', 'aria-valuemax': '100' });
    const stage = h('div', { class: 'cap-stage' }, img, knob);
    // Honeypot: invisible to people, tempting to form-filling bots.
    const hp = h('input', { class: 'hp', type: 'text', name: 'website', tabindex: '-1', autocomplete: 'off', 'aria-hidden': 'true' });
    const body = h('div', { class: 'captcha' },
      h('p', { class: 'cap-hint' }, t('cap_hint')),
      stage, progress, status, hp,
      h('div', { class: 'row between' },
        h('small', { class: 'faint' }, t('cap_keys')),
        btn(t('cap_new'), { icon: 'refresh', class: 'ghost small', onclick: () => load() })));

    const m = modal(t('cap_title'), body, {
      onclose: () => { if (worker) worker.terminate(); if (!finished) { finished = true; resolve(false); } },
    });

    let ch = null, nonce = null, knobX = 0, trace = [], t0 = 0, dragging = false, submitted = false;

    const setStatus = (kind, text, spin) => {
      clear(status);
      status.className = 'cap-status ' + (kind || '');
      if (spin) status.append(h('span', { class: 'spinner' }));
      else if (kind === 'ok') status.append(icon('check'));
      status.append(text);
    };
    const place = () => {
      const r = stage.getBoundingClientRect();
      const scale = r.width / (ch ? ch.width : 320);
      knob.style.left = (knobX * scale) + 'px';
      knob.style.top = ((ch ? ch.knob_y : 48) * scale) + 'px';
      knob.setAttribute('aria-valuenow', String(Math.round((knobX / (ch ? ch.width : 320)) * 100)));
    };

    async function load() {
      if (worker) worker.terminate();
      submitted = false; nonce = null; trace = []; bar.style.width = '0%';
      setStatus('', t('cap_working'), true);
      const r = await api('/api/captcha');
      if (!r.ok) { setStatus('bad', r.status === 429 ? t('ae_rate_limited') : t('ae_generic')); return; }
      ch = r.data;
      img.src = ch.image;
      knobX = ch.knob_start_x;
      place();
      worker = new Worker('/js/pow-worker.js');
      worker.onmessage = (e) => {
        if (e.data.progress != null) bar.style.width = (e.data.progress * 100).toFixed(0) + '%';
        if (e.data.nonce != null) {
          nonce = e.data.nonce; bar.style.width = '100%';
          worker.terminate(); worker = null;
          if (trace.length && !dragging) submit(); else setStatus('', t('cap_ready'));
        }
      };
      worker.postMessage({ salt: ch.salt, bits: ch.bits });
    }

    const toImgX = (clientX) => {
      const r = stage.getBoundingClientRect();
      return Math.max(12, Math.min(ch.width - 12, (clientX - r.left) * (ch.width / r.width)));
    };
    const record = (x, y) => trace.push([Math.round(performance.now() - t0), +x.toFixed(1), +y.toFixed(1)]);

    knob.addEventListener('pointerdown', (e) => {
      if (!ch || submitted) return;
      dragging = true; trace = []; t0 = performance.now();
      knob.setPointerCapture(e.pointerId);
      const r = stage.getBoundingClientRect();
      record(knobX, (e.clientY - r.top) * (ch.width / r.width));
      e.preventDefault();
    });
    knob.addEventListener('pointermove', (e) => {
      if (!dragging) return;
      const r = stage.getBoundingClientRect();
      knobX = toImgX(e.clientX);
      record(knobX, (e.clientY - r.top) * (ch.width / r.width));
      if (trace.length > 590) trace.splice(1, 1);
      place();
    });
    const release = () => {
      if (!dragging) return;
      dragging = false;
      if (nonce) submit(); else setStatus('', t('cap_working'), true);
    };
    knob.addEventListener('pointerup', release);
    knob.addEventListener('pointercancel', release);

    // Keyboard: arrows move the dot, Enter drops it.
    knob.addEventListener('keydown', (e) => {
      if (!ch || submitted) return;
      if (e.key === 'ArrowRight' || e.key === 'ArrowLeft') {
        if (!trace.length) t0 = performance.now();
        knobX = Math.max(12, Math.min(ch.width - 12, knobX + (e.key === 'ArrowRight' ? 1 : -1) * (e.shiftKey ? 12 : 4)));
        record(knobX, ch.knob_y);
        place(); e.preventDefault();
      } else if (e.key === 'Enter' || e.key === ' ') {
        e.preventDefault();
        if (trace.length) { if (nonce) submit(); else setStatus('', t('cap_working'), true); }
      }
    });

    async function submit() {
      if (submitted || !nonce) return;
      submitted = true;
      setStatus('', t('cap_checking'), true);
      const r = await api('/api/captcha/verify', { method: 'POST', body: { id: ch.id, nonce, x: knobX, trace, website: hp.value } });
      if (r.ok) {
        setStatus('ok', t('cap_ok'));
        finished = true;
        setTimeout(() => { m.close(); resolve(true); }, 350);
      } else {
        if (r.data && r.data.error === 'cross_site') { setStatus('bad', t('cap_cross_site')); return; }
        setStatus('bad', r.status === 429 ? t('ae_rate_limited') : t('cap_bad'));
        setTimeout(load, 900);
      }
    }

    window.addEventListener('resize', place);
    load();
  });
}
