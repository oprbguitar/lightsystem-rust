use pos_core::{Cart, Product, Sale};
use pos_print::TicketWriter;

#[test]
fn writes_internal_txt_pdf_escpos_and_metadata_for_a_sale() {
    let root = tempfile::tempdir().unwrap();
    let sale = Sale::from_cart(
        "CAJA01",
        Cart::new()
            .add_product(Product::new("DEMO-001", "Café Piña", 1250).unwrap(), 2)
            .unwrap(),
    )
    .unwrap();
    let files = TicketWriter::new(root.path()).write(&sale).unwrap();
    assert!(files.txt.is_file());
    assert!(files.pdf.is_file());
    assert!(files.escpos.is_file());
    assert!(files.metadata.is_file());
    let text = std::fs::read_to_string(files.txt).unwrap();
    assert!(text.contains("DOCUMENTO INTERNO - SIN VALIDEZ TRIBUTARIA"));
    assert!(text.contains("TOTAL: S/ 25.00"));
    let pdf = std::fs::read(files.pdf).unwrap();
    assert!(pdf.starts_with(b"%PDF-1.4"));
    assert!(pdf
        .windows(b"Caf\\351".len())
        .any(|window| window == b"Caf\\351"));
    let marker = b"startxref\n";
    let trailer_pos = pdf
        .windows(marker.len())
        .rposition(|window| window == marker)
        .unwrap();
    let xref_offset: usize = std::str::from_utf8(&pdf[trailer_pos + marker.len()..])
        .unwrap()
        .lines()
        .next()
        .unwrap()
        .parse()
        .unwrap();
    assert!(pdf[xref_offset..].starts_with(b"xref\n"));
    assert!(pdf.ends_with(b"%%EOF\n"));
}

#[test]
fn rejects_non_uuid_sale_ids_before_touching_the_output_directory() {
    let root = tempfile::tempdir().unwrap();
    let mut sale = Sale::from_cart(
        "CAJA01",
        Cart::new()
            .add_product(Product::new("X", "Producto", 50).unwrap(), 1)
            .unwrap(),
    )
    .unwrap();
    sale.id = "..\\escape".into();
    assert!(TicketWriter::new(root.path()).write(&sale).is_err());
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
}

#[test]
fn repeated_ticket_generation_is_idempotent_and_keeps_one_complete_folder() {
    let root = tempfile::tempdir().unwrap();
    let sale = Sale::from_cart(
        "CAJA01",
        Cart::new()
            .add_product(Product::new("X", "Café", 50).unwrap(), 1)
            .unwrap(),
    )
    .unwrap();
    let writer = TicketWriter::new(root.path());
    let first = writer.write(&sale).unwrap();
    let second = writer.write(&sale).unwrap();
    assert_eq!(first.directory, second.directory);
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 1);
}

#[test]
fn quarantines_incomplete_ticket_and_recreates_a_complete_receipt() {
    let root = tempfile::tempdir().unwrap();
    let sale = Sale::from_cart(
        "CAJA01",
        Cart::new()
            .add_product(Product::new("X", "Cafe", 50).unwrap(), 1)
            .unwrap(),
    )
    .unwrap();
    let existing = root.path().join(&sale.id);
    std::fs::create_dir(&existing).unwrap();
    std::fs::write(existing.join("ticket.txt"), "parcial").unwrap();
    let files = TicketWriter::new(root.path()).write(&sale).unwrap();
    assert_eq!(
        std::fs::read_to_string(
            root.path()
                .join("recovery")
                .read_dir()
                .unwrap()
                .next()
                .unwrap()
                .unwrap()
                .path()
                .join("ticket.txt")
        )
        .unwrap(),
        "parcial"
    );
    assert!(files.txt.is_file());
    assert!(files.pdf.is_file());
    assert!(files.escpos.is_file());
    assert!(files.metadata.is_file());
}

#[test]
fn quarantines_complete_ticket_with_mismatched_metadata_before_replay() {
    let root = tempfile::tempdir().unwrap();
    let sale = Sale::from_cart(
        "CAJA01",
        Cart::new()
            .add_product(Product::new("X", "Cafe", 50).unwrap(), 1)
            .unwrap(),
    )
    .unwrap();
    let writer = TicketWriter::new(root.path());
    let original = writer.write(&sale).unwrap();
    let mut mismatched = sale.clone();
    mismatched.instance_id = "CAJA99".into();
    std::fs::write(&original.metadata, serde_json::to_vec(&mismatched).unwrap()).unwrap();

    let repaired = writer.write(&sale).unwrap();
    let metadata: Sale =
        serde_json::from_slice(&std::fs::read(repaired.metadata).unwrap()).unwrap();
    assert_eq!(metadata, sale);
    assert_eq!(
        std::fs::read_dir(root.path().join("recovery"))
            .unwrap()
            .count(),
        1
    );
}

#[test]
fn quarantines_ticket_with_corrupted_artifact_even_when_metadata_matches() {
    let root = tempfile::tempdir().unwrap();
    let sale = Sale::from_cart(
        "CAJA01",
        Cart::new()
            .add_product(Product::new("X", "Cafe", 50).unwrap(), 1)
            .unwrap(),
    )
    .unwrap();
    let writer = TicketWriter::new(root.path());
    let existing = writer.write(&sale).unwrap();
    std::fs::write(&existing.pdf, b"corrupto").unwrap();

    let repaired = writer.write(&sale).unwrap();
    assert!(std::fs::read(&repaired.pdf)
        .unwrap()
        .starts_with(b"%PDF-1.4"));
    assert_eq!(
        std::fs::read_dir(root.path().join("recovery"))
            .unwrap()
            .count(),
        1
    );
}
