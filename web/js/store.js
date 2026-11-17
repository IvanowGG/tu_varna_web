// Local storage, one namespace per signed-in account, so two students sharing a PC
// never see each other's data. No passwords, tokens or cookies are stored here.

const NS = 'tuv';
let owner = 'guest';
const listeners = new Set();

export function setOwner(key) { owner = key || 'guest'; }
const k = (name) => `${NS}:${owner}:${name}`;

export function load(name, fallback) {
  try {
    const raw = localStorage.getItem(k(name));
    return raw == null ? structuredClone(fallback) : JSON.parse(raw);
  } catch { return structuredClone(fallback); }
}

export function save(name, value) {
  try {
    localStorage.setItem(k(name), JSON.stringify(value));
    listeners.forEach((fn) => fn(name));
    return true;
  } catch (e) {
    console.warn('save failed', e);
    return false;
  }
}

export function onChange(fn) { listeners.add(fn); return () => listeners.delete(fn); }

const COLLECTIONS = ['tasks', 'notes', 'grades', 'docs', 'timetable', 'settings', 'focus', 'links'];

export function exportAll() {
  const out = { app: 'tuv-hub', version: 1, exported_at: new Date().toISOString(), data: {} };
  for (const c of COLLECTIONS) {
    const raw = localStorage.getItem(k(c));
    if (raw != null) out.data[c] = JSON.parse(raw);
  }
  return out;
}

export function importAll(obj) {
  if (!obj || obj.app !== 'tuv-hub' || typeof obj.data !== 'object') throw new Error('bad file');
  for (const c of COLLECTIONS) {
    if (c in obj.data) localStorage.setItem(k(c), JSON.stringify(obj.data[c]));
  }
  listeners.forEach((fn) => fn('*'));
}

export function clearAll() {
  for (const c of COLLECTIONS) localStorage.removeItem(k(c));
  listeners.forEach((fn) => fn('*'));
}

// ---------- Settings with defaults ----------

/** Monday on or after a date. */
function mondayOnOrAfter(y, m, d) {
  const x = new Date(y, m, d);
  while (x.getDay() !== 1) x.setDate(x.getDate() + 1);
  return x;
}

/** A reasonable first guess for week 1; the student can correct it in Settings. */
export function defaultSemesterStart(now = new Date()) {
  const y = now.getFullYear();
  const m = now.getMonth();
  let d;
  if (m >= 8) d = mondayOnOrAfter(y, 8, 15);          // Sep–Dec: winter of this year
  else if (m === 0) d = mondayOnOrAfter(y - 1, 8, 15); // January: still winter
  else d = mondayOnOrAfter(y, 1, 15);                 // Feb–Aug: summer
  const p = (n) => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`;
}

export function settings() {
  return { semesterStart: defaultSemesterStart(), subgroup: '', focusMin: 25, breakMin: 5, ...load('settings', {}) };
}
export function setSetting(key, value) { save('settings', { ...load('settings', {}), [key]: value }); }

/** Teaching week number (1-based) for a date, from the semester start. */
export function weekNumber(date = new Date(), startISO = settings().semesterStart) {
  const start = new Date(startISO + 'T00:00:00');
  const d = new Date(date.getFullYear(), date.getMonth(), date.getDate());
  const days = Math.floor((d - start) / 86400000);
  return Math.floor(days / 7) + 1;
}

// ---------- Preferences cookie (theme + language only, no secrets) ----------
export function prefs() {
  const m = document.cookie.match(/(?:^|;\s*)tuv_prefs=([a-z]+)\.([a-z]+)/);
  return { theme: m ? m[1] : 'auto', lang: m ? m[2] : 'bg' };
}
export function setPrefs(p) {
  const v = `${p.theme}.${p.lang}`;
  const secure = location.protocol === 'https:' ? '; Secure' : '';
  document.cookie = `tuv_prefs=${v}; Path=/; Max-Age=31536000; SameSite=Lax${secure}`;
}
