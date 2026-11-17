// A small, safe Markdown renderer that builds DOM nodes directly (never innerHTML).
// Supports: # ## ### headings, paragraphs, - / 1. lists, > quotes, --- rules,
// | tables |, **bold**, *italic*, `code`, and plain http(s) links.

function inline(text) {
  const frag = document.createDocumentFragment();
  // Tokenize: `code`, **bold**, *italic*, links.
  const re = /(`[^`]+`|\*\*[^*]+\*\*|(?<![\w*])\*[^*\s][^*]*\*(?![\w*])|https?:\/\/[^\s<>()]+[^\s<>().,;:!?])/g;
  let last = 0;
  for (const m of text.matchAll(re)) {
    if (m.index > last) frag.append(text.slice(last, m.index));
    const tok = m[0];
    let el;
    if (tok.startsWith('`')) { el = document.createElement('code'); el.textContent = tok.slice(1, -1); }
    else if (tok.startsWith('**')) { el = document.createElement('strong'); el.append(inline(tok.slice(2, -2))); }
    else if (tok.startsWith('*')) { el = document.createElement('em'); el.append(inline(tok.slice(1, -1))); }
    else {
      el = document.createElement('a');
      el.href = tok; el.textContent = tok; el.target = '_blank'; el.rel = 'noopener noreferrer nofollow';
    }
    frag.append(el);
    last = m.index + tok.length;
  }
  if (last < text.length) frag.append(text.slice(last));
  return frag;
}

function cells(line) {
  return line.trim().replace(/^\|/, '').replace(/\|$/, '').split('|').map((c) => c.trim());
}

export function renderMarkdown(md, target) {
  const root = target || document.createElement('div');
  while (root.firstChild) root.firstChild.remove();
  const lines = (md || '').replace(/\r\n/g, '\n').split('\n');
  let i = 0;
  let para = [];
  const flush = () => {
    if (para.length) { const p = document.createElement('p'); p.append(inline(para.join(' '))); root.append(p); para = []; }
  };
  while (i < lines.length) {
    const raw = lines[i];
    const l = raw.trim();
    if (!l) { flush(); i++; continue; }
    let m;
    if ((m = /^(#{1,3})\s+(.*)$/.exec(l))) {
      flush();
      const el = document.createElement('h' + m[1].length); el.append(inline(m[2])); root.append(el); i++; continue;
    }
    if (/^(-{3,}|\*{3,})$/.test(l)) { flush(); root.append(document.createElement('hr')); i++; continue; }
    if (l.startsWith('>')) {
      flush();
      const q = document.createElement('blockquote');
      const buf = [];
      while (i < lines.length && lines[i].trim().startsWith('>')) { buf.push(lines[i].trim().replace(/^>\s?/, '')); i++; }
      q.append(inline(buf.join(' '))); root.append(q); continue;
    }
    if (l.startsWith('|')) {
      flush();
      const rows = [];
      while (i < lines.length && lines[i].trim().startsWith('|')) {
        const r = lines[i].trim();
        if (!/^\|[\s|:-]+\|?$/.test(r)) rows.push(cells(r));
        i++;
      }
      const table = document.createElement('table');
      rows.forEach((r, ri) => {
        const tr = document.createElement('tr');
        r.forEach((c) => { const td = document.createElement(ri === 0 ? 'th' : 'td'); td.append(inline(c)); tr.append(td); });
        table.append(tr);
      });
      root.append(table); continue;
    }
    if (/^([-*•])\s+/.test(l) || /^\d{1,3}[.)]\s+/.test(l)) {
      flush();
      const ordered = /^\d/.test(l);
      const list = document.createElement(ordered ? 'ol' : 'ul');
      while (i < lines.length) {
        const s = lines[i].trim();
        const mm = ordered ? /^\d{1,3}[.)]\s+(.*)$/.exec(s) : /^[-*•]\s+(.*)$/.exec(s);
        if (!mm) break;
        const li = document.createElement('li'); li.append(inline(mm[1])); list.append(li); i++;
      }
      root.append(list); continue;
    }
    para.push(l); i++;
  }
  flush();
  return root;
}

export function wordCount(md) {
  const m = (md || '').match(/[\p{L}\p{N}]+/gu);
  return m ? m.length : 0;
}
