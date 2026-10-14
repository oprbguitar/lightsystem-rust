use pos_core::{Cart, Product};

#[test]
fn totals_are_calculated_in_minor_currency_units() {
    let product = Product::new("DEMO-001", "Cafe", 1250).unwrap();
    let cart = Cart::new().add_product(product, 3).unwrap();
    assert_eq!(cart.total_minor(), 3750);
}

#[test]
fn rejects_zero_quantity_and_empty_product_codes() {
    assert!(Product::new("", "Cafe", 1250).is_err());
    let product = Product::new("DEMO-001", "Cafe", 1250).unwrap();
    assert!(Cart::new().add_product(product, 0).is_err());
}

#[test]
fn merges_repeated_products_and_rejects_arithmetic_overflow() {
    let product = Product::new("DEMO-001", "Cafe", 1250).unwrap();
    let cart = Cart::new()
        .add_product(product.clone(), 1)
        .unwrap()
        .add_product(product, 2)
        .unwrap();
    assert_eq!(cart.lines().len(), 1);
    assert_eq!(cart.lines()[0].quantity, 3);
    assert!(Cart::new()
        .add_product(Product::new("BIG", "Grande", i64::MAX).unwrap(), 2)
        .is_err());
}
