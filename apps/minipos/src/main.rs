use clap::Parser;
use pos_core::{Cart, Sale};
use pos_devices::AppConfig;
use pos_print::TicketWriter;
use pos_storage::Store;
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process::ExitCode,
};

#[derive(Debug, Parser)]
#[command(
    name = "minipos",
    version,
    about = "MiniPOS · terminal de punto de venta local"
)]
struct Args {
    #[arg(long, default_value = "CAJA01")]
    instance: String,
    #[arg(long)]
    data_dir: Option<PathBuf>,
    #[arg(long)]
    config: Option<PathBuf>,
    #[arg(long, help = "Carga el catálogo de demostración sin conexión")]
    demo: bool,
    #[arg(
        long,
        help = "Registra una venta de muestra y genera TXT/PDF; útil para validar la instalación"
    )]
    demo_sale: bool,
}

fn main() -> ExitCode {
    match run(Args::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("MiniPOS: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    validate_instance(&args.instance)?;
    let data_dir = args
        .data_dir
        .unwrap_or_else(|| PathBuf::from("datos").join(args.instance.to_lowercase()));
    fs::create_dir_all(&data_dir)?;
    let config_path = args.config.unwrap_or_else(|| data_dir.join("config.toml"));
    let config = AppConfig::load_or_create(config_path, &args.instance)?;
    let database_file = PathBuf::from(&config.database_file);
    let mut store = Store::open_with_database(&data_dir, database_file)?;
    if args.demo || args.demo_sale || store.catalog_count()? == 0 {
        store.seed_demo_catalog()?;
    }
    append_log(&data_dir, &format!("Inicio de instancia {}", args.instance))?;
    let branding = pos_print::TicketBranding {
        logo_text: config.store_logo_text.clone(),
        commercial_name: config.commercial_name.clone(),
        address: config.store_address.clone(),
        phone: config.store_phone.clone(),
    };
    let ticket_writer =
        TicketWriter::new(data_dir.join("comprobantes")).with_branding(branding.clone());
    for pending in store.pending_ticket_sales()? {
        match ticket_writer.write(&pending) {
            Ok(_) => {
                store.mark_ticket_complete(&pending.id)?;
                append_log(
                    &data_dir,
                    &format!("Ticket recuperado para venta {}", pending.id),
                )?;
            }
            Err(error) => {
                append_log(
                    &data_dir,
                    &format!("Ticket pendiente {}; se reintentara: {error}", pending.id),
                )?;
                eprintln!(
                    "Aviso: no se pudo recuperar el ticket {} ({error}). La venta sigue guardada y la caja continuara.",
                    pending.id
                );
            }
        }
    }

    if args.demo_sale {
        let product = store
            .product_by_code("DEMO-001")?
            .ok_or("No se encontró DEMO-001 en catálogo demo")?;
        let cart = Cart::new().add_product(product, 1)?;
        let sale = Sale::from_cart(&args.instance, cart)?;
        store.save_sale(&sale)?;
        let files = ticket_writer.write(&sale)?;
        store.mark_ticket_complete(&sale.id)?;
        append_log(
            &data_dir,
            &format!(
                "Venta demo {} por {} minor-units",
                sale.id, sale.total_minor
            ),
        )?;
        println!("Venta demo registrada en {}", args.instance);
        println!("SQLite: {}", data_dir.join(&config.database_file).display());
        println!("TXT: {}", files.txt.display());
        println!("PDF: {}", files.pdf.display());
        return Ok(());
    }

    pos_terminal::run(&args.instance, &mut store, &data_dir, &config)?;
    append_log(&data_dir, &format!("Cierre de instancia {}", args.instance))?;
    Ok(())
}

fn validate_instance(instance: &str) -> Result<(), Box<dyn std::error::Error>> {
    let valid = !instance.is_empty()
        && instance.len() <= 32
        && instance
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if !valid {
        return Err(
            "El ID de caja debe tener 1-32 caracteres ASCII: letras, números, guion o guion bajo"
                .into(),
        );
    }
    Ok(())
}

fn append_log(data_dir: &Path, message: &str) -> std::io::Result<()> {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(data_dir.join("logs").join("minipos.log"))?;
    writeln!(file, "{} {}", chrono::Utc::now().to_rfc3339(), message)
}

#[cfg(test)]
mod tests {
    use super::validate_instance;
    #[test]
    fn instance_ids_are_path_safe_and_bounded() {
        assert!(validate_instance("CAJA_01-A").is_ok());
        assert!(validate_instance("../CAJA").is_err());
        assert!(validate_instance(&"x".repeat(33)).is_err());
    }
}
