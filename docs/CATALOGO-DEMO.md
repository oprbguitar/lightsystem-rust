# Catálogo local de demostración

## Contenido y uso

Al iniciar MiniPOS con `--demo`, o al iniciar por primera vez con una base vacía, el programa agrega los productos que falten en el SQLite de esa caja. La carga se puede repetir sin reemplazar nombres, precios, stock ni metadatos editados localmente. Incluye SKU, nombre comercial, categoría, subcategoría, marca, modelo, especificaciones disponibles, precio fuente, enlace del producto y disponibilidad que el proveedor mostraba al tomar la muestra. También crea una cantidad inicial simulada de entre 5 y 44 unidades para probar el inventario de la distribuidora; ese stock disminuye en la misma transacción que confirma la venta y bloquea ventas que excederían las existencias.

Al abrir una caja con catálogo, la lista aparece como pantalla inicial. Busque escribiendo SKU, nombre, categoría, marca o modelo; los resultados se filtran al instante. Use las flechas para recorrerlos y ENTER para añadir el producto enfocado. F1 registra productos propios; F2 modifica el producto seleccionado. Los cambios quedan en SQLite local. El panel presenta las especificaciones y distingue el precio publicado convertido a soles del precio fuente en dólares. F12 regresa a la venta.

## Procedencia y precios

- Proveedor del snapshot: [Impacto, catálogo de componentes](https://www.impacto.com.pe/componentes).
- Consulta del catálogo: **2026-10-13**.
- Moneda fuente: USD; se conserva el precio fuente en centavos de dólar.
- Referencia de conversión: **S/ 3,448 por USD** el 2026-10-13, tomada del tipo de cambio SUNAT/SBS publicado por [Cheqea](https://chequea.pe/tipo-de-cambio-sunat/).
- Precio mostrado en soles: conversión de referencia, redondeada al céntimo; no es una cotización emitida por el proveedor ni se actualiza al ejecutar el programa.
- `Stock` es una cantidad inicial simulada para pruebas, separada de la disponibilidad del proveedor.
- `Stock proveedor` es la disponibilidad que Impacto publicaba en el snapshot; no es stock de la distribuidora.

Se filtraron productos activos con precio y SKU único, excluyendo laptops, computadoras completas, combos, licencias, consolas, sillas, tabletas, proyectores, equipos de venta y la categoría miscelánea. El archivo incluye 1.040 SKU únicos. Los tres productos `DEMO-001` a `DEMO-003` se mantienen aparte para compatibilidad con el tutorial y los flujos automatizados existentes.

Para inspeccionar el archivo fuente sin abrirlo como una línea larga, use PowerShell:

```powershell
$catalogPath = if (Test-Path .\catalogo\catalogo-demo.json) { .\catalogo\catalogo-demo.json } else { .\crates\pos-storage\data\demo_catalog_impacto_2026-10-13-partes.json }
$catalog = Get-Content -Raw $catalogPath | ConvertFrom-Json
$catalog.items.Count
$catalog.items | Select-Object -First 10 code, name, brand, category, price_usd_minor, listed_stock
```

## Actualización

Este catálogo es una instantánea fechada incorporada al binario. Para una nueva muestra hay que volver a consultar el catálogo y el tipo de cambio, revisar las exclusiones y guardar la nueva fecha y enlaces de procedencia. No se debe presentar este snapshot como inventario ni precio actual en vivo.
