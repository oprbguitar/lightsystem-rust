use fs2::FileExt;
use pos_core::{Cart, CheckoutDetails, Product, Sale};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::{
    fs::{self, File, OpenOptions},
    path::{Path, PathBuf},
    time::Duration,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("No se pudo preparar el directorio de datos: {0}")]
    Io(#[from] std::io::Error),
    #[error("No se pudo acceder a SQLite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("La base de datos ya está abierta por otro proceso: {0}")]
    Locked(PathBuf),
    #[error(
        "database_file debe ser un nombre de archivo local dentro del directorio de caja: {0}"
    )]
    InvalidDatabasePath(PathBuf),
    #[error("Inventario insuficiente para el producto {0}")]
    InsufficientStock(String),
    #[error("El catálogo de prueba no es válido: {0}")]
    InvalidDemoCatalog(String),
    #[error("El producto no es válido: {0}")]
    InvalidProduct(String),
}

pub struct Store {
    connection: Connection,
    lock: File,
    data_dir: PathBuf,
}

impl Store {
    pub fn open(data_dir: impl AsRef<Path>) -> Result<Self, StorageError> {
        Self::open_with_database(data_dir, "minipos.sqlite3")
    }

    pub fn open_with_database(
        data_dir: impl AsRef<Path>,
        database_file: impl AsRef<Path>,
    ) -> Result<Self, StorageError> {
        let data_dir = data_dir.as_ref().to_path_buf();
        fs::create_dir_all(&data_dir)?;
        for child in ["comprobantes", "cola-impresion", "logs"] {
            fs::create_dir_all(data_dir.join(child))?;
        }
        let lock_path = data_dir.join("minipos.lock");
        let lock = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(&lock_path)?;
        if lock.try_lock_exclusive().is_err() {
            return Err(StorageError::Locked(data_dir));
        }
        let database_file = database_file.as_ref();
        let file_name = database_file.to_string_lossy();
        let safe_local_name = !database_file.is_absolute()
            && !file_name.contains('/')
            && !file_name.contains('\\')
            && database_file.components().count() == 1
            && database_file.file_name().is_some();
        if !safe_local_name {
            return Err(StorageError::InvalidDatabasePath(
                database_file.to_path_buf(),
            ));
        }
        let database_path = data_dir.join(database_file);
        let connection = Connection::open(database_path)?;
        connection.busy_timeout(Duration::from_secs(5))?;
        connection.execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;
            CREATE TABLE IF NOT EXISTS products (code TEXT PRIMARY KEY, name TEXT NOT NULL, unit_price_minor INTEGER NOT NULL CHECK(unit_price_minor >= 0));
            CREATE TABLE IF NOT EXISTS product_inventory (product_code TEXT PRIMARY KEY REFERENCES products(code) ON DELETE CASCADE, category TEXT NOT NULL DEFAULT '', subcategory TEXT NOT NULL DEFAULT '', brand TEXT NOT NULL DEFAULT '', model TEXT NOT NULL DEFAULT '', specifications TEXT NOT NULL DEFAULT '', stock_quantity INTEGER NOT NULL DEFAULT 0 CHECK(stock_quantity >= 0), listed_stock_quantity INTEGER, price_usd_minor INTEGER, price_exchange_rate_milli INTEGER, price_source TEXT NOT NULL DEFAULT '', price_source_url TEXT, price_checked_on TEXT);
            CREATE TABLE IF NOT EXISTS sales (id TEXT PRIMARY KEY, instance_id TEXT NOT NULL, created_at TEXT NOT NULL, total_minor INTEGER NOT NULL, payment_method TEXT NOT NULL, details_json TEXT NOT NULL DEFAULT 'null');
            CREATE TABLE IF NOT EXISTS sale_lines (sale_id TEXT NOT NULL REFERENCES sales(id) ON DELETE CASCADE, product_code TEXT NOT NULL, name TEXT NOT NULL, quantity INTEGER NOT NULL CHECK(quantity > 0), unit_price_minor INTEGER NOT NULL, total_minor INTEGER NOT NULL);
            CREATE TABLE IF NOT EXISTS drafts (instance_id TEXT PRIMARY KEY, cart_json TEXT NOT NULL, saved_at TEXT NOT NULL);")?;
        ensure_sale_details_column(&connection)?;
        connection.execute_batch("CREATE TABLE IF NOT EXISTS ticket_jobs (sale_id TEXT PRIMARY KEY REFERENCES sales(id) ON DELETE CASCADE, status TEXT NOT NULL CHECK(status IN ('pending','complete')) DEFAULT 'pending');")?;
        Ok(Self {
            connection,
            lock,
            data_dir,
        })
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    pub fn seed_demo_catalog(&self) -> Result<usize, StorageError> {
        let snapshot: DemoCatalogSnapshot = serde_json::from_str(include_str!(
            "../data/demo_catalog_impacto_2026-10-13-partes.json"
        ))
        .map_err(|error| StorageError::InvalidDemoCatalog(error.to_string()))?;
        if snapshot.items.len() < 1_000 || snapshot.usd_pen_reference_rate_milli <= 0 {
            return Err(StorageError::InvalidDemoCatalog(
                "se requieren al menos 1.000 productos y un tipo de cambio válido".into(),
            ));
        }
        let tx = self.connection.unchecked_transaction()?;
        for product in demo_catalog() {
            tx.execute(
                "INSERT OR IGNORE INTO products(code,name,unit_price_minor) VALUES(?1,?2,?3)",
                params![product.code, product.name, product.unit_price_minor],
            )?;
            upsert_inventory(
                &tx,
                &product,
                InventorySeed {
                    stock_quantity: 25,
                    listed_stock_quantity: None,
                    price_usd_minor: None,
                    exchange_rate_milli: None,
                    price_source: "MiniPOS demo",
                    price_source_url: None,
                    price_checked_on: None,
                },
            )?;
        }
        let imported = snapshot.items.len();
        for item in snapshot.items {
            if item.code.trim().is_empty()
                || item.name.trim().is_empty()
                || item.price_usd_minor <= 0
                || item.listed_stock < 0
            {
                return Err(StorageError::InvalidDemoCatalog(format!(
                    "producto inválido: {}",
                    item.code
                )));
            }
            let price_pen_minor = i128::from(item.price_usd_minor)
                .checked_mul(i128::from(snapshot.usd_pen_reference_rate_milli))
                .and_then(|value| value.checked_add(500))
                .map(|value| value / 1_000)
                .and_then(|value| i64::try_from(value).ok())
                .ok_or_else(|| {
                    StorageError::InvalidDemoCatalog(format!(
                        "precio fuera de rango: {}",
                        item.code
                    ))
                })?;
            tx.execute(
                "INSERT OR IGNORE INTO products(code,name,unit_price_minor) VALUES(?1,?2,?3)",
                params![item.code, item.name, price_pen_minor],
            )?;
            let specs = item
                .specifications
                .iter()
                .map(|(key, value)| format!("{key}: {value}"))
                .collect::<Vec<_>>()
                .join(" · ");
            tx.execute(
                "INSERT OR IGNORE INTO product_inventory(product_code,category,subcategory,brand,model,specifications,stock_quantity,listed_stock_quantity,price_usd_minor,price_exchange_rate_milli,price_source,price_source_url,price_checked_on)
                 VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",
                params![item.code, item.category, item.subcategory, item.brand, item.model, specs, demo_stock_for_code(&item.code), item.listed_stock, item.price_usd_minor, snapshot.usd_pen_reference_rate_milli, snapshot.retailer, item.price_source_url, snapshot.snapshot_on],
            )?;
        }
        tx.commit()?;
        Ok(imported)
    }

    pub fn catalog_count(&self) -> Result<usize, StorageError> {
        let count: i64 = self
            .connection
            .query_row("SELECT COUNT(*) FROM products", [], |row| row.get(0))?;
        usize::try_from(count)
            .map_err(|_| StorageError::InvalidDemoCatalog("conteo fuera de rango".into()))
    }

    pub fn product_by_code(&self, code: &str) -> Result<Option<Product>, StorageError> {
        Ok(self
            .connection
            .query_row(
                "SELECT p.code,p.name,p.unit_price_minor,COALESCE(i.category,''),COALESCE(i.subcategory,''),COALESCE(i.brand,''),COALESCE(i.model,''),COALESCE(i.specifications,''),COALESCE(i.stock_quantity,0),i.listed_stock_quantity,i.price_usd_minor,i.price_exchange_rate_milli,COALESCE(i.price_source,''),i.price_source_url,i.price_checked_on FROM products p LEFT JOIN product_inventory i ON i.product_code=p.code WHERE p.code=?1",
                [code],
                map_product,
            )
            .optional()?)
    }

    pub fn search_products(&self, query: &str) -> Result<Vec<Product>, StorageError> {
        let pattern = format!("%{}%", query.replace('%', "\\%").replace('_', "\\_"));
        let mut stmt = self.connection.prepare("SELECT p.code,p.name,p.unit_price_minor,COALESCE(i.category,''),COALESCE(i.subcategory,''),COALESCE(i.brand,''),COALESCE(i.model,''),COALESCE(i.specifications,''),COALESCE(i.stock_quantity,0),i.listed_stock_quantity,i.price_usd_minor,i.price_exchange_rate_milli,COALESCE(i.price_source,''),i.price_source_url,i.price_checked_on FROM products p LEFT JOIN product_inventory i ON i.product_code=p.code WHERE p.code LIKE ?1 ESCAPE '\\' OR p.name LIKE ?1 ESCAPE '\\' OR i.brand LIKE ?1 ESCAPE '\\' OR i.model LIKE ?1 ESCAPE '\\' OR i.category LIKE ?1 ESCAPE '\\' OR i.subcategory LIKE ?1 ESCAPE '\\' ORDER BY CASE WHEN p.code = ?2 COLLATE NOCASE THEN 0 WHEN p.code LIKE ?3 ESCAPE '\\' THEN 1 WHEN p.name LIKE ?3 ESCAPE '\\' THEN 2 ELSE 3 END, p.name COLLATE NOCASE LIMIT 5000")?;
        let exact = query.trim();
        let prefix = format!("{}%", exact.replace('%', "\\%").replace('_', "\\_"));
        let rows = stmt.query_map(params![pattern, exact, prefix], map_product)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StorageError::from)
    }

    pub fn save_product(&mut self, product: &Product) -> Result<(), StorageError> {
        if product.code.trim().is_empty()
            || product.code.chars().count() > 64
            || product.code.chars().any(char::is_control)
            || product.name.trim().is_empty()
            || product.name.chars().count() > 160
            || product.unit_price_minor < 0
            || product.stock_quantity < 0
        {
            return Err(StorageError::InvalidProduct(
                "revisa código, nombre, precio y stock".into(),
            ));
        }
        let previous_price = self
            .product_by_code(product.code.trim())?
            .map(|existing| existing.unit_price_minor);
        let tx = self.connection.transaction()?;
        tx.execute(
            "INSERT INTO products(code,name,unit_price_minor) VALUES(?1,?2,?3)
             ON CONFLICT(code) DO UPDATE SET name=excluded.name,unit_price_minor=excluded.unit_price_minor",
            params![product.code.trim(), product.name.trim(), product.unit_price_minor],
        )?;
        tx.execute(
            "INSERT INTO product_inventory(product_code,category,subcategory,brand,model,specifications,stock_quantity,price_source)
             VALUES(?1,?2,?3,?4,?5,?6,?7,'Registro local')
             ON CONFLICT(product_code) DO UPDATE SET category=excluded.category,subcategory=excluded.subcategory,brand=excluded.brand,model=excluded.model,specifications=excluded.specifications,stock_quantity=excluded.stock_quantity",
            params![product.code.trim(), product.category, product.subcategory, product.brand, product.model, product.specifications, product.stock_quantity],
        )?;
        if previous_price.is_some_and(|previous| previous != product.unit_price_minor) {
            tx.execute(
                "UPDATE product_inventory SET price_source='Precio editado localmente',price_source_url=NULL,price_checked_on=NULL,price_usd_minor=NULL,price_exchange_rate_milli=NULL,listed_stock_quantity=NULL WHERE product_code=?1",
                [product.code.trim()],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn save_sale(&mut self, sale: &Sale) -> Result<(), StorageError> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let details_json = serde_json::to_string(&sale.checkout)
            .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
        tx.execute("INSERT INTO sales(id,instance_id,created_at,total_minor,payment_method,details_json) VALUES(?1,?2,?3,?4,?5,?6)", params![sale.id, sale.instance_id, sale.created_at.to_rfc3339(), sale.total_minor, sale.payment_method, details_json])?;
        for line in &sale.lines {
            let quantity = i64::from(line.quantity);
            let changed = tx.execute(
                "UPDATE product_inventory SET stock_quantity=stock_quantity-?1 WHERE product_code=?2 AND stock_quantity>=?1",
                params![quantity, line.product_code],
            )?;
            if changed == 0 {
                let tracked: bool = tx.query_row(
                    "SELECT EXISTS(SELECT 1 FROM product_inventory WHERE product_code=?1)",
                    [&line.product_code],
                    |row| row.get(0),
                )?;
                if tracked {
                    return Err(StorageError::InsufficientStock(line.product_code.clone()));
                }
            }
            tx.execute("INSERT INTO sale_lines(sale_id,product_code,name,quantity,unit_price_minor,total_minor) VALUES(?1,?2,?3,?4,?5,?6)", params![sale.id, line.product_code, line.name, line.quantity, line.unit_price_minor, line.total_minor])?;
        }
        tx.execute(
            "INSERT INTO ticket_jobs(sale_id,status) VALUES(?1,'pending')",
            [&sale.id],
        )?;
        tx.execute(
            "DELETE FROM drafts WHERE instance_id=?1",
            [&sale.instance_id],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn pending_ticket_sales(&self) -> Result<Vec<Sale>, StorageError> {
        let mut header = self.connection.prepare("SELECT s.id,s.instance_id,s.created_at,s.total_minor,s.payment_method,s.details_json FROM sales s JOIN ticket_jobs j ON j.sale_id=s.id WHERE j.status='pending' ORDER BY s.created_at")?;
        let headers = header.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
            ))
        })?;
        let mut sales = Vec::new();
        for header in headers {
            let (id, instance_id, created_at, total_minor, payment_method, details_json) = header?;
            let created_at = chrono::DateTime::parse_from_rfc3339(&created_at)
                .map(|value| value.with_timezone(&chrono::Utc))
                .map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        2,
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })?;
            let mut lines_stmt = self.connection.prepare("SELECT product_code,name,quantity,unit_price_minor,total_minor FROM sale_lines WHERE sale_id=?1 ORDER BY rowid")?;
            let lines = lines_stmt.query_map([&id], |row| {
                Ok(pos_core::SaleLine {
                    product_code: row.get(0)?,
                    name: row.get(1)?,
                    quantity: row.get(2)?,
                    unit_price_minor: row.get(3)?,
                    total_minor: row.get(4)?,
                })
            })?;
            let checkout = serde_json::from_str::<Option<CheckoutDetails>>(&details_json).map_err(
                |error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        5,
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                },
            )?;
            sales.push(Sale {
                id,
                instance_id,
                created_at,
                lines: lines.collect::<Result<Vec<_>, _>>()?,
                total_minor,
                payment_method,
                checkout,
            });
        }
        Ok(sales)
    }

    pub fn mark_ticket_complete(&self, sale_id: &str) -> Result<(), StorageError> {
        self.connection.execute(
            "UPDATE ticket_jobs SET status='complete' WHERE sale_id=?1",
            [sale_id],
        )?;
        Ok(())
    }

    pub fn save_draft(&self, instance_id: &str, cart: &Cart) -> Result<(), StorageError> {
        let json = serde_json::to_string(cart)
            .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
        self.connection.execute("INSERT INTO drafts(instance_id,cart_json,saved_at) VALUES(?1,?2,datetime('now')) ON CONFLICT(instance_id) DO UPDATE SET cart_json=excluded.cart_json,saved_at=excluded.saved_at", params![instance_id, json])?;
        Ok(())
    }

    pub fn load_draft(&self, instance_id: &str) -> Result<Cart, StorageError> {
        let json: Option<String> = self
            .connection
            .query_row(
                "SELECT cart_json FROM drafts WHERE instance_id=?1",
                [instance_id],
                |row| row.get(0),
            )
            .optional()?;
        json.map(|value| {
            serde_json::from_str(&value).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    Box::new(error),
                )
            })
        })
        .transpose()?
        .map_or(Ok(Cart::new()), Ok)
    }

    pub fn sales(&self) -> Result<Vec<SaleSummary>, StorageError> {
        let mut stmt = self.connection.prepare("SELECT id,instance_id,created_at,total_minor,payment_method FROM sales ORDER BY created_at DESC")?;
        let rows = stmt.query_map([], |row| {
            Ok(SaleSummary {
                id: row.get(0)?,
                instance_id: row.get(1)?,
                created_at: row.get(2)?,
                total_minor: row.get(3)?,
                payment_method: row.get(4)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StorageError::from)
    }
}

fn ensure_sale_details_column(connection: &Connection) -> Result<(), rusqlite::Error> {
    let mut statement = connection.prepare("PRAGMA table_info(sales)")?;
    let columns = statement.query_map([], |row| row.get::<_, String>(1))?;
    let has_details = columns
        .collect::<Result<Vec<_>, _>>()?
        .iter()
        .any(|column| column == "details_json");
    if !has_details {
        connection.execute(
            "ALTER TABLE sales ADD COLUMN details_json TEXT NOT NULL DEFAULT 'null'",
            [],
        )?;
    }
    Ok(())
}

impl Drop for Store {
    fn drop(&mut self) {
        let _ = self.lock.unlock();
    }
}

#[derive(Debug, Clone)]
pub struct SaleSummary {
    pub id: String,
    pub instance_id: String,
    pub created_at: String,
    pub total_minor: i64,
    pub payment_method: String,
}

#[derive(Deserialize)]
struct DemoCatalogSnapshot {
    snapshot_on: String,
    retailer: String,
    #[allow(dead_code)]
    catalog_url: String,
    usd_pen_reference_rate_milli: i64,
    #[allow(dead_code)]
    fx_date: String,
    #[allow(dead_code)]
    fx_source_url: String,
    items: Vec<DemoCatalogItem>,
}

#[derive(Deserialize)]
struct DemoCatalogItem {
    code: String,
    name: String,
    category: String,
    subcategory: String,
    brand: String,
    model: String,
    specifications: BTreeMap<String, String>,
    listed_stock: i64,
    price_usd_minor: i64,
    price_source_url: String,
}

struct InventorySeed<'a> {
    stock_quantity: i64,
    listed_stock_quantity: Option<i64>,
    price_usd_minor: Option<i64>,
    exchange_rate_milli: Option<i64>,
    price_source: &'a str,
    price_source_url: Option<&'a str>,
    price_checked_on: Option<&'a str>,
}

fn upsert_inventory(
    connection: &Connection,
    product: &Product,
    inventory: InventorySeed<'_>,
) -> Result<(), rusqlite::Error> {
    connection.execute(
        "INSERT INTO product_inventory(product_code,category,subcategory,brand,model,specifications,stock_quantity,listed_stock_quantity,price_usd_minor,price_exchange_rate_milli,price_source,price_source_url,price_checked_on)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)
         ON CONFLICT(product_code) DO NOTHING",
        params![product.code, product.category, product.subcategory, product.brand, product.model, product.specifications, inventory.stock_quantity, inventory.listed_stock_quantity, inventory.price_usd_minor, inventory.exchange_rate_milli, inventory.price_source, inventory.price_source_url, inventory.price_checked_on],
    )?;
    Ok(())
}

fn map_product(row: &rusqlite::Row<'_>) -> Result<Product, rusqlite::Error> {
    Ok(Product {
        code: row.get(0)?,
        name: row.get(1)?,
        unit_price_minor: row.get(2)?,
        category: row.get(3)?,
        subcategory: row.get(4)?,
        brand: row.get(5)?,
        model: row.get(6)?,
        specifications: row.get(7)?,
        stock_quantity: row.get(8)?,
        listed_stock_quantity: row.get(9)?,
        price_usd_minor: row.get(10)?,
        price_exchange_rate_milli: row.get(11)?,
        price_source: row.get(12)?,
        price_source_url: row.get(13)?,
        price_checked_on: row.get(14)?,
    })
}

fn demo_stock_for_code(code: &str) -> i64 {
    let hash = code.bytes().fold(2_166_136_261_u32, |hash, byte| {
        (hash ^ u32::from(byte)).wrapping_mul(16_777_619)
    });
    i64::from(hash % 40 + 5)
}

pub fn demo_catalog() -> Vec<Product> {
    [
        ("DEMO-001", "Cafe molido 250 g", 1250),
        ("DEMO-002", "Agua sin gas 600 ml", 300),
        ("DEMO-003", "Galletas de avena", 550),
    ]
    .into_iter()
    .map(|(code, name, price)| Product::new(code, name, price).expect("catalog data is valid"))
    .collect()
}
