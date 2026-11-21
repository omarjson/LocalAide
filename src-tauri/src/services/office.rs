//! Office Maker: real Word, Excel and PowerPoint files, written by hand.
//!
//! An Office file is a ZIP of small XML parts (OOXML). The builders here
//! keep each part as small as a valid file allows — no styles, no themes
//! beyond PowerPoint's required minimum — so the chat can make a document
//! from plain blocks the model already knows how to write.

use serde::Deserialize;

use crate::app::error::AppError;

/// What the model sends for one content block of a Word document.
#[derive(Debug, Deserialize)]
pub struct DocBlock {
    /// "heading", "paragraph", "bullet" or "table".
    pub kind: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub items: Vec<String>,
    #[serde(default)]
    pub rows: Vec<Vec<String>>,
    /// Heading level 1-3; larger numbers are smaller headings.
    #[serde(default)]
    pub level: u8,
}

/// One sheet of an Excel workbook: a name and a grid of cells.
#[derive(Debug, Deserialize)]
pub struct Sheet {
    #[serde(default = "default_sheet_name")]
    pub name: String,
    pub rows: Vec<Vec<String>>,
}

fn default_sheet_name() -> String {
    "Sheet".to_string()
}

/// One slide of a deck: a title and bullet lines.
#[derive(Debug, Deserialize)]
pub struct Slide {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub bullets: Vec<String>,
}

/// Escape the five XML characters for text content.
fn xml(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

// --- The ZIP container -------------------------------------------------

/// CRC-32 (IEEE), bitwise: slow per byte but document parts are tiny.
fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

struct ZipEntry {
    name: String,
    data: Vec<u8>,
}

/// A ZIP with every entry stored (no compression), which OOXML readers
/// accept and keeps the writer dependency-free. DOS timestamps are fixed,
/// so the same content builds the same bytes.
fn build_zip(entries: &[ZipEntry]) -> Vec<u8> {
    const DOS_TIME: u16 = 0; // 00:00:00
    const DOS_DATE: u16 = (2026 - 1980) << 9 | (1 << 5) | 1; // 2026-01-01
    let mut out = Vec::new();
    let mut central = Vec::new();
    for entry in entries {
        let crc = crc32(&entry.data);
        let offset = out.len() as u32;
        let name = entry.name.as_bytes();
        // Local file header: signature, versions, flags (0), method 0
        // (stored), time, date, crc, sizes, name length, name.
        out.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        out.extend_from_slice(&20u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&DOS_TIME.to_le_bytes());
        out.extend_from_slice(&DOS_DATE.to_le_bytes());
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&(entry.data.len() as u32).to_le_bytes());
        out.extend_from_slice(&(entry.data.len() as u32).to_le_bytes());
        out.extend_from_slice(&(name.len() as u16).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(name);
        out.extend_from_slice(&entry.data);
        // Central directory record for the same file.
        central.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
        central.extend_from_slice(&20u16.to_le_bytes());
        central.extend_from_slice(&20u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&DOS_TIME.to_le_bytes());
        central.extend_from_slice(&DOS_DATE.to_le_bytes());
        central.extend_from_slice(&crc.to_le_bytes());
        central.extend_from_slice(&(entry.data.len() as u32).to_le_bytes());
        central.extend_from_slice(&(entry.data.len() as u32).to_le_bytes());
        central.extend_from_slice(&(name.len() as u16).to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes()); // extra
        central.extend_from_slice(&0u16.to_le_bytes()); // comment
        central.extend_from_slice(&0u16.to_le_bytes()); // disk
        central.extend_from_slice(&0u16.to_le_bytes()); // internal attrs
        central.extend_from_slice(&0u32.to_le_bytes()); // external attrs
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(name);
    }
    let central_offset = out.len() as u32;
    let central_size = central.len() as u32;
    let count = entries.len() as u16;
    out.extend_from_slice(&central);
    // End of central directory.
    out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&central_size.to_le_bytes());
    out.extend_from_slice(&central_offset.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

fn content_types_docx() -> ZipEntry {
    ZipEntry {
        name: "[Content_Types].xml".to_string(),
        data: br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#.to_vec(),
    }
}

fn root_rels() -> ZipEntry {
    ZipEntry {
        name: "_rels/.rels".to_string(),
        data: br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#.to_vec(),
    }
}

// --- Word --------------------------------------------------------------

fn docx_paragraph(text: &str, bold: bool, half_points: u32, before: u32, after: u32) -> String {
    let mut rpr = String::new();
    if bold {
        rpr.push_str("<w:b/>");
    }
    if half_points > 0 {
        rpr.push_str(&format!("<w:sz w:val=\"{half_points}\"/>"));
    }
    let spacing = if before > 0 || after > 0 {
        format!(
            "<w:spacing w:before=\"{before}\" w:after=\"{after}\"/>"
        )
    } else {
        String::new()
    };
    format!(
        "<w:p><w:pPr>{spacing}</w:pPr><w:r><w:rPr>{rpr}</w:rPr><w:t xml:space=\"preserve\">{}</w:t></w:r></w:p>",
        xml(text)
    )
}

fn docx_table(rows: &[Vec<String>]) -> String {
    let cols = rows.iter().map(|r| r.len()).max().unwrap_or(1).max(1);
    let mut table = String::from(
        "<w:tbl><w:tblPr><w:tblBorders><w:top w:val=\"single\" w:sz=\"4\" w:color=\"999999\"/><w:left w:val=\"single\" w:sz=\"4\" w:color=\"999999\"/><w:bottom w:val=\"single\" w:sz=\"4\" w:color=\"999999\"/><w:right w:val=\"single\" w:sz=\"4\" w:color=\"999999\"/><w:insideH w:val=\"single\" w:sz=\"4\" w:color=\"999999\"/><w:insideV w:val=\"single\" w:sz=\"4\" w:color=\"999999\"/></w:tblBorders></w:tblPr><w:tblGrid>",
    );
    for _ in 0..cols {
        table.push_str("<w:gridCol w:w=\"2400\"/>");
    }
    table.push_str("</w:tblGrid>");
    for (index, row) in rows.iter().enumerate() {
        table.push_str("<w:tr>");
        for cell in row.iter().take(cols) {
            let bold = index == 0;
            table.push_str(&format!(
                "<w:tc><w:tcPr/><w:p><w:r><w:rPr>{}<w:sz w:val=\"22\"/></w:rPr><w:t xml:space=\"preserve\">{}</w:t></w:r></w:p></w:tc>",
                if bold { "<w:b/>" } else { "" },
                xml(cell)
            ));
        }
        table.push_str("</w:tr>");
    }
    table.push_str("</w:tbl>");
    table
}

/// Builds a .docx from the blocks. Returns the bytes to write.
pub fn make_docx(title: &str, blocks: &[DocBlock]) -> Result<Vec<u8>, AppError> {
    let mut body = String::new();
    if !title.trim().is_empty() {
        body.push_str(&docx_paragraph(title, true, 40, 0, 160));
    }
    for block in blocks {
        match block.kind.as_str() {
            "heading" => {
                let size = match block.level.clamp(1, 3) {
                    1 => 30,
                    2 => 26,
                    _ => 24,
                };
                body.push_str(&docx_paragraph(&block.text, true, size, 200, 80));
            }
            "bullet" => {
                for item in &block.items {
                    body.push_str(&docx_paragraph(&format!("•  {item}"), false, 0, 0, 60));
                }
                if block.items.is_empty() && !block.text.is_empty() {
                    body.push_str(&docx_paragraph(&format!("•  {}", block.text), false, 0, 0, 60));
                }
            }
            "table" => {
                if !block.rows.is_empty() {
                    body.push_str(&docx_table(&block.rows));
                    body.push_str("<w:p/>");
                }
            }
            // Anything else is a paragraph.
            _ => {
                if !block.text.is_empty() {
                    body.push_str(&docx_paragraph(&block.text, false, 0, 0, 120));
                }
            }
        }
    }
    let document = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><w:body>{body}<w:sectPr/></w:body></w:document>"
    );
    Ok(build_zip(&[
        content_types_docx(),
        root_rels(),
        ZipEntry {
            name: "word/document.xml".to_string(),
            data: document.into_bytes(),
        },
    ]))
}

// --- Excel -------------------------------------------------------------

/// Column letters A..Z, AA.. for indexes 0..701.
fn column_letter(index: usize) -> String {
    let mut n = index as u32 + 1;
    let mut letters = Vec::new();
    while n > 0 {
        n -= 1;
        letters.push((b'A' + (n % 26) as u8) as char);
        n /= 26;
    }
    letters.into_iter().rev().collect()
}

fn sheet_xml(rows: &[Vec<String>]) -> String {
    let mut body = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\"><sheetData>",
    );
    for (r, row) in rows.iter().enumerate() {
        body.push_str(&format!("<row r=\"{}\">", r + 1));
        for (c, cell) in row.iter().enumerate() {
            let reference = format!("{}{}", column_letter(c), r + 1);
            if let Ok(number) = cell.trim().parse::<f64>() {
                body.push_str(&format!("<c r=\"{reference}\"><v>{number}</v></c>"));
            } else if !cell.is_empty() {
                body.push_str(&format!(
                    "<c r=\"{reference}\" t=\"inlineStr\"><is><t xml:space=\"preserve\">{}</t></is></c>",
                    xml(cell)
                ));
            }
        }
        body.push_str("</row>");
    }
    body.push_str("</sheetData></worksheet>");
    body
}

/// Builds an .xlsx workbook, one sheet per `sheets`.
pub fn make_xlsx(sheets: &[Sheet]) -> Result<Vec<u8>, AppError> {
    if sheets.is_empty() {
        return Err(AppError::invalid_input(
            "An Excel file needs at least one sheet with rows.",
        ));
    }
    let mut workbook_sheets = String::new();
    let mut workbook_rels = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">",
    );
    let mut entries = Vec::new();
    for (index, sheet) in sheets.iter().enumerate() {
        let id = index + 1;
        workbook_sheets.push_str(&format!(
            "<sheet name=\"{}\" sheetId=\"{id}\" r:id=\"rId{id}\"/>",
            xml(&sheet.name)
        ));
        workbook_rels.push_str(&format!(
            "<Relationship Id=\"rId{id}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet\" Target=\"worksheets/sheet{id}.xml\"/>"
        ));
        entries.push(ZipEntry {
            name: format!("xl/worksheets/sheet{id}.xml"),
            data: sheet_xml(&sheet.rows).into_bytes(),
        });
    }
    workbook_sheets.push_str("</sheets>");
    workbook_rels.push_str("</Relationships>");
    let mut content_types = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"xml\" ContentType=\"application/xml\"/><Override PartName=\"/xl/workbook.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml\"/>",
    );
    for index in 1..=sheets.len() {
        content_types.push_str(&format!(
            "<Override PartName=\"/xl/worksheets/sheet{index}.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml\"/>"
        ));
    }
    content_types.push_str("</Types>");
    let workbook = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><workbook xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\"><sheets>{workbook_sheets}</sheets></workbook>"
    );
    let mut all = vec![
        ZipEntry {
            name: "[Content_Types].xml".to_string(),
            data: content_types.into_bytes(),
        },
        ZipEntry {
            name: "_rels/.rels".to_string(),
            data: br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#.to_vec(),
        },
        ZipEntry {
            name: "xl/workbook.xml".to_string(),
            data: workbook.into_bytes(),
        },
        ZipEntry {
            name: "xl/_rels/workbook.xml.rels".to_string(),
            data: workbook_rels.into_bytes(),
        },
    ];
    all.append(&mut entries);
    Ok(build_zip(&all))
}

// --- PowerPoint --------------------------------------------------------

const THEME1: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" name="LocalAide"><a:themeElements><a:clrScheme name="Office"><a:dk1><a:sysClr val="windowText" lastClr="000000"/></a:dk1><a:lt1><a:sysClr val="window" lastClr="FFFFFF"/></a:lt1><a:dk2><a:srgbClr val="14100C"/></a:dk2><a:lt2><a:srgbClr val="F7F5F0"/></a:lt2><a:accent1><a:srgbClr val="FF7A1A"/></a:accent1><a:accent2><a:srgbClr val="B9520A"/></a:accent2><a:accent3><a:srgbClr val="93836F"/></a:accent3><a:accent4><a:srgbClr val="6D675D"/></a:accent4><a:accent5><a:srgbClr val="FFD0A3"/></a:accent5><a:accent6><a:srgbClr val="E8680C"/></a:accent6></a:clrScheme><a:fontScheme name="Office"><a:majorFont><a:latin typeface="Calibri Light"/><a:ea typeface=""/><a:cs typeface=""/></a:majorFont><a:minorFont><a:latin typeface="Calibri"/><a:ea typeface=""/><a:cs typeface=""/></a:minorFont></a:fontScheme><a:fillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:fillStyleLst><a:lnStyleLst><a:ln><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln></a:lnStyleLst><a:effectStyleLst><a:effectStyle><a:effectLst/></a:effectStyle></a:effectStyleLst><a:bgFillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:bgFillStyleLst></a:themeElements></a:theme>"#;

fn slide_text(x: i32, y: i32, w: i32, h: i32, size: i32, bold: bool, lines: &[String]) -> String {
    let paragraphs: String = lines
        .iter()
        .map(|line| {
            format!(
                "<a:p><a:pPr/><a:r><a:rPr lang=\"en-US\" sz=\"{}\" b=\"{}\"/><a:t>{}</a:t></a:r></a:p>",
                size * 100,
                if bold { 1 } else { 0 },
                xml(line)
            )
        })
        .collect();
    format!(
        "<p:sp><p:spPr><a:xfrm><a:off x=\"{x}\" y=\"{y}\"/><a:ext cx=\"{w}\" cy=\"{h}\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></p:spPr><p:txBody><a:bodyPr/><a:lstStyle/>{paragraphs}</p:txBody></p:sp>",
    )
}

/// Builds a .pptx deck, one slide per `slides`, with the minimum part set
/// PowerPoint accepts: presentation, master, layout, theme and slides.
pub fn make_pptx(title: &str, slides: &[Slide]) -> Result<Vec<u8>, AppError> {
    if slides.is_empty() {
        return Err(AppError::invalid_input(
            "A deck needs at least one slide with a title.",
        ));
    }
    let mut slide_parts = Vec::new();
    let mut presentation_ids = String::new();
    let mut presentation_rels = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster\" Target=\"slideMasters/slideMaster1.xml\"/>",
    );
    let mut content_types = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"xml\" ContentType=\"application/xml\"/><Override PartName=\"/ppt/presentation.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml\"/><Override PartName=\"/ppt/slideMasters/slideMaster1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml\"/><Override PartName=\"/ppt/slideLayouts/slideLayout1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml\"/><Override PartName=\"/ppt/theme/theme1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.theme+xml\"/>",
    );
    for (index, slide) in slides.iter().enumerate() {
        let n = index + 1;
        let mut body = slide_text(457200, 274638, 7772400, 1325563, 40, true, &[slide.title.clone()]);
        let bullets: Vec<String> = if slide.bullets.is_empty() {
            vec![String::new()]
        } else {
            slide
                .bullets
                .iter()
                .map(|b| format!("•  {b}"))
                .collect()
        };
        body.push_str(&slide_text(457200, 1828800, 7772400, 4114800, 18, false, &bullets));
        let slide_xml = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><p:sld xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" xmlns:p=\"http://schemas.openxmlformats.org/presentationml/2006/main\"><p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id=\"1\" name=\"\"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/>{body}</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sld>"
        );
        slide_parts.push((
            format!("ppt/slides/slide{n}.xml"),
            slide_xml,
            format!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout\" Target=\"../slideLayouts/slideLayout1.xml\"/></Relationships>"
            ),
        ));
        presentation_ids.push_str(&format!(
            "<p:sldId id=\"{n}\" r:id=\"rId{}\"/>",
            n + 1
        ));
        presentation_rels.push_str(&format!(
            "<Relationship Id=\"rId{}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide\" Target=\"slides/slide{n}.xml\"/>",
            n + 1
        ));
        content_types.push_str(&format!(
            "<Override PartName=\"/ppt/slides/slide{n}.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.presentationml.slide+xml\"/>"
        ));
    }
    presentation_rels.push_str("</Relationships>");
    content_types.push_str("</Types>");
    let presentation = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><p:presentation xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" xmlns:p=\"http://schemas.openxmlformats.org/presentationml/2006/main\"><p:sldMasterIdLst><p:sldMasterId id=\"2147483648\" r:id=\"rId1\"/></p:sldMasterIdLst><p:sldIdLst>{presentation_ids}</p:sldIdLst><p:sldSz cx=\"9144000\" cy=\"6858000\"/></p:presentation>"
    );
    const MASTER: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sldMaster xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/></p:spTree></p:cSld><p:clrMap bg1="lt1" tx1="dk1" bg2="lt2" tx2="dk2" accent1="accent1" accent2="accent2" accent3="accent3" accent4="accent4" accent5="accent5" accent6="accent6" hlink="hlink" folHlink="folHlink"/><p:sldLayoutIdLst><p:sldLayoutId id="2147483649" r:id="rId1"/></p:sldLayoutIdLst></p:sldMaster>"#;
    const MASTER_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" Target="../theme/theme1.xml"/></Relationships>"#;
    const LAYOUT: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sldLayout xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" type="blank"><p:cSld name="Blank"><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/></p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sldLayout>"#;
    const LAYOUT_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="../slideMasters/slideMaster1.xml"/></Relationships>"#;
    let mut entries = vec![
        ZipEntry {
            name: "[Content_Types].xml".to_string(),
            data: content_types.into_bytes(),
        },
        ZipEntry {
            name: "_rels/.rels".to_string(),
            data: br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="ppt/presentation.xml"/></Relationships>"#.to_vec(),
        },
        ZipEntry {
            name: "ppt/presentation.xml".to_string(),
            data: presentation.into_bytes(),
        },
        ZipEntry {
            name: "ppt/_rels/presentation.xml.rels".to_string(),
            data: presentation_rels.into_bytes(),
        },
        ZipEntry {
            name: "ppt/slideMasters/slideMaster1.xml".to_string(),
            data: MASTER.as_bytes().to_vec(),
        },
        ZipEntry {
            name: "ppt/slideMasters/_rels/slideMaster1.xml.rels".to_string(),
            data: MASTER_RELS.as_bytes().to_vec(),
        },
        ZipEntry {
            name: "ppt/slideLayouts/slideLayout1.xml".to_string(),
            data: LAYOUT.as_bytes().to_vec(),
        },
        ZipEntry {
            name: "ppt/slideLayouts/_rels/slideLayout1.xml.rels".to_string(),
            data: LAYOUT_RELS.as_bytes().to_vec(),
        },
        ZipEntry {
            name: "ppt/theme/theme1.xml".to_string(),
            data: THEME1.as_bytes().to_vec(),
        },
    ];
    for (name, xml_body, rels) in slide_parts {
        let rels_name = name.replacen("ppt/slides/", "ppt/slides/_rels/", 1);
        entries.push(ZipEntry {
            name,
            data: xml_body.into_bytes(),
        });
        entries.push(ZipEntry {
            name: rels_name,
            data: rels.into_bytes(),
        });
    }
    Ok(build_zip(&entries))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A ZIP is well-formed when every local header's name matches its
    /// central record and the EOCD points at the directory.
    #[test]
    fn zip_round_trips_entries() {
        let entries = vec![
            ZipEntry {
                name: "a.txt".to_string(),
                data: b"hello".to_vec(),
            },
            ZipEntry {
                name: "b/c.txt".to_string(),
                data: vec![0u8; 300],
            },
        ];
        let zip = build_zip(&entries);
        assert_eq!(&zip[0..4], &0x0403_4b50u32.to_le_bytes());
        let eocd_at = zip.len() - 22;
        assert_eq!(&zip[eocd_at..eocd_at + 4], &0x0605_4b50u32.to_le_bytes());
        assert_eq!(crc32(b"hello"), crc32(b"hello"));
        assert_ne!(crc32(b"hello"), crc32(b"jello"));
    }

    #[test]
    fn docx_builds_with_blocks() {
        let blocks = vec![
            DocBlock { kind: "heading".into(), text: "Chapter".into(), items: vec![], rows: vec![], level: 1 },
            DocBlock { kind: "paragraph".into(), text: "Words & <marks>".into(), items: vec![], rows: vec![], level: 0 },
            DocBlock { kind: "bullet".into(), text: String::new(), items: vec!["one".into(), "two".into()], rows: vec![], level: 0 },
            DocBlock { kind: "table".into(), text: String::new(), items: vec![], rows: vec![vec!["a".into(), "b".into()], vec!["1".into(), "2".into()]], level: 0 },
        ];
        let zip = make_docx("Report", &blocks).unwrap();
        assert!(zip.len() > 400);
        assert!(zip.windows(4).any(|w| w == b"PK\x03\x04"));
    }

    #[test]
    fn xlsx_builds_and_numbers_stay_numbers() {
        let sheets = vec![Sheet {
            name: "Q1".into(),
            rows: vec![vec!["item".into(), "qty".into()], vec!["nails".into(), "42".into()]],
        }];
        let zip = make_xlsx(&sheets).unwrap();
        assert!(zip.len() > 400);
        let empty: Vec<Sheet> = Vec::new();
        assert!(make_xlsx(&empty).is_err());
    }

    #[test]
    fn pptx_builds_a_deck() {
        let slides = vec![Slide { title: "Overview".into(), bullets: vec!["First".into(), "Second".into()] }];
        let zip = make_pptx("Deck", &slides).unwrap();
        assert!(zip.len() > 1000);
        assert!(make_pptx("Deck", &[]).is_err());
    }

    #[test]
    fn column_letters_walk() {
        assert_eq!(column_letter(0), "A");
        assert_eq!(column_letter(25), "Z");
        assert_eq!(column_letter(26), "AA");
        assert_eq!(column_letter(701), "ZZ");
    }
}
