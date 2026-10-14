use pos_core::{Cart, Product, Sale};
use pos_storage::Store;

#[test]
fn each_data_directory_keeps_its_own_sales_history() {
    let root = tempfile::tempdir().unwrap();
    let mut first = Store::open(root.path().join("caja01")).unwrap();
    let second = Store::open(root.path().join("caja02")).unwrap();
    let sale = Sale::from_cart(
        "CAJA01",
        Cart::new()
            .add_product(Product::new("DEMO-001", "Cafe", 1250).unwrap(), 1)
            .unwrap(),
    )
    .unwrap();
    first.save_sale(&sale).unwrap();
    assert_eq!(first.sales().unwrap().len(), 1);
    assert!(second.sales().unwrap().is_empty());
}

#[test]
fn prevents_two_process_locks_for_the_same_database_directory() {
    let root = tempfile::tempdir().unwrap();
    let first = Store::open(root.path().join("caja01")).unwrap();
    assert!(Store::open(root.path().join("caja01")).is_err());
    drop(first);
    assert!(Store::open(root.path().join("caja01")).is_ok());
}

#[test]
fn saved_draft_survives_reopening_the_same_instance() {
    let root = tempfile::tempdir().unwrap();
    let cart = Cart::new()
        .add_product(Product::new("DEMO-001", "Cafe", 1250).unwrap(), 2)
        .unwrap();
    {
        let store = Store::open(root.path().join("caja01")).unwrap();
        store.save_draft("CAJA01", &cart).unwrap();
    }
    let reopened = Store::open(root.path().join("caja01")).unwrap();
    assert_eq!(reopened.load_draft("CAJA01").unwrap(), cart);
}

#[test]
fn searches_demo_catalog_by_name_and_code() {
    let root = tempfile::tempdir().unwrap();
    let store = Store::open(root.path().join("caja01")).unwrap();
    store.seed_demo_catalog().unwrap();
    assert_eq!(store.search_products("Cafe").unwrap()[0].code, "DEMO-001");
    assert_eq!(store.search_products("003").unwrap()[0].code, "DEMO-003");
}

#[test]
fn sale_and_pending_ticket_job_commit_together_and_clear_its_draft() {
    let root = tempfile::tempdir().unwrap();
    let mut store = Store::open(root.path().join("caja01")).unwrap();
    let cart = Cart::new()
        .add_product(Product::new("DEMO-001", "Cafe", 1250).unwrap(), 1)
        .unwrap();
    let sale = Sale::from_cart("CAJA01", cart.clone()).unwrap();
    store.save_draft("CAJA01", &cart).unwrap();
    store.save_sale(&sale).unwrap();
    assert!(store.load_draft("CAJA01").unwrap().is_empty());
    assert_eq!(store.pending_ticket_sales().unwrap(), vec![sale.clone()]);
    store.mark_ticket_complete(&sale.id).unwrap();
    assert!(store.pending_ticket_sales().unwrap().is_empty());
}

#[test]
fn rejects_database_paths_that_can_escape_the_instance_directory() {
    let root = tempfile::tempdir().unwrap();
    let data = root.path().join("caja01");
    assert!(Store::open_with_database(&data, r"..\shared.sqlite3").is_err());
    assert!(Store::open_with_database(&data, root.path().join("shared.sqlite3")).is_err());
}
