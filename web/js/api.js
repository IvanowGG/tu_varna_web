// Calls to our own server. Same-origin only; the browser adds the Origin header that the
// server checks on every POST.

export async function api(path, { method = 'GET', body, signal } = {}) {
  let res;
  try {
    res = await fetch(path, {
      method,
      credentials: 'same-origin',
      headers: body !== undefined ? { 'content-type': 'application/json' } : {},
      body: body !== undefined ? JSON.stringify(body) : undefined,
      signal,
    });
  } catch (e) {
    if (e.name === 'AbortError') throw e;
    return { ok: false, status: 0, data: { error: 'network' } };
  }
  let data = null;
  const ct = res.headers.get('content-type') || '';
  if (ct.includes('application/json')) {
    try { data = await res.json(); } catch { data = null; }
  }
  return { ok: res.ok, status: res.status, data: data || {}, res };
}

/** POST that answers with a file; returns a Blob or an error code. */
export async function download(path, body) {
  const r = await fetch(path, { method: 'POST', credentials: 'same-origin', headers: { 'content-type': 'application/json' }, body: JSON.stringify(body) });
  if (!r.ok) return { ok: false, status: r.status };
  return { ok: true, blob: await r.blob() };
}

export function saveBlob(blob, filename) {
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url; a.download = filename; a.rel = 'noopener';
  document.body.append(a); a.click(); a.remove();
  setTimeout(() => URL.revokeObjectURL(url), 2000);
}

/**
 * Read our Server-Sent Events stream from a POST.
 * onEvent(name, data) is called for each event. Returns when the stream ends.
 */
export async function stream(path, body, onEvent, signal) {
  const res = await fetch(path, {
    method: 'POST', credentials: 'same-origin', signal,
    headers: { 'content-type': 'application/json', accept: 'text/event-stream' },
    body: JSON.stringify(body),
  });
  if (!res.ok || !(res.headers.get('content-type') || '').startsWith('text/event-stream')) {
    let data = {};
    try { data = await res.json(); } catch { /* not json */ }
    onEvent('error', { code: data.error || 'generic', status: res.status });
    return;
  }
  const reader = res.body.getReader();
  const dec = new TextDecoder();
  let buf = '';
  for (;;) {
    const { value, done } = await reader.read();
    if (done) break;
    buf += dec.decode(value, { stream: true });
    let i;
    while ((i = buf.indexOf('\n\n')) >= 0) {
      const chunk = buf.slice(0, i); buf = buf.slice(i + 2);
      let name = 'message', data = '';
      for (const line of chunk.split('\n')) {
        if (line.startsWith('event:')) name = line.slice(6).trim();
        else if (line.startsWith('data:')) data += line.slice(5).trim();
      }
      let parsed = data;
      try { parsed = JSON.parse(data); } catch { /* keep string */ }
      onEvent(name, parsed);
    }
  }
}
