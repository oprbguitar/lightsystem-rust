# MiniPOS portable para Windows

Extrae el ZIP en una carpeta local y abre PowerShell en esa carpeta. No se requiere instalador, administrador, Python ni servicio local.

Una base vacía carga el catálogo demo automáticamente, incluso si abres `minipos.exe` directamente desde el Explorador. `--demo` también agrega productos que falten; conserva los cambios de nombre, precio y stock guardados en esa base. Puedes personalizar `store_logo_text`, `commercial_name`, `store_address` y `store_phone` en `datos\caja01\config.toml`. El logo de la terminal es textual.

```powershell
.\minipos.exe --instance CAJA01 --data-dir .\datos\caja01 --demo
```

Al abrir entra en Caja, con la lista de productos y el carrito visibles. Escribe para filtrar; usa ↑/↓ para elegir y ENTER para agregar varios productos. TAB selecciona líneas del carrito; F9/F10 cambia cantidades y SUPR quita una línea. F8 abre el cobro demo; completa el cliente/documento y el medio de pago, y presiona F8 para confirmar y generar un PDF imprimible. F4 abre Almacén; F1 registra un producto y F2 edita el seleccionado. F12 regresa a Caja y F3 cierra guardando el borrador. Para abrir tres terminales independientes, ejecuta:

La caja consulta el tamaño actual de la consola y redibuja al cambiarlo. Al ensancharla aparecen el catálogo y el carrito lado a lado; al aumentar la altura se muestran más filas por página. El archivo de configuración solo se usa como respaldo si la terminal no proporciona sus dimensiones.

```powershell
.\scripts\start-three-boxes.ps1
```

El catálogo de demostración contiene 1.040 partes, componentes y periféricos con sus fichas técnicas y precios publicados consultados el 13/10/2026. Escribe para filtrar por SKU, producto, marca, modelo o categoría; usa ↑/↓ para elegir y ENTER para agregar. El precio se muestra en soles como conversión de referencia desde USD y junto al precio fuente. Es una instantánea, no una cotización actualizada al ejecutar. El `stock` inicial es simulado para pruebas y se descuenta al vender; el stock publicado por el proveedor aparece aparte. `CATALOGO-DEMO.md` documenta procedencia, tipo de cambio y criterios del conjunto. El archivo JSON incluido bajo `catalogo/` permite inspeccionar las filas sin acceder a la red.

La carpeta `datos` contiene SQLite, configuración, registro local y comprobantes. Caja y Almacén comparten esta base local por caja; no hay conexión con SUNAT, RENIEC, pagos ni servidores remotos. DNI/RUC se valida por formato, sin consulta externa. El IGV y el tipo de cambio de cobro son referenciales y los PDF declaran que son demos sin validez tributaria.

El paquete contiene un binario firmado solo si la distribución que recibiste incluye una firma verificable; esta entrega local no está firmada. Windows 11 x64 se ejecutó en el host de compilación. El ejecutable x86 se compiló y corrió bajo WoW64 en ese mismo host; Windows 10, Windows 10 x86 y máquinas virtuales no están validados. No se declara soporte para Windows 7 o XP.

