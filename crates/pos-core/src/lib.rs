use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DomainError {
    #[error("El código y el nombre del producto son obligatorios")]
    InvalidProduct,
    #[error("El precio no puede ser negativo")]
    InvalidPrice,
    #[error("La cantidad debe ser mayor que cero")]
    InvalidQuantity,
    #[error("El producto {0} no está en el carrito")]
    ProductNotInCart(String),
    #[error("El carrito está vacío")]
    EmptyCart,
    #[error("El total excede el rango permitido")]
    TotalOverflow,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Product {
    pub code: String,
    pub name: String,
    pub unit_price_minor: i64,
    pub category: String,
    pub subcategory: String,
    pub brand: String,
    pub model: String,
    pub specifications: String,
    pub stock_quantity: i64,
    pub listed_stock_quantity: Option<i64>,
    pub price_usd_minor: Option<i64>,
    pub price_exchange_rate_milli: Option<i64>,
    pub price_source: String,
    pub price_source_url: Option<String>,
    pub price_checked_on: Option<String>,
}

impl Product {
    pub fn new(
        code: impl Into<String>,
        name: impl Into<String>,
        unit_price_minor: i64,
    ) -> Result<Self, DomainError> {
        let (code, name) = (code.into(), name.into());
        if code.trim().is_empty() || name.trim().is_empty() {
            return Err(DomainError::InvalidProduct);
        }
        if unit_price_minor < 0 {
            return Err(DomainError::InvalidPrice);
        }
        Ok(Self {
            code,
            name,
            unit_price_minor,
            category: String::new(),
            subcategory: String::new(),
            brand: String::new(),
            model: String::new(),
            specifications: String::new(),
            stock_quantity: 0,
            listed_stock_quantity: None,
            price_usd_minor: None,
            price_exchange_rate_milli: None,
            price_source: String::new(),
            price_source_url: None,
            price_checked_on: None,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SaleLine {
    pub product_code: String,
    pub name: String,
    pub quantity: u32,
    pub unit_price_minor: i64,
    pub total_minor: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Cart {
    lines: Vec<SaleLine>,
}

impl Cart {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn lines(&self) -> &[SaleLine] {
        &self.lines
    }
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }
    pub fn add_product(mut self, product: Product, quantity: u32) -> Result<Self, DomainError> {
        if quantity == 0 {
            return Err(DomainError::InvalidQuantity);
        }
        let total = product
            .unit_price_minor
            .checked_mul(i64::from(quantity))
            .ok_or(DomainError::TotalOverflow)?;
        if let Some(line) = self
            .lines
            .iter_mut()
            .find(|l| l.product_code == product.code)
        {
            line.quantity = line
                .quantity
                .checked_add(quantity)
                .ok_or(DomainError::TotalOverflow)?;
            line.total_minor = line
                .total_minor
                .checked_add(total)
                .ok_or(DomainError::TotalOverflow)?;
        } else {
            self.lines.push(SaleLine {
                product_code: product.code,
                name: product.name,
                quantity,
                unit_price_minor: product.unit_price_minor,
                total_minor: total,
            });
        }
        self.lines
            .iter()
            .try_fold(0_i64, |sum, line| sum.checked_add(line.total_minor))
            .ok_or(DomainError::TotalOverflow)?;
        Ok(self)
    }

    /// Replace a cart line's quantity while retaining its product and unit price.
    pub fn set_quantity(mut self, product_code: &str, quantity: u32) -> Result<Self, DomainError> {
        if quantity == 0 {
            return Err(DomainError::InvalidQuantity);
        }
        let line = self
            .lines
            .iter_mut()
            .find(|line| line.product_code == product_code)
            .ok_or_else(|| DomainError::ProductNotInCart(product_code.to_owned()))?;
        let total_minor = line
            .unit_price_minor
            .checked_mul(i64::from(quantity))
            .ok_or(DomainError::TotalOverflow)?;
        line.quantity = quantity;
        line.total_minor = total_minor;
        self.lines
            .iter()
            .try_fold(0_i64, |sum, line| sum.checked_add(line.total_minor))
            .ok_or(DomainError::TotalOverflow)?;
        Ok(self)
    }

    /// Remove a cart line by product code.
    pub fn remove_product(mut self, product_code: &str) -> Result<Self, DomainError> {
        let Some(index) = self
            .lines
            .iter()
            .position(|line| line.product_code == product_code)
        else {
            return Err(DomainError::ProductNotInCart(product_code.to_owned()));
        };
        self.lines.remove(index);
        Ok(self)
    }

    pub fn total_minor(&self) -> i64 {
        self.lines.iter().map(|l| l.total_minor).sum()
    }
}

/// Optional checkout metadata captured when a simulated sale is finalized.
/// Monetary amounts use the same minor-unit convention as product and sale prices.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct CheckoutDetails {
    pub document_kind: String,
    pub document_number: String,
    pub customer_document_kind: String,
    pub customer_document_number: String,
    pub customer_name: String,
    pub payment_method: String,
    pub amount_received_minor: i64,
    pub change_minor: i64,
    /// Exchange rate scaled by 1,000 (for example, 3.75 is stored as 3,750).
    pub fx_rate_milli: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Sale {
    pub id: String,
    pub instance_id: String,
    pub created_at: DateTime<Utc>,
    pub lines: Vec<SaleLine>,
    pub total_minor: i64,
    pub payment_method: String,
    /// Missing on sales serialized before checkout metadata was introduced.
    #[serde(default)]
    pub checkout: Option<CheckoutDetails>,
}

impl Sale {
    pub fn from_cart(instance_id: impl Into<String>, cart: Cart) -> Result<Self, DomainError> {
        if cart.is_empty() {
            return Err(DomainError::EmptyCart);
        }
        let total_minor = cart
            .lines
            .iter()
            .try_fold(0_i64, |sum, line| sum.checked_add(line.total_minor))
            .ok_or(DomainError::TotalOverflow)?;
        Ok(Self {
            id: Uuid::new_v4().to_string(),
            instance_id: instance_id.into(),
            created_at: Utc::now(),
            lines: cart.lines,
            total_minor,
            payment_method: "Efectivo (simulado)".to_owned(),
            checkout: None,
        })
    }

    /// Calculate the taxable base for a total that already includes Peru's 18% IGV.
    /// Rounds each line to the nearest minor unit so the printed line tax and sale
    /// summary reconcile exactly.
    pub fn taxable_base_minor(&self) -> Result<i64, DomainError> {
        self.lines.iter().try_fold(0_i64, |sum, line| {
            let base = i128::from(line.total_minor)
                .checked_mul(100)
                .and_then(|amount| amount.checked_add(59))
                .and_then(|amount| i64::try_from(amount / 118).ok())
                .ok_or(DomainError::TotalOverflow)?;
            sum.checked_add(base).ok_or(DomainError::TotalOverflow)
        })
    }

    pub fn igv_minor(&self) -> Result<i64, DomainError> {
        self.total_minor
            .checked_sub(self.taxable_base_minor()?)
            .ok_or(DomainError::TotalOverflow)
    }
}

pub fn format_money(minor: i64) -> String {
    format!("S/ {}.{:02}", minor / 100, minor.abs() % 100)
}
