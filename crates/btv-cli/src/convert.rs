//! Conversores de export (validação de pendencias.md — antes todo formato
//! "binário" respondia 422 "em breve"). Converte o CONTEÚDO DE TEXTO de uma
//! entrega para os formatos de escritório/PDF por **serialização determinística
//! em Rust puro** — sem LibreOffice/pandoc e, portanto, **sem sandbox**: gerar
//! um DOCX/XLSX/PDF mínimo é escrever bytes de um formato conhecido, não rodar
//! código não confiável (o motivo do sandbox). DOCX/XLSX são ZIPs de XML
//! (OOXML); o ZIP é escrito à mão (método "stored", sem compressão + CRC32).
//!
//! **Honestidade sobre o que NÃO converte:** formatos que exigem renderização
//! ou conversão de mídia REAL — `PNG` (rasterização), `MIDI` (de MusicXML) —
//! seguem sem conversor (o handler devolve 422 honesto). `SVG`/`MusicXML` já
//! SÃO texto/XML; são servidos com o content-type certo, sem "conversão".

/// Resultado de uma conversão: bytes + content-type + extensão do arquivo.
pub struct Converted {
    pub bytes: Vec<u8>,
    pub content_type: &'static str,
    pub extension: &'static str,
}

/// Converte o texto para o `formato` (case-insensitive). `None` para formatos
/// sem conversor honesto (o chamador devolve 422). `SVG`/`MusicXML` passam como
/// texto/XML (já são markup), sem transformação.
pub fn convert(formato: &str, text: &str) -> Option<Converted> {
    match formato.to_ascii_lowercase().as_str() {
        "docx" => Some(Converted {
            bytes: to_docx(text),
            content_type: "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            extension: "docx",
        }),
        "xlsx" => Some(Converted {
            bytes: to_xlsx(text),
            content_type: "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            extension: "xlsx",
        }),
        "pdf" => Some(Converted {
            bytes: to_pdf(text),
            content_type: "application/pdf",
            extension: "pdf",
        }),
        "svg" => Some(Converted {
            bytes: text.as_bytes().to_vec(),
            content_type: "image/svg+xml",
            extension: "svg",
        }),
        "musicxml" => Some(Converted {
            bytes: text.as_bytes().to_vec(),
            content_type: "application/vnd.recordare.musicxml+xml",
            extension: "musicxml",
        }),
        // PNG (rasterização), MIDI (conversão de mídia): sem conversor honesto.
        _ => None,
    }
}

// ── XML helpers ────────────────────────────────────────────────────────────

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

// ── DOCX (WordprocessingML mínimo, mas válido) ─────────────────────────────

fn to_docx(text: &str) -> Vec<u8> {
    let paragraphs: String = text
        .lines()
        .map(|line| {
            format!(
                "<w:p><w:r><w:t xml:space=\"preserve\">{}</w:t></w:r></w:p>",
                xml_escape(line)
            )
        })
        .collect();
    let document = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
<w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\">\
<w:body>{paragraphs}<w:sectPr/></w:body></w:document>"
    );
    let content_types = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">\
<Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>\
<Default Extension=\"xml\" ContentType=\"application/xml\"/>\
<Override PartName=\"/word/document.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml\"/>\
</Types>";
    let rels = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
<Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"word/document.xml\"/>\
</Relationships>";
    zip_stored(&[
        ("[Content_Types].xml", content_types.as_bytes()),
        ("_rels/.rels", rels.as_bytes()),
        ("word/document.xml", document.as_bytes()),
    ])
}

// ── XLSX (SpreadsheetML mínimo, cada linha do texto vira uma linha) ────────

fn to_xlsx(text: &str) -> Vec<u8> {
    let rows: String = text
        .lines()
        .enumerate()
        .map(|(i, line)| {
            let r = i + 1;
            format!(
                "<row r=\"{r}\"><c r=\"A{r}\" t=\"inlineStr\"><is><t xml:space=\"preserve\">{}</t></is></c></row>",
                xml_escape(line)
            )
        })
        .collect();
    let sheet = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
<worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\">\
<sheetData>{rows}</sheetData></worksheet>"
    );
    let workbook = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
<workbook xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" \
xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\">\
<sheets><sheet name=\"Planilha1\" sheetId=\"1\" r:id=\"rId1\"/></sheets></workbook>";
    let wb_rels = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
<Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet\" Target=\"worksheets/sheet1.xml\"/>\
</Relationships>";
    let content_types = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">\
<Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>\
<Default Extension=\"xml\" ContentType=\"application/xml\"/>\
<Override PartName=\"/xl/workbook.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml\"/>\
<Override PartName=\"/xl/worksheets/sheet1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml\"/>\
</Types>";
    let rels = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
<Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"xl/workbook.xml\"/>\
</Relationships>";
    zip_stored(&[
        ("[Content_Types].xml", content_types.as_bytes()),
        ("_rels/.rels", rels.as_bytes()),
        ("xl/workbook.xml", workbook.as_bytes()),
        ("xl/_rels/workbook.xml.rels", wb_rels.as_bytes()),
        ("xl/worksheets/sheet1.xml", sheet.as_bytes()),
    ])
}

// ── PDF (uma página, fonte Helvetica, texto em várias linhas) ──────────────

fn pdf_escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('(', "\\(")
        .replace(')', "\\)")
}

fn to_pdf(text: &str) -> Vec<u8> {
    // Stream de conteúdo: começa no topo, uma linha por `Td` de -14pt.
    let mut content = String::from("BT\n/F1 12 Tf\n14 TL\n72 760 Td\n");
    let mut first = true;
    for line in text.lines() {
        if first {
            content.push_str(&format!("({}) Tj\n", pdf_escape(line)));
            first = false;
        } else {
            content.push_str(&format!("T*\n({}) Tj\n", pdf_escape(line)));
        }
    }
    if first {
        // texto vazio — ainda produz um PDF válido de página em branco.
        content.push_str("() Tj\n");
    }
    content.push_str("ET");

    // Objetos: 1 catalog, 2 pages, 3 page, 4 content, 5 font.
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>".to_string(),
        format!("<< /Length {} >>\nstream\n{content}\nendstream", content.len()),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string(),
    ];

    let mut pdf = Vec::new();
    pdf.extend_from_slice(b"%PDF-1.4\n");
    let mut offsets = Vec::with_capacity(objects.len());
    for (i, obj) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        pdf.extend_from_slice(format!("{} 0 obj\n{obj}\nendobj\n", i + 1).as_bytes());
    }
    let xref_pos = pdf.len();
    pdf.extend_from_slice(format!("xref\n0 {}\n", objects.len() + 1).as_bytes());
    pdf.extend_from_slice(b"0000000000 65535 f \n");
    for off in &offsets {
        pdf.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
    }
    pdf.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref_pos}\n%%EOF",
            objects.len() + 1
        )
        .as_bytes(),
    );
    pdf
}

// ── ZIP "stored" (sem compressão) escrito à mão + CRC32 ────────────────────

fn crc32(data: &[u8]) -> u32 {
    // Tabela IEEE 802.3 (poly 0xEDB88320), calculada uma vez.
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

/// Escreve um ZIP com os arquivos dados, método 0 (stored). Suficiente para os
/// pacotes OOXML (DOCX/XLSX), que aceitam entradas não comprimidas.
fn zip_stored(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    let mut offsets = Vec::with_capacity(files.len());

    for (name, data) in files {
        let crc = crc32(data);
        let name_bytes = name.as_bytes();
        offsets.push(out.len() as u32);
        // Local file header.
        out.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        out.extend_from_slice(&20u16.to_le_bytes()); // version needed
        out.extend_from_slice(&0u16.to_le_bytes()); // flags
        out.extend_from_slice(&0u16.to_le_bytes()); // method: stored
        out.extend_from_slice(&0u16.to_le_bytes()); // mod time
        out.extend_from_slice(&0u16.to_le_bytes()); // mod date
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&(data.len() as u32).to_le_bytes()); // comp size
        out.extend_from_slice(&(data.len() as u32).to_le_bytes()); // uncomp size
        out.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes()); // extra len
        out.extend_from_slice(name_bytes);
        out.extend_from_slice(data);

        // Central directory record.
        central.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
        central.extend_from_slice(&20u16.to_le_bytes()); // version made by
        central.extend_from_slice(&20u16.to_le_bytes()); // version needed
        central.extend_from_slice(&0u16.to_le_bytes()); // flags
        central.extend_from_slice(&0u16.to_le_bytes()); // method
        central.extend_from_slice(&0u16.to_le_bytes()); // mod time
        central.extend_from_slice(&0u16.to_le_bytes()); // mod date
        central.extend_from_slice(&crc.to_le_bytes());
        central.extend_from_slice(&(data.len() as u32).to_le_bytes());
        central.extend_from_slice(&(data.len() as u32).to_le_bytes());
        central.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes()); // extra
        central.extend_from_slice(&0u16.to_le_bytes()); // comment
        central.extend_from_slice(&0u16.to_le_bytes()); // disk start
        central.extend_from_slice(&0u16.to_le_bytes()); // internal attr
        central.extend_from_slice(&0u32.to_le_bytes()); // external attr
        central.extend_from_slice(&offsets[offsets.len() - 1].to_le_bytes());
        central.extend_from_slice(name_bytes);
    }

    let central_offset = out.len() as u32;
    let central_size = central.len() as u32;
    out.extend_from_slice(&central);
    // End of central directory.
    out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes()); // disk
    out.extend_from_slice(&0u16.to_le_bytes()); // disk with central dir
    out.extend_from_slice(&(files.len() as u16).to_le_bytes());
    out.extend_from_slice(&(files.len() as u16).to_le_bytes());
    out.extend_from_slice(&central_size.to_le_bytes());
    out.extend_from_slice(&central_offset.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes()); // comment len
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc32_bate_valor_conhecido() {
        // CRC32("123456789") = 0xCBF43926 (vetor de teste padrão).
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn docx_e_um_zip_valido_com_o_texto() {
        let c = convert("DOCX", "linha um\nlinha dois").unwrap();
        assert_eq!(c.extension, "docx");
        // Assinatura ZIP local header + EOCD presentes.
        assert_eq!(&c.bytes[0..4], &[0x50, 0x4b, 0x03, 0x04]);
        assert!(
            c.bytes.windows(4).any(|w| w == [0x50, 0x4b, 0x05, 0x06]),
            "EOCD ausente"
        );
        // O texto entra no document.xml (não comprimido → aparece nos bytes).
        let s = String::from_utf8_lossy(&c.bytes);
        assert!(s.contains("word/document.xml"));
        assert!(s.contains("linha um") && s.contains("linha dois"));
    }

    #[test]
    fn xlsx_uma_linha_por_texto() {
        let c = convert("xlsx", "a\nb\nc").unwrap();
        assert_eq!(c.extension, "xlsx");
        let s = String::from_utf8_lossy(&c.bytes);
        assert!(s.contains("xl/worksheets/sheet1.xml"));
        assert!(s.contains("r=\"1\"") && s.contains("r=\"3\""));
    }

    #[test]
    fn pdf_tem_cabecalho_e_texto() {
        let c = convert("PDF", "olá mundo\nsegunda linha").unwrap();
        assert_eq!(c.extension, "pdf");
        assert!(c.bytes.starts_with(b"%PDF-1.4"));
        assert!(c.bytes.ends_with(b"%%EOF"));
        let s = String::from_utf8_lossy(&c.bytes);
        assert!(s.contains("olá mundo") && s.contains("segunda linha"));
        assert!(s.contains("/Type /Catalog") && s.contains("startxref"));
    }

    #[test]
    fn pdf_escapa_parenteses() {
        let c = convert("PDF", "texto (com) parênteses").unwrap();
        let s = String::from_utf8_lossy(&c.bytes);
        assert!(s.contains("\\(com\\)"));
    }

    #[test]
    fn svg_e_musicxml_passam_como_texto() {
        assert_eq!(
            convert("SVG", "<svg/>").unwrap().content_type,
            "image/svg+xml"
        );
        assert_eq!(
            convert("MusicXML", "<score/>").unwrap().extension,
            "musicxml"
        );
    }

    #[test]
    fn formatos_sem_conversor_honesto_devolvem_none() {
        assert!(convert("PNG", "x").is_none());
        assert!(convert("MIDI", "x").is_none());
    }

    /// Prova mais forte que a inspeção de bytes: o DOCX/XLSX gerado abre num
    /// leitor de ZIP REAL (`zipfile` do Python), com CRC íntegro e as partes
    /// OOXML esperadas. Pula graciosamente se `python3` não existir.
    #[test]
    fn docx_e_xlsx_abrem_num_leitor_de_zip_real() {
        let py = std::process::Command::new("python3")
            .arg("--version")
            .output();
        if py.map(|o| !o.status.success()).unwrap_or(true) {
            eprintln!("skip: python3 ausente");
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        for (fmt, arquivo, parte) in [
            ("DOCX", "d.docx", "word/document.xml"),
            ("XLSX", "s.xlsx", "xl/worksheets/sheet1.xml"),
        ] {
            let c = convert(fmt, "primeira\nsegunda").unwrap();
            let path = dir.path().join(arquivo);
            std::fs::write(&path, &c.bytes).unwrap();
            let script = format!(
                "import zipfile,sys\n\
                 z=zipfile.ZipFile(r'{}')\n\
                 assert z.testzip() is None, 'CRC quebrado'\n\
                 assert '{}' in z.namelist(), 'parte OOXML ausente'\n\
                 assert b'primeira' in z.read('{}'), 'texto ausente'\n\
                 print('ok')",
                path.display(),
                parte,
                parte
            );
            let out = std::process::Command::new("python3")
                .arg("-c")
                .arg(&script)
                .output()
                .unwrap();
            assert!(
                out.status.success(),
                "{fmt} não abriu como ZIP válido: {}",
                String::from_utf8_lossy(&out.stderr)
            );
        }
    }
}
