use pos_core::{Cart, Sale};
use pos_storage::{StorageError, Store};

#[test]
fn demo_seed_imports_over_one_thousand_unique_computer_products_with_price_provenance() {
    let root = tempfile::tempdir().unwrap();
    let store = Store::open(root.path()).unwrap();

    let seeded = store.seed_demo_catalog().unwrap();
    assert!(seeded >= 1_000);
    assert_eq!(store.catalog_count().unwrap(), seeded + 3);

    let product = store.product_by_code("0-761345-10029-8").unwrap().unwrap();
    assert_eq!(product.category, "CASES");
    assert_eq!(product.brand, "ANTEC");
    assert!(product.stock_quantity > 0);
    assert_eq!(product.listed_stock_quantity, Some(1));
    assert!(product.price_usd_minor.unwrap() > 0);
    assert_eq!(product.price_exchange_rate_milli, Some(3_448));
    assert_eq!(product.price_checked_on.as_deref(), Some("2026-10-13"));
    assert!(product
        .price_source_url
        .as_deref()
        .unwrap()
        .contains("impacto.com.pe/producto/"));
    assert!(product.specifications.contains("ATX"));
}

#[test]
fn local_demo_inventory_decrements_transactionally_and_rejects_overselling() {
    let root = tempfile::tempdir().unwrap();
    let mut store = Store::open(root.path()).unwrap();
    store.seed_demo_catalog().unwrap();

    let product = store.product_by_code("0-761345-10029-8").unwrap().unwrap();
    let initial_stock = product.stock_quantity;
    let first_sale =
        Sale::from_cart("CAJA01", Cart::new().add_product(product, 1).unwrap()).unwrap();
    store.save_sale(&first_sale).unwrap();
    assert_eq!(
        store
            .product_by_code("0-761345-10029-8")
            .unwrap()
            .unwrap()
            .stock_quantity,
        initial_stock - 1
    );

    let remaining = store.product_by_code("0-761345-10029-8").unwrap().unwrap();
    let emptying_sale = Sale::from_cart(
        "CAJA01",
        Cart::new()
            .add_product(
                remaining.clone(),
                u32::try_from(remaining.stock_quantity).unwrap(),
            )
            .unwrap(),
    )
    .unwrap();
    store.save_sale(&emptying_sale).unwrap();
    let rejected = Sale::from_cart(
        "CAJA01",
        Cart::new()
            .add_product(
                store.product_by_code("0-761345-10029-8").unwrap().unwrap(),
                1,
            )
            .unwrap(),
    )
    .unwrap();
    assert!(matches!(
        store.save_sale(&rejected),
        Err(StorageError::InsufficientStock(code)) if code == "0-761345-10029-8"
    ));
    assert_eq!(store.sales().unwrap().len(), 2);
}

#[test]
fn demo_seed_is_repeatable_without_duplicate_catalog_rows() {
    let root = tempfile::tempdir().unwrap();
    let mut store = Store::open(root.path()).unwrap();
    let first_count = store.seed_demo_catalog().unwrap();
    let product = store.product_by_code("0-761345-10029-8").unwrap().unwrap();
    let initial_stock = product.stock_quantity;
    let sale = Sale::from_cart("CAJA01", Cart::new().add_product(product, 1).unwrap()).unwrap();
    store.save_sale(&sale).unwrap();
    let second_count = store.seed_demo_catalog().unwrap();
    assert_eq!(first_count, second_count);
    assert_eq!(store.catalog_count().unwrap(), second_count + 3);
    assert_eq!(
        store
            .product_by_code("0-761345-10029-8")
            .unwrap()
            .unwrap()
            .stock_quantity,
        initial_stock - 1
    );
}
