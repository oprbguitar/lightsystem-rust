use pos_core::{format_money, Sale};
use std::{
    fs,
    path::{Path, PathBuf},
};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum PrintError {
    #[error("No se pudieron generar los documentos: {0}")]
    Io(#[from] std::io::Error),
    #[error("No se pudo serializar la metadata: {0}")]
    Json(#[from] serde_json::Error),
    #[error("El identificador de venta no es UUID válido: {0}")]
    InvalidSaleId(String),
}

#[derive(Debug)]
pub struct TicketFiles {
    pub directory: PathBuf,
    pub txt: PathBuf,
    pub pdf: PathBuf,
    pub escpos: PathBuf,
    pub metadata: PathBuf,
}

#[derive(Debug, Clone, Default)]
pub struct TicketBranding {
    pub logo_text: String,
    pub commercial_name: String,
    pub address: String,
    pub phone: String,
}

pub struct TicketWriter {
    root: PathBuf,
    branding: TicketBranding,
}
impl TicketWriter {
    pub fn new(root: impl AsRef<Path>) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
            branding: TicketBranding::default(),
        }
    }
    pub fn with_branding(mut self, branding: TicketBranding) -> Self {
        self.branding = branding;
        self
    }
    pub fn write(&self, sale: &Sale) -> Result<TicketFiles, PrintError> {
        Uuid::parse_str(&sale.id).map_err(|_| PrintError::InvalidSaleId(sale.id.clone()))?;
        fs::create_dir_all(&self.root)?;
        let directory = self.root.join(&sale.id);
        let txt = directory.join("ticket.txt");
        let pdf = directory.join("ticket.pdf");
        let escpos = directory.join("ticket.escpos");
        let metadata = directory.join("metadata.json");
        if directory.exists() {
            if existing_ticket_matches(&directory, sale, &self.branding) {
                return Ok(TicketFiles {
                    directory,
                    txt,
                    pdf,
                    escpos,
                    metadata,
                });
            }
            let recovery = self.root.join("recovery");
            fs::create_dir_all(&recovery)?;
            let preserved = recovery.join(format!("{}-{}", sale.id, Uuid::new_v4()));
            fs::rename(&directory, preserved)?;
        }
        let staging = self
            .root
            .join(format!(".{}.tmp-{}", sale.id, Uuid::new_v4()));
        fs::create_dir(&staging)?;
        let text = render_text_with_branding(sale, &self.branding);
        let staged_paths = ["ticket.txt", "ticket.pdf", "ticket.escpos", "metadata.json"]
            .map(|name| staging.join(name));
        let write_result = (|| -> Result<(), PrintError> {
            fs::write(&staged_paths[0], &text)?;
            fs::write(&staged_paths[1], render_pdf(&text))?;
            fs::write(&staged_paths[2], render_escpos(&text))?;
            fs::write(&staged_paths[3], serde_json::to_vec_pretty(sale)?)?;
            fs::rename(&staging, &directory)?;
            Ok(())
        })();
        if write_result.is_err() {
            let _ = fs::remove_dir_all(&staging);
        }
        write_result?;
        Ok(TicketFiles {
            directory,
            txt,
            pdf,
            escpos,
            metadata,
        })
    }
}

fn existing_ticket_matches(directory: &Path, sale: &Sale, branding: &TicketBranding) -> bool {
    let text_path = directory.join("ticket.txt");
    let pdf_path = directory.join("ticket.pdf");
    let escpos_path = directory.join("ticket.escpos");
    let metadata_path = directory.join("metadata.json");
    if ![&text_path, &pdf_path, &escpos_path, &metadata_path]
        .iter()
        .all(|path| path.is_file())
    {
        return false;
    }
    let rendered_text = render_text_with_branding(sale, branding);
    let artifacts_match = fs::read(text_path).is_ok_and(|bytes| bytes == rendered_text.as_bytes())
        && fs::read(pdf_path).is_ok_and(|bytes| bytes == render_pdf(&rendered_text))
        && fs::read(escpos_path).is_ok_and(|bytes| bytes == render_escpos(&rendered_text));
    artifacts_match
        && fs::read(metadata_path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Sale>(&bytes).ok())
            .is_some_and(|existing| existing == *sale)
}

pub fn render_text(sale: &Sale) -> String {
    render_text_with_branding(
        sale,
        &TicketBranding {
            logo_text: "[ MINIPOS ]".into(),
            commercial_name: "MiniPOS Demo".into(),
            ..TicketBranding::default()
        },
    )
}

pub fn render_text_with_branding(sale: &Sale, branding: &TicketBranding) -> String {
    let lima = chrono::FixedOffset::west_opt(5 * 60 * 60).expect("UTC offset is valid");
    let created_local = sale.created_at.with_timezone(&lima);
    let taxable_base_minor = sale.taxable_base_minor().unwrap_or(sale.total_minor);
    let igv_minor = sale.igv_minor().unwrap_or(0);
    let mut lines = Vec::new();
    if !branding.logo_text.trim().is_empty() {
        lines.extend(branding.logo_text.lines().map(str::to_owned));
    }
    if !branding.commercial_name.trim().is_empty() {
        lines.push(branding.commercial_name.trim().to_owned());
    }
    lines.extend([
        sale.checkout.as_ref().map_or_else(
            || "NOTA DE VENTA · DEMO".to_owned(),
            |checkout| checkout.document_kind.clone(),
        ),
        "DOCUMENTO INTERNO - SIN VALIDEZ TRIBUTARIA".to_owned(),
    ]);
    if !branding.address.trim().is_empty() {
        lines.push(format!("DIRECCIÓN: {}", branding.address.trim()));
    }
    if !branding.phone.trim().is_empty() {
        lines.push(format!("TELÉFONO: {}", branding.phone.trim()));
    }
    lines.extend([
        format!("CAJA: {}", sale.instance_id),
        format!("OPERACIÓN: {}", &sale.id[..8.min(sale.id.len())]),
        format!(
            "FECHA Y HORA (PERÚ): {}",
            created_local.format("%d/%m/%Y %H:%M:%S")
        ),
        "----------------------------------------".into(),
    ]);
    if let Some(checkout) = sale.checkout.as_ref() {
        if !checkout.customer_name.trim().is_empty() {
            lines.push(format!("CLIENTE: {}", checkout.customer_name.trim()));
        }
        if !checkout.customer_document_number.trim().is_empty() {
            lines.push(format!(
                "{}: {}",
                checkout.customer_document_kind, checkout.customer_document_number
            ));
        }
        if !checkout.document_number.trim().is_empty() {
            lines.push(format!("N.º DOCUMENTO DEMO: {}", checkout.document_number));
        }
        lines.push("SKU     PRODUCTO / DESCRIPCIÓN          CANT.  PRECIO      IMPORTE".into());
    }
    for line in &sale.lines {
        let line_base = (i128::from(line.total_minor) * 100 + 59) / 118;
        let line_base = i64::try_from(line_base).unwrap_or(line.total_minor);
        let line_igv = line.total_minor.saturating_sub(line_base);
        lines.push(format!(
            "{} {} x{} · unit {} · total {}",
            line.product_code,
            line.name,
            line.quantity,
            format_money(line.unit_price_minor),
            format_money(line.total_minor)
        ));
        lines.push(format!(
            "  Base {} · IGV demo {}",
            format_money(line_base),
            format_money(line_igv)
        ));
    }
    lines.extend([
        "----------------------------------------".into(),
        format!("SUBTOTAL: {}", format_money(taxable_base_minor)),
        format!("IGV DEMO (18%): {}", format_money(igv_minor)),
        format!("TOTAL: {}", format_money(sale.total_minor)),
        format!("PAGO: {}", sale.payment_method),
    ]);
    if let Some(checkout) = sale.checkout.as_ref() {
        lines.push(format!(
            "RECIBIDO: {}",
            format_money(checkout.amount_received_minor)
        ));
        lines.push(format!("VUELTO: {}", format_money(checkout.change_minor)));
        lines.push(format!(
            "TIPO DE CAMBIO REFERENCIAL: S/ {:.3} por USD",
            checkout.fx_rate_milli as f64 / 1_000.0
        ));
        let usd_cents = i128::from(sale.total_minor)
            .checked_mul(1_000)
            .and_then(|value| value.checked_div(i128::from(checkout.fx_rate_milli.max(1))))
            .unwrap_or(0);
        lines.push(format!(
            "TOTAL REFERENCIAL USD: $ {}.{:02}",
            usd_cents / 100,
            usd_cents % 100
        ));
    }
    lines.push("".into());
    lines.join("\r\n")
}

fn render_pdf(text: &str) -> Vec<u8> {
    const LINES_PER_PAGE: usize = 52;
    let lines = text.lines().collect::<Vec<_>>();
    let page_count = lines.len().max(1).div_ceil(LINES_PER_PAGE);
    let mut objects = vec![Vec::new(); 3 + page_count * 2];
    objects[0] = b"<< /Type /Catalog /Pages 2 0 R >>".to_vec();
    objects[1] = format!(
        "<< /Type /Pages /Kids [{}] /Count {page_count} >>",
        (0..page_count)
            .map(|index| format!("{} 0 R", 4 + index * 2))
            .collect::<Vec<_>>()
            .join(" ")
    )
    .into_bytes();
    objects[2] =
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Courier /Encoding /WinAnsiEncoding >>".to_vec();
    for (page_index, page_lines) in lines.chunks(LINES_PER_PAGE).enumerate() {
        let mut stream = b"BT /F1 10 Tf 40 800 Td\n".to_vec();
        for line in page_lines {
            stream.push(b'(');
            for character in line.chars() {
                match character {
                    '(' | ')' | '\\' => {
                        stream.push(b'\\');
                        stream.push(character as u8);
                    }
                    character if character.is_ascii() => stream.push(character as u8),
                    character => match win_ansi(character) {
                        Some(byte) => stream.extend_from_slice(format!("\\{byte:03o}").as_bytes()),
                        None => stream.push(b'?'),
                    },
                }
            }
            stream.extend_from_slice(b") Tj 0 -14 Td\n");
        }
        stream.extend_from_slice(b"ET");
        let page_object = 4 + page_index * 2;
        objects[page_object - 1] = format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 842] /Resources << /Font << /F1 3 0 R >> >> /Contents {} 0 R >>",
            page_object + 1
        )
        .into_bytes();
        objects[page_object] = [
            format!("<< /Length {} >>\nstream\n", stream.len()).into_bytes(),
            stream,
            b"\nendstream".to_vec(),
        ]
        .concat();
    }
    let mut pdf = b"%PDF-1.4\n%MiniPOS internal receipt\n".to_vec();
    let mut offsets = vec![0_usize];
    for (index, object) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        pdf.extend_from_slice(format!("{} 0 obj\n", index + 1).as_bytes());
        pdf.extend_from_slice(object);
        pdf.extend_from_slice(b"\nendobj\n");
    }
    let xref = pdf.len();
    pdf.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", offsets.len()).as_bytes());
    for offset in offsets.iter().skip(1) {
        pdf.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    pdf.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n",
            offsets.len(),
            xref
        )
        .as_bytes(),
    );
    pdf
}

fn win_ansi(character: char) -> Option<u8> {
    let value = character as u32;
    if (0xA0..=0xFF).contains(&value) {
        Some(value as u8)
    } else {
        Some(match character {
            '€' => 0x80,
            '‚' => 0x82,
            'ƒ' => 0x83,
            '„' => 0x84,
            '…' => 0x85,
            '†' => 0x86,
            '‡' => 0x87,
            'ˆ' => 0x88,
            '‰' => 0x89,
            'Š' => 0x8A,
            '‹' => 0x8B,
            'Œ' => 0x8C,
            'Ž' => 0x8E,
            '‘' => 0x91,
            '’' => 0x92,
            '“' => 0x93,
            '”' => 0x94,
            '•' => 0x95,
            '–' => 0x96,
            '—' => 0x97,
            '˜' => 0x98,
            '™' => 0x99,
            'š' => 0x9A,
            '›' => 0x9B,
            'œ' => 0x9C,
            'ž' => 0x9E,
            'Ÿ' => 0x9F,
            _ => return None,
        })
    }
}

fn render_escpos(text: &str) -> Vec<u8> {
    let mut bytes = vec![0x1b, 0x40, 0x1b, 0x61, 0x01];
    bytes.extend_from_slice(text.as_bytes());
    bytes.extend_from_slice(&[0x0a, 0x0a, 0x0a, 0x1d, 0x56, 0x00]);
    bytes
}
