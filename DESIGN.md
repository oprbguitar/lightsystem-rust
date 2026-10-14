# Dirección de interfaz MiniPOS

## Superficie y usuarios

- Modo: `operate`, terminal local de caja para cajeros con operación frecuente por teclado.
- Tareas: operar una caja local, buscar y agregar varios productos, editar cantidades, identificar al cliente, simular pago y emitir una vista imprimible; administrar productos desde un módulo separado de almacén.
- Restricciones: consola Windows/Linux de 80×25 como base, con reflujo por ancho y alto actual, baja latencia, salida legible sin ratón y sin red.

## Dirección visual

- Arquetipo funcional: caja de dos zonas; búsqueda/lista de productos como lienzo operativo y carrito/cobro como inspector persistente. En terminales estrechas las zonas se apilan sin perder el flujo.
- Fondo negro; texto verde; encabezados azul/blanco; avisos en amarillo. La señal de color comunica rol y estado, nunca decoración.
- Tokens de pantalla: fondo `black`, panel superior `blue`, texto operativo `green`, etiquetas de tabla `cyan`, selección `darkgreen` con texto `white`, confirmaciones y avisos `yellow`. Columnas alineadas con una unidad monoespaciada y separadores ASCII.
- Caja: búsqueda incremental por SKU/nombre/marca/modelo/categoría; lista visible al iniciar, selección con ↑/↓ y ENTER agrega. El carrito admite varias líneas, cambios de cantidad y eliminación. F8 abre cobro; la confirmación genera documentos de demostración locales.
- Adaptación: cada redibujado consulta el tamaño real de Crossterm. En ancho de 108 columnas o más, productos y carrito comparten una vista de dos paneles; al aumentar la altura se amplían las filas visibles tanto en Caja como en Almacén. Por debajo de ese ancho la pantalla apila catálogo y carrito; los resultados se paginan usando el espacio vertical disponible.
- Almacén (F4): lista completa y filtros, stock de tienda diferenciado del stock fuente; F1 registra y F2 modifica. El módulo no confirma ventas.
- Marca: nombre de tienda y logotipo textual configurables en `config.toml`; el comprobante incluye también dirección y teléfono opcionales. La consola no muestra imágenes rasterizadas.
- Venta: tabla SKU/producto/cantidad/importe, total persistente y barra de teclas fija. La búsqueda y el carrito permanecen visibles durante la selección.
- Tipografía monoespaciada elegida por el usuario en la terminal anfitriona; la app no descarga fuentes ni cambia el tamaño global de la consola.
- Geometría cuadrada y líneas ASCII para compatibilidad entre consolas. Sin tarjetas, sombras, degradados ni movimiento.
- Motion: sin animaciones; cada actualización reemplaza la pantalla y conserva la ubicación de la entrada.

## Controles y accesibilidad

- F3 salir, F4 entrar/salir de almacén, F1 registrar producto, F2 editar/guardar, F5 guardar borrador, F6 vista previa, F8 abrir/confirmar cobro, F12 volver y ESC cancelar/regresar. F9/F10 ajustan cantidad y SUPR elimina la línea seleccionada.
- Los nombres y precios están alineados en columnas; se conserva texto junto a los colores.
- Crossterm permite que el anfitrión aplique ANSI. Si la consola no procesa colores, el texto sigue presente y las teclas no cambian.
- `columns` y `rows` actúan como respaldo si el emulador no informa su tamaño; en condiciones normales, el tamaño real de la consola define el área compuesta. Windows Terminal o la consola define fuente y tamaño real.

## Restricciones intencionales

- No aplicar el arquetipo ni la paleta web del selector: la TUI anfitriona impone celdas monoespaciadas, ANSI básico, teclado y terminales 80×25; la dirección IBM 5250 existente funciona en ese entorno.
- Esta UI es terminal nativa, no un frontend web. No tiene puntos de quiebre móviles ni widgets visuales; las dimensiones se adaptan a terminales de escritorio y máquinas virtuales al cambiar tamaño.
- No se añadió animación por `prefers-reduced-motion`: esta superficie no anima.

