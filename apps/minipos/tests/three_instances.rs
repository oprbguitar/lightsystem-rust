use pos_core::{Cart, Product, Sale};
use pos_storage::Store;
use rusqlite::Connection;
use std::process::Command;

#[test]
fn three_demo_instances_record_sales_in_separate_databases_and_print_tickets() {
    let root = tempfile::tempdir().unwrap();
    let executable = env!("CARGO_BIN_EXE_minipos");
    let mut children = Vec::new();
    for id in ["CAJA01", "CAJA02", "CAJA03"] {
        let data = root.path().join(id.to_lowercase());
        children.push(
            Command::new(executable)
                .args(["--instance", id, "--data-dir"])
                .arg(&data)
                .args(["--demo", "--demo-sale"])
                .spawn()
                .unwrap(),
        );
    }
    for mut child in children {
        assert!(child.wait().unwrap().success());
    }
    let dirs = std::fs::read_dir(root.path())
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(dirs.len(), 3);
    for entry in dirs {
        let instance = entry.file_name().to_string_lossy().to_uppercase();
        let database = entry.path().join("minipos.sqlite3");
        assert!(database.is_file());
        let connection = Connection::open(database).unwrap();
        let (sales, sale_instance): (i64, String) = connection
            .query_row("SELECT COUNT(*), MIN(instance_id) FROM sales", [], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .unwrap();
        assert_eq!(sales, 1);
        assert_eq!(sale_instance, instance);
        let status: String = connection
            .query_row("SELECT status FROM ticket_jobs", [], |row| row.get(0))
            .unwrap();
        assert_eq!(status, "complete");
        let receipts = entry.path().join("comprobantes");
        let sale_dirs = std::fs::read_dir(receipts)
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(sale_dirs.len(), 1);
        assert!(sale_dirs[0].path().join("ticket.txt").exists());
        assert!(sale_dirs[0].path().join("ticket.pdf").exists());
        assert!(sale_dirs[0].path().join("ticket.escpos").exists());
        let metadata: serde_json::Value = serde_json::from_slice(
            &std::fs::read(sale_dirs[0].path().join("metadata.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(metadata["instance_id"], instance);
        assert_eq!(
            metadata["id"],
            sale_dirs[0].file_name().to_string_lossy().as_ref()
        );
    }
}

#[test]
fn startup_recovers_a_committed_sale_whose_ticket_was_not_written() {
    let root = tempfile::tempdir().unwrap();
    let data = root.path().join("caja01");
    {
        let mut store = Store::open(&data).unwrap();
        store.seed_demo_catalog().unwrap();
        let sale = Sale::from_cart(
            "CAJA01",
            Cart::new()
                .add_product(Product::new("DEMO-001", "Cafe", 1250).unwrap(), 1)
                .unwrap(),
        )
        .unwrap();
        store.save_sale(&sale).unwrap();
    }
    let status = Command::new(env!("CARGO_BIN_EXE_minipos"))
        .args(["--instance", "CAJA01", "--data-dir"])
        .arg(&data)
        .args(["--demo-sale"])
        .status()
        .unwrap();
    assert!(status.success());
    let connection = Connection::open(data.join("minipos.sqlite3")).unwrap();
    let (sales, pending): (i64, i64) = connection.query_row(
        "SELECT (SELECT COUNT(*) FROM sales),(SELECT COUNT(*) FROM ticket_jobs WHERE status='pending')",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    ).unwrap();
    assert_eq!(sales, 2);
    assert_eq!(pending, 0);
    assert_eq!(
        std::fs::read_dir(data.join("comprobantes"))
            .unwrap()
            .count(),
        2
    );
}
