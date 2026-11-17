//! Markdown → Word (.docx), built by hand so no office software is needed on the server.
//! Layout follows common Bulgarian university rules: A4, Times New Roman 12 pt,
//! 1.5 line spacing, justified text. A line with only `---` starts a new page.

use crate::AppState;
use crate::util::xml_escape;
use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::json;
use std::io::Write;
use std::sync::Arc;

fn run(text: &str, bold: bool, italic: bool, code: bool) -> String {
    let mut rpr = String::new();
    if bold {
        rpr.push_str("<w:b/>");
    }
    if italic {
        rpr.push_str("<w:i/>");
    }
    if code {
        rpr.push_str("<w:rFonts w:ascii=\"Consolas\" w:hAnsi=\"Consolas\" w:cs=\"Consolas\"/><w:sz w:val=\"20\"/>");
    }
    let rpr = if rpr.is_empty() {
        String::new()
    } else {
        format!("<w:rPr>{rpr}</w:rPr>")
    };
    format!(
        "<w:r>{rpr}<w:t xml:space=\"preserve\">{}</w:t></w:r>",
        xml_escape(text)
    )
}

/// **bold**, *italic*, `code` inside a line.
pub fn inline(s: &str) -> String {
    let mut out = String::new();
    let (mut bold, mut italic, mut code) = (false, false, false);
    let mut buf = String::new();
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '`' {
            out.push_str(&run(&buf, bold, italic, code));
            buf.clear();
            code = !code;
            i += 1;
            continue;
        }
        if !code && c == '*' && i + 1 < chars.len() && chars[i + 1] == '*' {
            out.push_str(&run(&buf, bold, italic, code));
            buf.clear();
            bold = !bold;
            i += 2;
            continue;
        }
        if !code && (c == '*' || c == '_') && is_emphasis_mark(&chars, i) {
            out.push_str(&run(&buf, bold, italic, code));
            buf.clear();
            italic = !italic;
            i += 1;
            continue;
        }
        buf.push(c);
        i += 1;
    }
    if !buf.is_empty() {
        out.push_str(&run(&buf, bold, italic, code));
    }
    out
}

/// A single * or _ toggles italics only next to a word character (so "2 * 3" stays text).
fn is_emphasis_mark(c: &[char], i: usize) -> bool {
    let before = if i > 0 { c[i - 1] } else { ' ' };
    let after = c.get(i + 1).copied().unwrap_or(' ');
    let word = |ch: char| ch.is_alphanumeric();
    (word(after) && !word(before)) || (word(before) && !word(after))
}

fn para(style: Option<&str>, body: &str, extra_ppr: &str) -> String {
    let st = style
        .map(|s| format!("<w:pStyle w:val=\"{s}\"/>"))
        .unwrap_or_default();
    format!("<w:p><w:pPr>{st}{extra_ppr}</w:pPr>{body}</w:p>")
}

fn table(rows: &[Vec<String>]) -> String {
    let cols = rows.iter().map(|r| r.len()).max().unwrap_or(1).max(1);
    let width = 9000 / cols;
    let mut x = String::from(
        "<w:tbl><w:tblPr><w:tblStyle w:val=\"TableGrid\"/><w:tblW w:w=\"0\" w:type=\"auto\"/>\
<w:tblBorders><w:top w:val=\"single\" w:sz=\"4\"/><w:left w:val=\"single\" w:sz=\"4\"/><w:bottom w:val=\"single\" w:sz=\"4\"/>\
<w:right w:val=\"single\" w:sz=\"4\"/><w:insideH w:val=\"single\" w:sz=\"4\"/><w:insideV w:val=\"single\" w:sz=\"4\"/></w:tblBorders></w:tblPr><w:tblGrid>",
    );
    for _ in 0..cols {
        x.push_str(&format!("<w:gridCol w:w=\"{width}\"/>"));
    }
    x.push_str("</w:tblGrid>");
    for (ri, r) in rows.iter().enumerate() {
        x.push_str("<w:tr>");
        for ci in 0..cols {
            let cell = r.get(ci).map(String::as_str).unwrap_or("");
            let body = if ri == 0 {
                run(cell.trim_matches('*'), true, false, false)
            } else {
                inline(cell)
            };
            x.push_str(&format!(
                "<w:tc><w:tcPr><w:tcW w:w=\"{width}\" w:type=\"dxa\"/></w:tcPr><w:p><w:pPr><w:spacing w:line=\"240\" w:after=\"0\"/><w:jc w:val=\"left\"/></w:pPr>{body}</w:p></w:tc>"
            ));
        }
        x.push_str("</w:tr>");
    }
    x.push_str("</w:tbl>");
    x
}

fn split_row(line: &str) -> Vec<String> {
    let t = line.trim().trim_start_matches('|').trim_end_matches('|');
    t.split('|').map(|c| c.trim().to_string()).collect()
}

fn is_separator_row(line: &str) -> bool {
    let t = line.trim();
    t.starts_with('|') && t.chars().all(|c| matches!(c, '|' | '-' | ':' | ' '))
}

pub fn document_xml(md: &str) -> String {
    let mut body = String::new();
    let lines: Vec<&str> = md.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        let raw = lines[i];
        let l = raw.trim_end();
        let t = l.trim_start();
        if t.is_empty() {
            i += 1;
            continue;
        }
        if t.starts_with('|') {
            let mut rows = Vec::new();
            while i < lines.len() && lines[i].trim_start().starts_with('|') {
                if !is_separator_row(lines[i]) {
                    rows.push(split_row(lines[i]));
                }
                i += 1;
            }
            body.push_str(&table(&rows));
            body.push_str(&para(None, "", ""));
            continue;
        }
        if t == "---" || t == "***" {
            body.push_str("<w:p><w:r><w:br w:type=\"page\"/></w:r></w:p>");
        } else if let Some(h) = t.strip_prefix("### ") {
            body.push_str(&para(Some("Heading3"), &inline(h), ""));
        } else if let Some(h) = t.strip_prefix("## ") {
            body.push_str(&para(Some("Heading2"), &inline(h), ""));
        } else if let Some(h) = t.strip_prefix("# ") {
            body.push_str(&para(Some("Heading1"), &inline(h), ""));
        } else if let Some(q) = t.strip_prefix("> ").or_else(|| t.strip_prefix('>')) {
            body.push_str(&para(Some("Quote"), &inline(q), ""));
        } else if let Some(b) = t
            .strip_prefix("- ")
            .or_else(|| t.strip_prefix("* "))
            .or_else(|| t.strip_prefix("• "))
        {
            let depth = (l.len() - t.len()) / 2;
            let ind = 360 + depth * 360;
            body.push_str(&para(
                Some("ListParagraph"),
                &format!("{}{}", run("• ", false, false, false), inline(b)),
                &format!("<w:ind w:left=\"{ind}\" w:hanging=\"240\"/>"),
            ));
        } else if let Some((num, rest)) = numbered(t) {
            body.push_str(&para(
                Some("ListParagraph"),
                &format!(
                    "{}{}",
                    run(&format!("{num}. "), false, false, false),
                    inline(rest)
                ),
                "<w:ind w:left=\"360\" w:hanging=\"300\"/>",
            ));
        } else {
            body.push_str(&para(None, &inline(t), ""));
        }
        i += 1;
    }
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
<w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><w:body>{body}\
<w:sectPr><w:pgSz w:w=\"11906\" w:h=\"16838\"/><w:pgMar w:top=\"1418\" w:right=\"1134\" w:bottom=\"1418\" w:left=\"1701\" w:header=\"708\" w:footer=\"708\" w:gutter=\"0\"/></w:sectPr>\
</w:body></w:document>"
    )
}

fn numbered(t: &str) -> Option<(&str, &str)> {
    let dot = t.find(". ")?;
    let n = &t[..dot];
    (!n.is_empty() && n.len() <= 3 && n.bytes().all(|b| b.is_ascii_digit()))
        .then(|| (n, &t[dot + 2..]))
}

const STYLES: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
<w:docDefaults><w:rPrDefault><w:rPr><w:rFonts w:ascii="Times New Roman" w:hAnsi="Times New Roman" w:cs="Times New Roman" w:eastAsia="Times New Roman"/><w:sz w:val="24"/><w:szCs w:val="24"/><w:lang w:val="bg-BG"/></w:rPr></w:rPrDefault>
<w:pPrDefault><w:pPr><w:spacing w:after="120" w:line="360" w:lineRule="auto"/><w:jc w:val="both"/></w:pPr></w:pPrDefault></w:docDefaults>
<w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/><w:qFormat/></w:style>
<w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:qFormat/><w:pPr><w:keepNext/><w:spacing w:before="240" w:after="240"/><w:jc w:val="center"/><w:outlineLvl w:val="0"/></w:pPr><w:rPr><w:b/><w:sz w:val="32"/><w:szCs w:val="32"/></w:rPr></w:style>
<w:style w:type="paragraph" w:styleId="Heading2"><w:name w:val="heading 2"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:qFormat/><w:pPr><w:keepNext/><w:spacing w:before="240" w:after="120"/><w:jc w:val="left"/><w:outlineLvl w:val="1"/></w:pPr><w:rPr><w:b/><w:sz w:val="28"/><w:szCs w:val="28"/></w:rPr></w:style>
<w:style w:type="paragraph" w:styleId="Heading3"><w:name w:val="heading 3"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:qFormat/><w:pPr><w:keepNext/><w:spacing w:before="180" w:after="60"/><w:jc w:val="left"/><w:outlineLvl w:val="2"/></w:pPr><w:rPr><w:b/><w:i/></w:rPr></w:style>
<w:style w:type="paragraph" w:styleId="ListParagraph"><w:name w:val="List Paragraph"/><w:basedOn w:val="Normal"/><w:pPr><w:spacing w:after="60"/><w:jc w:val="left"/></w:pPr></w:style>
<w:style w:type="paragraph" w:styleId="Quote"><w:name w:val="Quote"/><w:basedOn w:val="Normal"/><w:pPr><w:ind w:left="567"/></w:pPr><w:rPr><w:i/><w:color w:val="555555"/></w:rPr></w:style>
<w:style w:type="table" w:styleId="TableGrid"><w:name w:val="Table Grid"/><w:tblPr><w:tblCellMar><w:left w:w="108" w:type="dxa"/><w:right w:w="108" w:type="dxa"/></w:tblCellMar></w:tblPr></w:style>
</w:styles>"#;

pub fn build(title: &str, md: &str) -> Result<Vec<u8>, String> {
    use zip::write::SimpleFileOptions;
    let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let mut z = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let core = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<cp:coreProperties xmlns:cp=\"http://schemas.openxmlformats.org/package/2006/metadata/core-properties\" \
xmlns:dc=\"http://purl.org/dc/elements/1.1/\"><dc:title>{}</dc:title><dc:creator>TU-Varna Student Hub</dc:creator></cp:coreProperties>",
        xml_escape(title)
    );
    let files: [(&str, String); 6] = [
        (
            "[Content_Types].xml",
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">\
<Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"xml\" ContentType=\"application/xml\"/>\
<Override PartName=\"/word/document.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml\"/>\
<Override PartName=\"/word/styles.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml\"/>\
<Override PartName=\"/docProps/core.xml\" ContentType=\"application/vnd.openxmlformats-package.core-properties+xml\"/></Types>"
                .into(),
        ),
        (
            "_rels/.rels",
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
<Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"word/document.xml\"/>\
<Relationship Id=\"rId2\" Type=\"http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties\" Target=\"docProps/core.xml\"/></Relationships>"
                .into(),
        ),
        (
            "word/_rels/document.xml.rels",
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
<Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles\" Target=\"styles.xml\"/></Relationships>"
                .into(),
        ),
        ("word/document.xml", document_xml(md)),
        ("word/styles.xml", STYLES.into()),
        ("docProps/core.xml", core),
    ];
    for (name, content) in files {
        z.start_file(name, opts).map_err(|e| e.to_string())?;
        z.write_all(content.as_bytes()).map_err(|e| e.to_string())?;
    }
    Ok(z.finish().map_err(|e| e.to_string())?.into_inner())
}

#[derive(Deserialize)]
pub struct ExportReq {
    pub title: String,
    pub markdown: String,
}

/// ASCII-only, safe file name; the full name goes in filename* (RFC 5987).
fn filenames(title: &str) -> (String, String) {
    let base: String = title
        .chars()
        .filter(|c| {
            !matches!(
                c,
                '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '\r' | '\n'
            )
        })
        .take(80)
        .collect();
    let base = if base.trim().is_empty() {
        "document".to_string()
    } else {
        base.trim().to_string()
    };
    let ascii: String = base
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let encoded: String = url::form_urlencoded::byte_serialize(format!("{base}.docx").as_bytes())
        .collect::<String>()
        .replace('+', "%20");
    (format!("{ascii}.docx"), encoded)
}

pub async fn export(
    State(st): State<Arc<AppState>>,
    axum::Extension(ip): axum::Extension<crate::security::ClientIp>,
    headers: HeaderMap,
    Json(req): Json<ExportReq>,
) -> Response {
    if !st.limiter.allow(ip.0, crate::security::Bucket::Export) {
        return crate::security::too_many();
    }
    if let Err(r) = crate::require_user(&st, &headers) {
        return r;
    }
    if req.markdown.len() > 400_000 || req.title.chars().count() > 200 {
        return (
            StatusCode::PAYLOAD_TOO_LARGE,
            Json(json!({"error": "too_long"})),
        )
            .into_response();
    }
    match build(&req.title, &req.markdown) {
        Ok(bytes) => {
            let (ascii, encoded) = filenames(&req.title);
            let mut res = bytes.into_response();
            let h = res.headers_mut();
            h.insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static(
                    "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
                ),
            );
            if let Ok(v) = HeaderValue::from_str(&format!(
                "attachment; filename=\"{ascii}\"; filename*=UTF-8''{encoded}"
            )) {
                h.insert(header::CONTENT_DISPOSITION, v);
            }
            h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
            res
        }
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "export_failed"})),
        )
            .into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_maps_to_word_parts() {
        let x = document_xml(
            "# Заглавие\n\nТекст с **удебелен** и *курсив* и `код`.\n\n- точка\n1. първо\n\n---\n\n| A | B |\n|---|---|\n| 1 | <2> |\n",
        );
        assert!(x.contains("w:val=\"Heading1\""));
        assert!(x.contains("<w:b/>") && x.contains("<w:i/>") && x.contains("Consolas"));
        assert!(x.contains("• ") && x.contains("1. "));
        assert!(x.contains("w:type=\"page\""));
        assert!(
            x.contains("<w:tbl>") && x.contains("&lt;2&gt;"),
            "table cells are escaped"
        );
        assert!(!x.contains("|---|"));
    }

    #[test]
    fn math_star_is_not_italic() {
        assert!(!inline("2 * 3 = 6").contains("<w:i/>"));
    }

    #[test]
    fn docx_is_a_valid_zip_with_the_main_parts() {
        let bytes = build("Тест", "# Здравей\n\nсвят").unwrap();
        assert_eq!(&bytes[..2], b"PK");
        let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        for name in [
            "[Content_Types].xml",
            "_rels/.rels",
            "word/document.xml",
            "word/styles.xml",
            "word/_rels/document.xml.rels",
        ] {
            assert!(z.by_name(name).is_ok(), "missing {name}");
        }
    }

    #[test]
    fn filename_is_safe() {
        let (a, e) = filenames("Курсова \"работа\"/1\r\n");
        assert!(a.ends_with(".docx") && !a.contains('"') && !a.contains('/'));
        assert!(e.starts_with("%D0%9A"));
    }
}
