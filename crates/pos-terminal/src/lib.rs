use crossterm::{
    cursor::{Hide, MoveTo, Show},
    event::{self, Event, KeyCode, KeyEventKind},
    execute, queue,
    style::{Color, Print, ResetColor, SetBackgroundColor, SetForegroundColor},
    terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use pos_core::{format_money, Cart, CheckoutDetails, Product, Sale};
use pos_devices::AppConfig;
use pos_print::{render_text_with_branding, TicketBranding};
use pos_storage::Store;
use std::{
    io::{self, Write},
    path::Path,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TerminalError {
    #[error("Error de terminal: {0}")]
    Io(#[from] io::Error),
    #[error(transparent)]
    Storage(#[from] pos_storage::StorageError),
}

struct TerminalGuard;
impl TerminalGuard {
    fn enter() -> io::Result<Self> {
        terminal::enable_raw_mode()?;
        execute!(
            io::stdout(),
            EnterAlternateScreen,
            Hide,
            Clear(ClearType::All)
        )?;
        Ok(Self)
    }
}
impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = terminal::disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, Show, ResetColor);
    }
}

pub fn run(
    instance_id: &str,
    store: &mut Store,
    receipts_dir: &Path,
    config: &AppConfig,
) -> Result<(), TerminalError> {
    let _guard = TerminalGuard::enter()?;
    let mut state = ScreenState::new(store.load_draft(instance_id)?);
    state.search_results = store.search_products("")?;
    if !state.search_results.is_empty() {
        state.status = format!(
            "Caja lista · {} productos disponibles · escriba para buscar y ENTER agrega.",
            state.search_results.len()
        );
    }
    loop {
        draw(instance_id, &state, config)?;
        let Event::Key(key) = event::read()? else {
            continue;
        };
        if key.kind == KeyEventKind::Release {
            continue;
        }
        if handle_key_with_config(
            key.code,
            instance_id,
            store,
            receipts_dir,
            &mut state,
            config,
        )? {
            break;
        }
    }
    Ok(())
}

#[derive(Debug)]
struct ScreenState {
    cart: Cart,
    input: String,
    status: String,
    search_mode: bool,
    search_results: Vec<Product>,
    selected_result: usize,
    selected_cart_line: usize,
    product_form: Option<ProductForm>,
    checkout_form: Option<CheckoutForm>,
}

#[derive(Debug)]
struct ProductForm {
    fields: Vec<String>,
    field_index: usize,
    editing: bool,
    original: Option<Product>,
}

#[derive(Debug)]
struct CheckoutForm {
    document_kind: usize,
    customer_document_kind: usize,
    customer_document: String,
    customer_name: String,
    payment_kind: usize,
    amount_received: String,
    fx_rate: String,
    field_index: usize,
}

impl CheckoutForm {
    fn new(total_minor: i64) -> Self {
        Self {
            document_kind: 0,
            customer_document_kind: 0,
            customer_document: String::new(),
            customer_name: String::new(),
            payment_kind: 0,
            amount_received: format!("{}.{:02}", total_minor / 100, total_minor.abs() % 100),
            fx_rate: "3.448".into(),
            field_index: 0,
        }
    }
}

impl ScreenState {
    fn new(cart: Cart) -> Self {
        Self {
            cart,
            input: String::new(),
            status: "Listo. Escanee un código o escriba un código y presione ENTER.".into(),
            search_mode: false,
            search_results: Vec::new(),
            selected_result: 0,
            product_form: None,
            checkout_form: None,
            selected_cart_line: 0,
        }
    }
}

#[cfg(test)]
fn handle_key(
    key: KeyCode,
    instance_id: &str,
    store: &mut Store,
    receipts_dir: &Path,
    state: &mut ScreenState,
) -> Result<bool, TerminalError> {
    handle_key_with_config(
        key,
        instance_id,
        store,
        receipts_dir,
        state,
        &AppConfig::default(),
    )
}

fn handle_key_with_config(
    key: KeyCode,
    instance_id: &str,
    store: &mut Store,
    receipts_dir: &Path,
    state: &mut ScreenState,
    config: &AppConfig,
) -> Result<bool, TerminalError> {
    if state.checkout_form.is_some() {
        if key == KeyCode::F(8) {
            let form = state.checkout_form.take().expect("checkout is active");
            confirm_checkout(form, instance_id, store, receipts_dir, state, config)?;
            return Ok(false);
        }
        if key == KeyCode::Esc || key == KeyCode::F(12) {
            state.checkout_form = None;
            state.status = "Cobro cancelado; el carrito sigue abierto.".into();
            return Ok(false);
        }
        let form = state.checkout_form.as_mut().expect("checkout is active");
        match key {
            KeyCode::Tab | KeyCode::Down => form.field_index = (form.field_index + 1) % 7,
            KeyCode::Up => form.field_index = (form.field_index + 6) % 7,
            KeyCode::Left | KeyCode::Right => match form.field_index {
                0 => {
                    form.document_kind = cycle_option(form.document_kind, 2, key);
                    if form.document_kind == 1 {
                        form.customer_document_kind = 1;
                    }
                }
                1 => {
                    form.customer_document_kind = cycle_option(form.customer_document_kind, 2, key)
                }
                4 => form.payment_kind = cycle_option(form.payment_kind, 3, key),
                _ => {}
            },
            KeyCode::Backspace => match form.field_index {
                2 => {
                    form.customer_document.pop();
                }
                3 => {
                    form.customer_name.pop();
                }
                5 => {
                    form.amount_received.pop();
                }
                6 => {
                    form.fx_rate.pop();
                }
                _ => {}
            },
            KeyCode::Char(character) if !character.is_control() => match form.field_index {
                2 if form.customer_document.len() < 11 => form.customer_document.push(character),
                3 if form.customer_name.chars().count() < 80 => form.customer_name.push(character),
                5 if form.amount_received.len() < 16 => form.amount_received.push(character),
                6 if form.fx_rate.len() < 12 => form.fx_rate.push(character),
                _ => {}
            },
            _ => {}
        }
        return Ok(false);
    }
    if let Some(form) = state.product_form.as_mut() {
        match key {
            KeyCode::Esc | KeyCode::F(12) => {
                state.product_form = None;
                state.status = "Edición cancelada; el catálogo sigue abierto.".into();
            }
            KeyCode::Tab | KeyCode::Down => {
                let first = usize::from(form.editing);
                form.field_index =
                    first + (form.field_index + 1 - first) % (form.fields.len() - first);
            }
            KeyCode::Up => {
                let first = usize::from(form.editing);
                form.field_index = form
                    .field_index
                    .checked_sub(1)
                    .filter(|index| *index >= first)
                    .unwrap_or(form.fields.len() - 1);
            }
            KeyCode::Enter if form.field_index + 1 < form.fields.len() => {
                form.field_index += 1;
            }
            KeyCode::Enter | KeyCode::F(2) => {
                let form = state.product_form.take().expect("form is active");
                save_product_form(form, store, state)?;
            }
            KeyCode::Backspace => {
                form.fields[form.field_index].pop();
            }
            KeyCode::Char(character)
                if !character.is_control() && form.fields[form.field_index].len() < 160 =>
            {
                form.fields[form.field_index].push(character);
            }
            _ => {}
        }
        return Ok(false);
    }
    match key {
        KeyCode::F(3) => {
            if !state.cart.is_empty() {
                store.save_draft(instance_id, &state.cart)?;
            }
            return Ok(true);
        }
        KeyCode::F(4) => {
            state.search_mode = !state.search_mode;
            state.input.clear();
            state.search_results = if state.search_mode {
                store.search_products("")?
            } else {
                Vec::new()
            };
            state.selected_result = 0;
            state.status = if state.search_mode {
                "ALMACÉN · catálogo e inventario · buscar para filtrar.".into()
            } else {
                "CAJA · escriba para buscar, seleccione y agregue al carrito.".into()
            };
        }
        KeyCode::F(1) if state.search_mode => {
            state.product_form = Some(product_form(None));
            state.status = "Producto nuevo: completa los campos · TAB cambia · F2 guarda.".into();
        }
        KeyCode::F(2) if state.search_mode => {
            if let Some(product) = state.search_results.get(state.selected_result).cloned() {
                state.product_form = Some(product_form(Some(product)));
                state.status = "Producto: completa los campos · TAB/↑/↓ cambia · F2 guarda.".into();
            } else {
                state.status =
                    "Seleccione un producto para modificarlo; F1 registra uno nuevo.".into();
            }
        }
        KeyCode::F(5) => {
            store.save_draft(instance_id, &state.cart)?;
            state.status =
                "Carrito guardado localmente; se recuperará al volver a abrir esta caja.".into();
        }
        KeyCode::F(6) => {
            if state.cart.is_empty() {
                state.status = "No hay artículos para previsualizar.".into();
            } else if let Ok(sale) = Sale::from_cart(instance_id, state.cart.clone()) {
                let preview = receipts_dir.join("cola-impresion").join("vista-previa.txt");
                let branding = ticket_branding(config);
                std::fs::write(preview, render_text_with_branding(&sale, &branding))?;
                state.status =
                    "Vista previa TXT colocada en cola-impresion. Aún no se envió a una impresora."
                        .into();
            }
        }
        KeyCode::F(8) => {
            if state.search_mode {
                state.status = "F8 cobra desde CAJA; F12 regresa al módulo de venta.".into();
            } else if state.cart.is_empty() {
                state.status = "Agregue artículos antes de cobrar.".into();
            } else {
                state.checkout_form = Some(CheckoutForm::new(state.cart.total_minor()));
                state.status = "Cobro de demostración · seleccione campos y F8 confirma.".into();
            }
        }
        KeyCode::F(12) | KeyCode::Esc if state.search_mode => {
            state.search_mode = false;
            state.input.clear();
            state.search_results = store.search_products("")?;
            state.selected_result = 0;
            state.status = "Regreso a CAJA · búsqueda y carrito listos.".into();
        }
        KeyCode::Esc => {
            state.input.clear();
            refresh_catalog(store, state)?;
        }
        KeyCode::F(12) => {
            state.status = "No hay otra pantalla abierta.".into();
        }
        KeyCode::Enter => {
            if state.search_mode {
                if let Some(product) = state.search_results.get(state.selected_result) {
                    state.status = format!(
                        "Almacén · {} · stock tienda {} · {}.",
                        product.code,
                        product.stock_quantity,
                        format_money(product.unit_price_minor)
                    );
                } else {
                    state.status = "No hay coincidencias; ajuste la búsqueda.".into();
                }
            } else {
                let query = state.input.trim().to_owned();
                if !query.is_empty() {
                    if let Some(product) = store.product_by_code(&query)? {
                        add_to_cart(state, product)?;
                        return Ok(false);
                    }
                }
                if let Some(product) = state.search_results.get(state.selected_result).cloned() {
                    add_to_cart(state, product)?;
                } else {
                    state.status = "No hay coincidencias; ajuste la búsqueda.".into();
                }
            }
        }
        KeyCode::Backspace => {
            state.input.pop();
            refresh_catalog(store, state)?;
        }
        KeyCode::Tab if !state.cart.is_empty() => {
            state.selected_cart_line = (state.selected_cart_line + 1) % state.cart.lines().len();
            state.status =
                "Línea del carrito seleccionada · F9/F10 cantidad · SUPR elimina.".into();
        }
        KeyCode::F(9) if !state.cart.is_empty() => {
            adjust_selected_cart_line(state, 1)?;
        }
        KeyCode::F(10) if !state.cart.is_empty() => {
            adjust_selected_cart_line(state, -1)?;
        }
        KeyCode::Delete if !state.cart.is_empty() => {
            let code = state.cart.lines()[state.selected_cart_line]
                .product_code
                .clone();
            state.cart = state
                .cart
                .clone()
                .remove_product(&code)
                .map_err(|error| io::Error::other(error.to_string()))?;
            state.selected_cart_line = state
                .selected_cart_line
                .min(state.cart.lines().len().saturating_sub(1));
            state.status = format!("Producto {code} retirado del carrito.");
        }
        KeyCode::Up if !state.search_results.is_empty() => {
            state.selected_result = state
                .selected_result
                .checked_sub(1)
                .unwrap_or(state.search_results.len() - 1);
            state.status = format_product_status(&state.search_results[state.selected_result]);
        }
        KeyCode::Down if !state.search_results.is_empty() => {
            state.selected_result = (state.selected_result + 1) % state.search_results.len();
            state.status = format_product_status(&state.search_results[state.selected_result]);
        }
        KeyCode::Char(character) if !character.is_control() && state.input.len() < 40 => {
            state.input.push(character);
            refresh_catalog(store, state)?;
        }
        _ => {}
    }
    Ok(false)
}

fn refresh_catalog(store: &Store, state: &mut ScreenState) -> Result<(), TerminalError> {
    state.search_results = store.search_products(&state.input)?;
    state.selected_result = 0;
    state.status = if state.search_results.is_empty() {
        "No hay coincidencias; pruebe otro SKU, nombre, marca o categoría.".into()
    } else if state.input.is_empty() {
        format!(
            "Catálogo completo · {} productos.",
            state.search_results.len()
        )
    } else {
        format!(
            "{} resultados · ↑/↓ elige · ENTER agrega.",
            state.search_results.len()
        )
    };
    Ok(())
}

fn adjust_selected_cart_line(state: &mut ScreenState, delta: i32) -> Result<(), TerminalError> {
    let Some(line) = state.cart.lines().get(state.selected_cart_line) else {
        return Ok(());
    };
    let code = line.product_code.clone();
    let quantity = if delta > 0 {
        line.quantity.saturating_add(delta as u32)
    } else {
        line.quantity.saturating_sub(delta.unsigned_abs())
    };
    if quantity == 0 {
        state.cart = state
            .cart
            .clone()
            .remove_product(&code)
            .map_err(|error| io::Error::other(error.to_string()))?;
        state.selected_cart_line = state
            .selected_cart_line
            .min(state.cart.lines().len().saturating_sub(1));
        state.status = format!("{code} retirado del carrito al reducir a cero.");
    } else {
        state.cart = state
            .cart
            .clone()
            .set_quantity(&code, quantity)
            .map_err(|error| io::Error::other(error.to_string()))?;
        state.status = format!("{code}: cantidad {quantity}.");
    }
    Ok(())
}

fn confirm_checkout(
    form: CheckoutForm,
    instance_id: &str,
    store: &mut Store,
    receipts_dir: &Path,
    state: &mut ScreenState,
    config: &AppConfig,
) -> Result<(), TerminalError> {
    let customer_document_kind = if form.customer_document_kind == 0 {
        "DNI"
    } else {
        "RUC"
    };
    let expected_digits = if customer_document_kind == "DNI" {
        8
    } else {
        11
    };
    let document = form.customer_document.trim();
    let document_kind = if form.document_kind == 0 {
        "BOLETA DEMO"
    } else {
        "FACTURA DEMO"
    };
    if (!document.is_empty()
        && (document.len() != expected_digits
            || !document.chars().all(|digit| digit.is_ascii_digit())))
        || (form.document_kind == 1 && (customer_document_kind != "RUC" || document.is_empty()))
        || (form.document_kind == 1 && form.customer_name.trim().is_empty())
    {
        let invoice = form.document_kind == 1;
        state.checkout_form = Some(form);
        state.status = if invoice {
            "Factura demo requiere RUC de 11 dígitos y razón social.".into()
        } else {
            format!("El {customer_document_kind} debe contener {expected_digits} dígitos.")
        };
        return Ok(());
    }
    let Some(amount_received_minor) = parse_price_minor(&form.amount_received) else {
        state.checkout_form = Some(form);
        state.status = "Monto recibido inválido; use soles y hasta dos decimales.".into();
        return Ok(());
    };
    let Some(fx_rate_milli) = parse_scaled_milli(&form.fx_rate) else {
        state.checkout_form = Some(form);
        state.status = "Tipo de cambio inválido; por ejemplo 3.448 soles por dólar.".into();
        return Ok(());
    };
    let payment_method =
        ["Efectivo", "Tarjeta simulada", "Yape/Plin simulado"][form.payment_kind].to_owned();
    let total_minor = state.cart.total_minor();
    if amount_received_minor < total_minor {
        state.checkout_form = Some(form);
        state.status = "El monto recibido no cubre el total de la venta.".into();
        return Ok(());
    }
    if form.payment_kind != 0 && amount_received_minor != total_minor {
        state.checkout_form = Some(form);
        state.status = "Para tarjeta/Yape simulado, recibido debe igualar el total.".into();
        return Ok(());
    }
    let mut sale = Sale::from_cart(instance_id, state.cart.clone())
        .map_err(|error| io::Error::other(error.to_string()))?;
    sale.payment_method = payment_method.clone();
    sale.checkout = Some(CheckoutDetails {
        document_kind: document_kind.into(),
        document_number: format!("{}-{}", instance_id, &sale.id[..8]),
        customer_document_kind: customer_document_kind.into(),
        customer_document_number: document.into(),
        customer_name: form.customer_name.trim().to_owned(),
        payment_method,
        amount_received_minor,
        change_minor: amount_received_minor - total_minor,
        fx_rate_milli,
    });
    match store.save_sale(&sale) {
        Ok(()) => {}
        Err(pos_storage::StorageError::InsufficientStock(code)) => {
            state.checkout_form = Some(form);
            state.status = format!("Stock insuficiente para {code}; el cobro sigue abierto.");
            return Ok(());
        }
        Err(error) => return Err(error.into()),
    }
    state.cart = Cart::new();
    state.selected_cart_line = 0;
    state.search_results = store.search_products(&state.input)?;
    match pos_print::TicketWriter::new(receipts_dir.join("comprobantes"))
        .with_branding(ticket_branding(config))
        .write(&sale)
    {
        Ok(files) => {
            store.mark_ticket_complete(&sale.id)?;
            state.status = format!("Venta demo guardada · PDF local: {}", files.pdf.display());
        }
        Err(error) => {
            state.status = format!(
                "Venta {} confirmada; PDF pendiente de recuperación: {}",
                sale.id, error
            );
        }
    }
    Ok(())
}

fn parse_scaled_milli(value: &str) -> Option<i64> {
    let normalized = value.trim().replace(',', ".");
    let mut parts = normalized.split('.');
    let whole = parts.next()?.parse::<i64>().ok()?;
    let fraction = parts.next().unwrap_or("");
    if parts.next().is_some()
        || whole <= 0
        || fraction.len() > 3
        || !fraction.chars().all(|character| character.is_ascii_digit())
    {
        return None;
    }
    let padded = format!("{fraction:0<3}");
    whole
        .checked_mul(1_000)?
        .checked_add(padded.parse::<i64>().ok()?)
}

fn cycle_option(index: usize, length: usize, key: KeyCode) -> usize {
    if key == KeyCode::Left {
        (index + length - 1) % length
    } else {
        (index + 1) % length
    }
}

fn split_included_igv(total_minor: i64) -> (i64, i64) {
    let base = (i128::from(total_minor) * 100 + 59) / 118;
    let base = i64::try_from(base).unwrap_or(total_minor);
    (base, total_minor.saturating_sub(base))
}

fn split_cart_igv(cart: &Cart) -> (i64, i64) {
    let base = cart.lines().iter().fold(0_i64, |sum, line| {
        sum.saturating_add(split_included_igv(line.total_minor).0)
    });
    (base, cart.total_minor().saturating_sub(base))
}

fn ticket_branding(config: &AppConfig) -> TicketBranding {
    TicketBranding {
        logo_text: config.store_logo_text.clone(),
        commercial_name: config.commercial_name.clone(),
        address: config.store_address.clone(),
        phone: config.store_phone.clone(),
    }
}

fn product_form(product: Option<Product>) -> ProductForm {
    let editing = product.is_some();
    let fields = product.as_ref().map_or_else(
        || {
            vec![
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                "0.00".into(),
                "0".into(),
            ]
        },
        |product| {
            vec![
                product.code.clone(),
                product.name.clone(),
                product.category.clone(),
                product.brand.clone(),
                product.specifications.clone(),
                format!(
                    "{}.{:02}",
                    product.unit_price_minor / 100,
                    product.unit_price_minor.abs() % 100
                ),
                product.stock_quantity.to_string(),
            ]
        },
    );
    ProductForm {
        fields,
        field_index: usize::from(editing),
        editing,
        original: product,
    }
}

fn save_product_form(
    form: ProductForm,
    store: &mut Store,
    state: &mut ScreenState,
) -> Result<(), TerminalError> {
    let [code, name, category, brand, specifications, price, stock] = form.fields.as_slice() else {
        unreachable!("product form has seven fields")
    };
    let Some(unit_price_minor) = parse_price_minor(price) else {
        state.product_form = Some(form);
        state.status =
            "Precio inválido: use soles con hasta dos decimales, por ejemplo 19.90.".into();
        return Ok(());
    };
    let Ok(stock_quantity) = stock.trim().parse::<i64>() else {
        state.product_form = Some(form);
        state.status = "Stock inválido: ingrese un entero igual o mayor que cero.".into();
        return Ok(());
    };
    if stock_quantity < 0
        || code.trim().chars().count() > 64
        || code.chars().any(char::is_control)
        || name.trim().is_empty()
        || name.trim().chars().count() > 160
    {
        state.product_form = Some(form);
        state.status = "Revise SKU (máximo 64), nombre (1 a 160) y stock (cero o más).".into();
        return Ok(());
    }
    if !form.editing && store.product_by_code(code.trim())?.is_some() {
        state.product_form = Some(form);
        state.status = "Ese código ya existe; use F2 sobre el producto para modificarlo.".into();
        return Ok(());
    }
    let mut product = match Product::new(code.trim(), name.trim(), unit_price_minor) {
        Ok(product) => product,
        Err(error) => {
            state.product_form = Some(form);
            state.status = error.to_string();
            return Ok(());
        }
    };
    product.category = category.trim().to_owned();
    product.brand = brand.trim().to_owned();
    product.specifications = specifications.trim().to_owned();
    product.stock_quantity = stock_quantity;
    if let Some(original) = form.original.as_ref() {
        product.subcategory = original.subcategory.clone();
        product.model = original.model.clone();
        if specifications.trim().is_empty() {
            product.specifications = original.specifications.clone();
        }
    }
    store.save_product(&product)?;
    state.search_mode = true;
    state.input.clear();
    state.search_results = store.search_products("")?;
    state.selected_result = state
        .search_results
        .iter()
        .position(|item| item.code == product.code)
        .unwrap_or(0);
    state.status = format!(
        "Producto {} {} correctamente.",
        product.code,
        if form.editing {
            "actualizado"
        } else {
            "registrado"
        }
    );
    Ok(())
}

fn parse_price_minor(value: &str) -> Option<i64> {
    let normalized = value.trim().replace(',', ".");
    let mut parts = normalized.split('.');
    let whole = parts.next()?.parse::<i64>().ok()?;
    let fraction = parts.next().unwrap_or("0");
    if parts.next().is_some()
        || whole < 0
        || fraction.len() > 2
        || !fraction.chars().all(|c| c.is_ascii_digit())
    {
        return None;
    }
    let cents = match fraction.len() {
        0 => 0,
        1 => fraction.parse::<i64>().ok()? * 10,
        _ => fraction.parse::<i64>().ok()?,
    };
    whole.checked_mul(100)?.checked_add(cents)
}

fn add_to_cart(state: &mut ScreenState, product: Product) -> Result<(), TerminalError> {
    let product_code = product.code.clone();
    state.cart = state
        .cart
        .clone()
        .add_product(product.clone(), 1)
        .map_err(|error| io::Error::other(error.to_string()))?;
    state.selected_cart_line = state
        .cart
        .lines()
        .iter()
        .position(|line| line.product_code == product_code)
        .unwrap_or(0);
    state.status = format!(
        "Agregado: {} · {}",
        product.name,
        format_money(product.unit_price_minor)
    );
    Ok(())
}

fn format_product_status(product: &Product) -> String {
    format!(
        "{} {} · stock prueba {} · prov. {} · {} {}",
        product.brand,
        product.model,
        product.stock_quantity,
        product
            .listed_stock_quantity
            .map_or_else(|| "s/d".into(), |stock| stock.to_string()),
        product.price_source,
        product.price_checked_on.as_deref().unwrap_or("")
    )
}

fn draw(instance: &str, state: &ScreenState, config: &AppConfig) -> io::Result<()> {
    let interface = &config.interface;
    let (terminal_width, terminal_height) =
        terminal::size().unwrap_or((interface.columns, interface.rows));
    let width = usize::from(terminal_width.clamp(40, 240));
    let height = usize::from(terminal_height.clamp(15, 120));
    let rows = build_rows_with_brand(
        instance,
        state,
        width,
        height,
        &config.store_logo_text,
        &config.commercial_name,
    );
    let mut stdout = io::stdout();
    queue!(
        stdout,
        MoveTo(0, 0),
        Clear(ClearType::All),
        SetBackgroundColor(color(&interface.background_color)),
        SetForegroundColor(color(&interface.text_color))
    )?;
    for (idx, row) in rows.iter().take(height).enumerate() {
        if idx == 0 {
            queue!(
                stdout,
                SetBackgroundColor(color(&interface.header_color)),
                SetForegroundColor(Color::White)
            )?;
        } else {
            let foreground = if row.ends_with('\u{0}') {
                Color::White
            } else if state.search_mode && idx == 5 {
                Color::Cyan
            } else {
                color(&interface.text_color)
            };
            queue!(
                stdout,
                SetBackgroundColor(if row.ends_with('\u{0}') {
                    Color::DarkGreen
                } else {
                    color(&interface.background_color)
                }),
                SetForegroundColor(if idx == rows.len() - 1 {
                    Color::Yellow
                } else {
                    foreground
                })
            )?;
        }
        queue!(
            stdout,
            MoveTo(0, idx as u16),
            Print(fit(row.trim_end_matches('\u{0}'), width))
        )?;
    }
    queue!(stdout, ResetColor)?;
    stdout.flush()
}

#[cfg(test)]
fn build_rows(instance: &str, state: &ScreenState, width: usize) -> Vec<String> {
    build_rows_with_brand(instance, state, width, 25, "[ MINIPOS ]", "MiniPOS Demo")
}

fn build_rows_with_brand(
    instance: &str,
    state: &ScreenState,
    width: usize,
    height: usize,
    logo_text: &str,
    commercial_name: &str,
) -> Vec<String> {
    let mut rows = vec![
        format!(
            " {} {}  |  {}  |  {} ",
            truncate(logo_text, 16),
            truncate(commercial_name, 25),
            instance,
            if state.checkout_form.is_some() {
                "COBRO DEMO"
            } else if state.search_mode {
                "ALMACÉN · CATÁLOGO"
            } else {
                "CAJA · VENTA"
            }
        ),
        format!(
            " {:<12} {:<36} {:>8} {:>8} ",
            "CÓDIGO", "DESCRIPCIÓN", "CANT.", "IMPORTE"
        ),
        "-".repeat(width),
    ];
    if let Some(form) = state.checkout_form.as_ref() {
        rows.push(" COBRO · SIMULACIÓN LOCAL · SIN CONEXIÓN SUNAT".into());
        rows.push(" Cambie opciones con ←/→; TAB/↑/↓ cambia de campo; F8 confirma.".into());
        rows.push(String::new());
        let fields = [
            format!(
                "TIPO DE DOCUMENTO: {}",
                ["BOLETA DEMO", "FACTURA DEMO"][form.document_kind]
            ),
            format!(
                "CLIENTE IDENTIFICADO COMO: {}",
                ["DNI", "RUC"][form.customer_document_kind]
            ),
            format!("NÚMERO: {}", form.customer_document),
            format!("NOMBRE / RAZÓN SOCIAL: {}", form.customer_name),
            format!(
                "PAGO: {}",
                ["EFECTIVO", "TARJETA (DEMO)", "YAPE/PLIN (DEMO)"][form.payment_kind]
            ),
            format!("RECIBIDO PEN: {}", form.amount_received),
            format!("TIPO DE CAMBIO PEN/USD: {}", form.fx_rate),
        ];
        for (index, value) in fields.iter().enumerate() {
            rows.push(format!(
                " {}{}",
                if index == form.field_index {
                    "> "
                } else {
                    "  "
                },
                value
            ));
        }
        let total = state.cart.total_minor();
        let (base, igv) = split_cart_igv(&state.cart);
        rows.extend([
            "-".repeat(width),
            format!(
                " BASE REFERENCIAL: {}   IGV DEMO 18%: {}",
                format_money(base),
                format_money(igv)
            ),
            format!(" TOTAL DE VENTA: {}", format_money(total)),
            " El PDF dirá DEMO / SIN VALIDEZ TRIBUTARIA.".into(),
        ]);
    } else if let Some(form) = state.product_form.as_ref() {
        rows.push(" PRODUCTO · REGISTRAR O MODIFICAR".into());
        rows.push(" Use TAB o ↑/↓ para cambiar de campo; ENTER avanza y F2 guarda.".into());
        rows.push(" En edición, el código SKU se conserva para no afectar las ventas.".into());
        rows.push(String::new());
        for (index, (label, value)) in [
            "SKU",
            "NOMBRE",
            "CATEGORÍA",
            "MARCA",
            "FICHA / COLOR",
            "PRECIO PEN",
            "STOCK",
        ]
        .iter()
        .zip(&form.fields)
        .enumerate()
        {
            rows.push(format!(
                " {}{:<14} {}",
                if index == form.field_index { ">" } else { " " },
                label,
                value
            ));
        }
        rows.extend([
            String::new(),
            " EJEMPLO DE PRECIO: 19.90 · stock: entero desde cero".into(),
        ]);
    } else if state.search_mode {
        let page_size = height.saturating_sub(16).clamp(9, 40);
        let page_count = state.search_results.len().div_ceil(page_size);
        let page = if page_count == 0 {
            0
        } else {
            state.selected_result / page_size + 1
        };
        rows.push(format!(
            " CATÁLOGO · {} PRODUCTOS · PÁG {page}/{page_count} · ↑/↓ RECORRE",
            state.search_results.len()
        ));
        rows.push("-".repeat(width));
        rows.push(format!(
            " {:<14} {:<43} {:>6} {:>9}",
            "SKU", "PRODUCTO / MARCA", "STOCK", "PRECIO"
        ));
        let page_start = (state.selected_result / page_size) * page_size;
        for (index, product) in state
            .search_results
            .iter()
            .enumerate()
            .skip(page_start)
            .take(page_size)
        {
            rows.push(format!(
                " {:<14} {:<43} {:>6} {:>9}",
                truncate(&product.code, 14),
                truncate(&format!("{} {}", product.brand, product.name), 43),
                product.stock_quantity,
                format_money(product.unit_price_minor)
            ));
            if index == state.selected_result {
                rows.last_mut().expect("inserted row").push('\u{0}');
            }
        }
        while rows.len() < 14 {
            rows.push(String::new());
        }
        rows.push("-".repeat(width));
        if let Some(product) = state.search_results.get(state.selected_result) {
            rows.push(format!(
                " DETALLE {} · {} · {}",
                product.code, product.category, product.subcategory
            ));
            rows.push(truncate(&format!(" {}", product.specifications), width));
            if let Some(price_usd_minor) = product.price_usd_minor {
                rows.push(format!(
                    " {} · USD {}.{:02} · FX 3.448 · publicado {} · {}",
                    product.price_source,
                    price_usd_minor / 100,
                    price_usd_minor % 100,
                    product.price_checked_on.as_deref().unwrap_or("s/f"),
                    product.listed_stock_quantity.map_or_else(
                        || "sin stock fuente".into(),
                        |stock| format!("stock proveedor {stock}")
                    )
                ));
            } else {
                rows.push(format!(
                    " {} · stock de tienda {} · precio ingresado en PEN",
                    if product.price_source.is_empty() {
                        "Registro local"
                    } else {
                        &product.price_source
                    },
                    product.stock_quantity
                ));
            }
        } else {
            rows.extend([
                " Escriba un nombre, SKU, marca o categoría.".into(),
                String::new(),
                String::new(),
            ]);
        }
        rows.push(
            " Stock de prueba simulado; la venta lo descuenta. Stock fuente separado.".into(),
        );
    } else {
        append_register_rows(&mut rows, state, width, height);
    }
    if state.checkout_form.is_none() {
        rows.push(format!(" BUSCAR / SKU > {}", state.input));
    }
    rows.push(format!(" {}", state.status));
    rows.push("".into());
    rows.push(if state.product_form.is_some() {
        " F2 Guardar   TAB/↑/↓ Campo   ESC Cancelar".into()
    } else if state.checkout_form.is_some() {
        " F8 Confirmar demo   ←/→ Opción   TAB Campo   ESC Cancelar".into()
    } else if state.search_mode {
        " ALMACÉN   ENTER Ficha   F1 Nuevo   F2 Editar   F12 Caja   F3 Salir".into()
    } else {
        " ENTER agrega · TAB línea · F9/F10 cant. · SUPR quita · F4 Almacén · F8 cobra".into()
    });
    rows.push(format!(" {} ", truncate(commercial_name, width)));
    rows
}

fn append_register_rows(rows: &mut Vec<String>, state: &ScreenState, width: usize, height: usize) {
    let page_size = if width >= 108 {
        height.saturating_sub(12).clamp(4, 32)
    } else {
        height.saturating_sub(17).div_ceil(2).clamp(4, 20)
    };
    let page_start = (state.selected_result / page_size) * page_size;
    let cart_page_start = (state.selected_cart_line / page_size) * page_size;
    rows.push(format!(
        " {} · {} PRODUCTOS · ↑/↓ SELECCIONA · {}",
        if state.search_mode {
            "ALMACÉN"
        } else {
            "CAJA"
        },
        state.search_results.len(),
        if state.search_mode {
            "ENTER VER FICHA"
        } else {
            "ENTER AGREGA"
        }
    ));
    if state.search_mode {
        rows.last_mut()
            .expect("inserted list heading")
            .push_str(" · F1 NUEVO · F2 EDITAR");
    }
    if width >= 108 {
        let left_width = (width - 3) / 2;
        let right_width = width - left_width - 3;
        rows.push(format!(
            " {:<left_width$} │ {:<right_width$}",
            "CATÁLOGO / INVENTARIO",
            format!("VENTA · {} LÍNEAS", state.cart.lines().len())
        ));
        for offset in 0..page_size {
            let product = state.search_results.get(page_start + offset);
            let cart_index = cart_page_start + offset;
            let cart_line = state.cart.lines().get(cart_index);
            let left = product.map_or_else(String::new, |product| {
                format!(
                    "{} {} · stock {} · {}",
                    product.code,
                    truncate(&product.name, left_width.saturating_sub(40)),
                    product.stock_quantity,
                    format_money(product.unit_price_minor)
                )
            });
            let right = cart_line.map_or_else(String::new, |line| {
                format!(
                    "{} {} ×{} · {}",
                    if cart_index == state.selected_cart_line {
                        ">"
                    } else {
                        " "
                    },
                    truncate(&line.name, right_width.saturating_sub(28)),
                    line.quantity,
                    format_money(line.total_minor)
                )
            });
            rows.push(format!(
                "{}{} │ {:<right_width$}",
                if page_start + offset == state.selected_result {
                    ">"
                } else {
                    " "
                },
                truncate(&left, left_width.saturating_sub(1)),
                truncate(&right, right_width)
            ));
        }
    } else {
        for offset in 0..page_size {
            let Some(product) = state.search_results.get(page_start + offset) else {
                rows.push(String::new());
                continue;
            };
            rows.push(format!(
                "{} {:<12} {:<30} {:>5} {:>9}",
                if page_start + offset == state.selected_result {
                    ">"
                } else {
                    " "
                },
                truncate(&product.code, 12),
                truncate(&product.name, 30),
                product.stock_quantity,
                format_money(product.unit_price_minor)
            ));
            if page_start + offset == state.selected_result {
                rows.last_mut().expect("inserted row").push('\u{0}');
            }
        }
        if let Some(product) = state.search_results.get(state.selected_result) {
            rows.push(format!(
                " DETALLE: {} · {} · {}",
                product.code,
                truncate(&product.brand, 16),
                truncate(&product.category, width.saturating_sub(42))
            ));
            rows.push(truncate(
                &format!(" FICHA / COLOR: {}", product.specifications),
                width,
            ));
        } else {
            rows.push(" SIN RESULTADOS · pruebe por SKU, nombre o marca.".into());
            rows.push(String::new());
        }
        rows.push("-".repeat(width));
        rows.push(format!(
            " CARRITO DE VENTA · {} PRODUCTOS",
            state.cart.lines().len()
        ));
        for (index, line) in state
            .cart
            .lines()
            .iter()
            .enumerate()
            .skip(cart_page_start)
            .take(page_size)
        {
            rows.push(format!(
                "{} {:<12} {:<30} ×{:>3} {:>9}",
                if index == state.selected_cart_line {
                    ">"
                } else {
                    " "
                },
                truncate(&line.product_code, 12),
                truncate(&line.name, 30),
                line.quantity,
                format_money(line.total_minor)
            ));
        }
        while state.cart.lines().len().saturating_sub(cart_page_start) < page_size
            && rows.len() < 18
        {
            rows.push(String::new());
            if rows.len() >= 18 {
                break;
            }
        }
    }
    if width >= 108 {
        if let Some(product) = state.search_results.get(state.selected_result) {
            rows.push(truncate(
                &format!(
                    " DETALLE {} · {} · {} · {}",
                    product.code, product.brand, product.category, product.specifications
                ),
                width,
            ));
        }
    }
    let total = state.cart.total_minor();
    let (base, igv) = split_cart_igv(&state.cart);
    rows.push(format!(
        " BASE {} · IGV DEMO {} · TOTAL {}",
        format_money(base),
        format_money(igv),
        format_money(total)
    ));
}

fn color(value: &str) -> Color {
    match value.to_lowercase().as_str() {
        "black" => Color::Black,
        "yellow" => Color::Yellow,
        "white" => Color::White,
        "cyan" => Color::Cyan,
        "red" => Color::Red,
        "blue" => Color::Blue,
        "darkblue" => Color::DarkBlue,
        "darkgreen" => Color::DarkGreen,
        "green" => Color::Green,
        _ => Color::Green,
    }
}
fn truncate(value: &str, max: usize) -> String {
    value.chars().take(max).collect()
}
fn fit(value: &str, width: usize) -> String {
    let value = truncate(value, width);
    format!("{value:<width$}")
}

#[cfg(test)]
mod tests {
    use super::{build_rows, color, fit, handle_key, truncate, ScreenState};
    use crossterm::{event::KeyCode, style::Color};
    use pos_core::Cart;
    use pos_storage::Store;

    #[test]
    fn colors_and_fixed_column_helpers_match_the_terminal_design() {
        assert_eq!(color("black"), Color::Black);
        assert_eq!(color("blue"), Color::Blue);
        assert_eq!(color("green"), Color::Green);
        assert_eq!(color("yellow"), Color::Yellow);
        assert_eq!(color("unknown"), Color::Green);
        assert_eq!(fit("abc", 6), "abc   ");
        assert_eq!(truncate("abcdef", 4), "abcd");
    }

    #[test]
    fn keyboard_sales_flow_persists_drafts_and_prints_receipts() {
        let root = tempfile::tempdir().unwrap();
        let data = root.path().join("data");
        let mut store = Store::open(&data).unwrap();
        store.seed_demo_catalog().unwrap();
        let mut state = ScreenState::new(Cart::new());
        handle_key(KeyCode::F(6), "CAJA01", &mut store, &data, &mut state).unwrap();
        assert!(state.status.contains("No hay artículos"));
        handle_key(KeyCode::F(8), "CAJA01", &mut store, &data, &mut state).unwrap();
        assert!(state.status.contains("Agregue artículos"));
        handle_key(KeyCode::F(4), "CAJA01", &mut store, &data, &mut state).unwrap();
        assert!(state.search_mode);
        for character in "Cafe".chars() {
            handle_key(
                KeyCode::Char(character),
                "CAJA01",
                &mut store,
                &data,
                &mut state,
            )
            .unwrap();
        }
        handle_key(KeyCode::Enter, "CAJA01", &mut store, &data, &mut state).unwrap();
        assert_eq!(state.search_results.len(), 1);
        handle_key(KeyCode::Enter, "CAJA01", &mut store, &data, &mut state).unwrap();
        assert_eq!(state.cart.lines()[0].product_code, "DEMO-001");
        handle_key(KeyCode::F(6), "CAJA01", &mut store, &data, &mut state).unwrap();
        assert!(data.join("cola-impresion/vista-previa.txt").exists());
        handle_key(KeyCode::F(5), "CAJA01", &mut store, &data, &mut state).unwrap();
        assert_eq!(store.load_draft("CAJA01").unwrap(), state.cart);
        handle_key(KeyCode::F(8), "CAJA01", &mut store, &data, &mut state).unwrap();
        assert!(state.cart.is_empty());
        assert_eq!(store.sales().unwrap().len(), 1);
        assert!(store.pending_ticket_sales().unwrap().is_empty());
        assert_eq!(
            std::fs::read_dir(data.join("comprobantes"))
                .unwrap()
                .count(),
            1
        );
    }

    #[test]
    fn input_errors_multimatch_backspace_and_exit_are_recoverable() {
        let root = tempfile::tempdir().unwrap();
        let data = root.path().join("data");
        let mut store = Store::open(&data).unwrap();
        store.seed_demo_catalog().unwrap();
        let mut state = ScreenState::new(Cart::new());
        handle_key(KeyCode::Enter, "CAJA02", &mut store, &data, &mut state).unwrap();
        assert!(state.status.contains("Ingrese"));
        for character in "MISSX".chars() {
            handle_key(
                KeyCode::Char(character),
                "CAJA02",
                &mut store,
                &data,
                &mut state,
            )
            .unwrap();
        }
        handle_key(KeyCode::Backspace, "CAJA02", &mut store, &data, &mut state).unwrap();
        handle_key(KeyCode::Char('G'), "CAJA02", &mut store, &data, &mut state).unwrap();
        handle_key(KeyCode::Enter, "CAJA02", &mut store, &data, &mut state).unwrap();
        assert!(state.status.contains("no encontrado"));
        handle_key(KeyCode::F(4), "CAJA02", &mut store, &data, &mut state).unwrap();
        for character in "DEMO".chars() {
            handle_key(
                KeyCode::Char(character),
                "CAJA02",
                &mut store,
                &data,
                &mut state,
            )
            .unwrap();
        }
        handle_key(KeyCode::Enter, "CAJA02", &mut store, &data, &mut state).unwrap();
        assert!(state.search_results.len() > 1);
        assert!(state.status.contains("coincidencias"));
        handle_key(KeyCode::Enter, "CAJA02", &mut store, &data, &mut state).unwrap();
        assert_eq!(state.cart.lines().len(), 1);
        handle_key(KeyCode::F(12), "CAJA02", &mut store, &data, &mut state).unwrap();
        assert!(!state.search_mode);
        assert!(handle_key(KeyCode::F(3), "CAJA02", &mut store, &data, &mut state).unwrap());
    }

    #[test]
    fn catalog_search_filters_by_category_and_adds_the_selected_detailed_product() {
        let root = tempfile::tempdir().unwrap();
        let data = root.path().join("data");
        let mut store = Store::open(&data).unwrap();
        store.seed_demo_catalog().unwrap();
        let mut state = ScreenState::new(Cart::new());
        handle_key(KeyCode::F(4), "CAJA03", &mut store, &data, &mut state).unwrap();
        for character in "CASES".chars() {
            handle_key(
                KeyCode::Char(character),
                "CAJA03",
                &mut store,
                &data,
                &mut state,
            )
            .unwrap();
        }
        handle_key(KeyCode::Enter, "CAJA03", &mut store, &data, &mut state).unwrap();
        assert!(!state.search_results.is_empty());
        let selected_code = state.search_results[1].code.clone();
        handle_key(KeyCode::Down, "CAJA03", &mut store, &data, &mut state).unwrap();
        assert!(state.status.contains("Impacto"));
        handle_key(KeyCode::Enter, "CAJA03", &mut store, &data, &mut state).unwrap();
        assert_eq!(state.cart.lines()[0].product_code, selected_code);
    }

    #[test]
    fn rows_show_stock_provenance_price_details_and_selected_page() {
        let root = tempfile::tempdir().unwrap();
        let store = Store::open(root.path()).unwrap();
        store.seed_demo_catalog().unwrap();
        let mut state = ScreenState::new(Cart::new());
        let product = store.search_products("CASES").unwrap();
        assert!(product.len() > 10);
        state.search_mode = true;
        state.search_results = product;
        state.selected_result = 10;
        let rows = build_rows("CAJA01", &state, 80);
        assert!(rows[0].contains("BÚSQUEDA DE PRODUCTOS"));
        assert!(rows[3].contains("PÁG 2/6"));
        assert!(rows.iter().any(|row| row.contains("Impacto · USD")));
        assert!(rows
            .iter()
            .any(|row| row.contains("Stock de prueba simulado")));
        assert!(rows.iter().any(|row| row.ends_with('\u{0}')));
        assert!(rows[rows.len() - 2].contains("F4 Buscar"));

        state.search_mode = false;
        state.cart = Cart::new()
            .add_product(
                store.product_by_code("0-761345-10029-8").unwrap().unwrap(),
                1,
            )
            .unwrap();
        let sales_rows = build_rows("CAJA01", &state, 80);
        assert!(sales_rows.iter().any(|row| row.contains("TOTAL:")));
        assert!(sales_rows.iter().any(|row| row.contains("0-761345-10")));
    }

    #[test]
    fn insufficient_stock_keeps_the_sale_cart_open() {
        let root = tempfile::tempdir().unwrap();
        let data = root.path().join("data");
        let mut store = Store::open(&data).unwrap();
        store.seed_demo_catalog().unwrap();
        let mut state = ScreenState::new(Cart::new());
        let product = store.product_by_code("0-761345-10029-8").unwrap().unwrap();
        state.cart = Cart::new()
            .add_product(
                product.clone(),
                u32::try_from(product.stock_quantity).unwrap() + 1,
            )
            .unwrap();

        handle_key(KeyCode::F(8), "CAJA04", &mut store, &data, &mut state).unwrap();

        assert!(state.status.contains("Stock insuficiente"));
        assert!(!state.cart.is_empty());
        assert!(store.sales().unwrap().is_empty());
    }
}
