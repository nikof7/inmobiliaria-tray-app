# Drop Folder + Inbox: Sistema de Ingesta de Documentos

## Resumen Ejecutivo

Sistema de gestión documental para administración de propiedades donde los usuarios guardan archivos en una carpeta local de su computadora y estos aparecen automáticamente en una bandeja de entrada web para ser clasificados y asignados a la entidad correspondiente (propiedad, inquilino, contrato, etc.).

El objetivo es eliminar por completo la fricción de "descargar un archivo → abrir el navegador → buscar el botón de subir → seleccionar el archivo → subirlo". En su lugar, el flujo se reduce a: **guardar el archivo en una carpeta → clasificarlo cuando quieras desde la web**.

---

## El Problema

Los usuarios de la aplicación de gestión de propiedades trabajan constantemente con documentos: facturas, contratos, identificaciones, fotos, comprobantes, etc. Estos archivos llegan por múltiples canales (email, WhatsApp, escáner, descargas web) y necesitan terminar vinculados a una entidad específica dentro del sistema.

El flujo actual obliga al usuario a:

1. Recibir o descargar el archivo en algún lugar de su computadora
2. Abrir la aplicación web
3. Navegar hasta la entidad correcta
4. Usar un formulario de carga para seleccionar el archivo
5. Completar metadata (tags, categoría)

Este proceso es suficientemente molesto como para que los usuarios lo posterguen, lo hagan a medias, o directamente no lo hagan — resultando en documentos desorganizados, perdidos o nunca registrados en el sistema.

---

## La Solución

### Concepto Central

Cada usuario tiene **una carpeta local en su computadora** llamada "MiApp Inbox" (o el nombre que se defina). Una **aplicación de escritorio mínima** (tray app) vigila esa carpeta. Cada vez que un archivo nuevo aparece ahí, la aplicación lo sube automáticamente al servidor en segundo plano.

Los archivos subidos llegan a una **bandeja de entrada** dentro de la aplicación web, donde el usuario — cuando le resulte conveniente — los clasifica: les asigna una entidad, les pone tags, y los archiva. Una vez clasificado, el archivo desaparece de la bandeja y queda vinculado a su entidad correspondiente como cualquier otro documento del sistema.

### Principio Fundamental

**Separar el acto de capturar un documento del acto de clasificarlo.** Capturar debe ser instantáneo y sin fricción (guardar en una carpeta). Clasificar puede hacerse después, por lotes, cuando el usuario tenga un momento.

---

## Flujo del Usuario

### Captura (en la computadora)

1. El usuario recibe un archivo por cualquier medio.
2. En lugar de guardarlo en Descargas o en el escritorio, lo guarda (o mueve) a la carpeta **Inmobiliaria** que está en su computadora.
3. Un ícono en la bandeja del sistema le confirma visualmente que el archivo fue detectado y subido.
4. El usuario sigue con lo que estaba haciendo.

No necesita abrir el navegador. No necesita buscar una entidad. No necesita llenar ningún formulario. Solo guarda un archivo en una carpeta.

### Clasificación (en la web)

1. El usuario abre la aplicación web cuando quiera.
2. Ve un indicador claro: **"Tenés 5 archivos por clasificar"**.
3. Entra a la bandeja de entrada y ve una lista de archivos pendientes, cada uno con:
   - Nombre del archivo
   - Fecha y hora de subida
   - Quién lo subió (relevante si hay múltiples usuarios)
4. Para cada archivo, el usuario:
   - Selecciona a qué entidad pertenece (propiedad, inquilino, propietario.) usando un selector rápido.
   - Selecciona a qué elemento de la entidad pertenece (ej. Inquilino Juan Pérez)
   - Opcionalmente agrega tags descriptivos (factura, identificación, contrato, foto, etc.)
   - Opcionalmente renombra el archivo
5. Hace click en "Clasificar" y el archivo pasa de la bandeja a estar vinculado a la entidad seleccionada.

La clasificación se puede hacer de a uno o en lote.

---

## Componentes del Sistema

### 1. Aplicación de Escritorio (Tray App)

Una aplicación construida con Tauri que se instala en la computadora del usuario. Es mínima — no tiene ventana principal, solo vive en la bandeja del sistema (system tray).

**Qué hace:**

- Al instalarse, crea la carpeta "MiApp Inbox" en una ubicación estándar del sistema (Documentos o donde el usuario elija).
- Se ejecuta al iniciar el sistema operativo.
- Vigila continuamente la carpeta por archivos nuevos.
- Cuando detecta un archivo nuevo, lo sube al servidor vía la API de PocketBase.
- Muestra un ícono en la bandeja del sistema con estado actual.
- Notifica brevemente al usuario cuando un archivo se subió exitosamente.
- Elimina automáticamente el archivo local después de una subida exitosa (o lo mueve a una subcarpeta "subidos", configurable).

**Menú de la bandeja del sistema:**

- Abrir carpeta → Abre la carpeta en Finder/Explorer
- Abrir MiApp → Abre la aplicación web en el navegador
- Estado: "Conectado" / "Sin conexión" / "Subiendo..."
- Archivos recientes: lista de los últimos archivos subidos con estado
- Configuración → Configurar carpeta, servidor, credenciales
- Salir

**Comportamiento offline:** Si no hay conexión a internet, la tray app mantiene los archivos en la carpeta y los sube cuando se restablezca la conexión. El usuario ve un indicador de "pendientes de subida".

**Autenticación:** El usuario inicia sesión una vez al configurar la app (las mismas credenciales que usa en la web). El token se almacena de forma segura en el keychain del sistema operativo.

**Archivos ignorados:** La app ignora archivos temporales del sistema operativo (.DS*Store, Thumbs.db, desktop.ini), archivos temporales de aplicaciones (~$*.docx, \_.tmp, \*.swp), y archivos ocultos.

### 2. Bandeja de Entrada Web (Inbox)

Una sección dentro de la aplicación React existente que muestra los archivos subidos por el usuario logeado que aún no han sido clasificados.

**Qué muestra:**

- Contador visible en la navegación principal: badge con número de archivos pendientes.
- Lista de archivos pendientes ordenados por fecha de subida (más recientes primero).
- Para cada archivo: vista previa, nombre y fecha.
- Interfaz de clasificación rápida inline (sin abrir otra página).

**Funcionalidades de clasificación:**

- Selector de entidades: propietarios, inquilinos o propiedades.
- Buscador de elementos dentro de la entidad seleccionada.
- Tags: input multiselect de tags
- Renombrar: posibilidad de cambiar el nombre del archivo antes de clasificarlo.
- Acción "Clasificar": mueve el archivo de inbox a documentos de la entidad seleccionada.
- Acción "Descartar": elimina el archivo (con confirmación).

**Operaciones en lote:**

- Aplicar la misma entidad y elemento a todos los seleccionados de una vez.
- Útil cuando el usuario sube varios documentos de la misma propiedad, inquilino o propietario.

### 3. Base de Datos (PocketBase)

Dos colecciones relevantes:

**Colección "files_inbox"** — archivos subidos pendientes de clasificación:

- file: Archivo (campo file nativo de PocketBase)
- name: Nombre original del archivo
- user: Usuario que lo subió
- created: Fecha de subida
- status: Estado pendiente / clasificado / descartado

**Colección ya existente "files"** — ya clasificados y vinculados a una entidad.

- file: campo file nativo de pocketbase
- name: nombre del archivo
- tags: relación con la colección de file_tags
- owner: relación con owners
- tenant: relación con tenants
- property: relación con properties

Cuando un archivo se clasifica, se mueve de "inbox" a "files" (o se crea el registro en files y se marca el de inbox como clasificado).

---

## Experiencia del Usuario — Escenarios Reales

### Escenario 1: Factura recibida por email

> María administra 15 propiedades. Recibe por email la factura del plomero que arregló un caño en Av. Libertador 1234.

1. Abre el email, hace click derecho en el adjunto → "Guardar como" → selecciona la carpeta "MiApp Inbox".
2. En la bandeja del sistema ve el ✓ de subida exitosa.
3. Una hora después, cuando tiene un momento, abre la web.
4. Ve "1 archivo por clasificar", entra al inbox.
5. Ve "factura-plomero-enero.pdf", escribe "Libert..." en el buscador, selecciona la propiedad.
6. Agrega tags "factura", "plomería", "mantenimiento". Click "Clasificar".
7. La factura queda vinculada a la propiedad, buscable y ordenada.

---

## Beneficios

### Para los usuarios

- **Cero cambio de contexto**: guardar un archivo en una carpeta es algo que ya saben hacer y que pueden hacer desde cualquier aplicación.
- **Velocidad**: la captura del documento toma literalmente 2 segundos. La clasificación puede esperar.
- **Trabajo por lotes**: pueden acumular documentos durante el día y clasificar todo junto en 5 minutos al final de la jornada.
- **Sin pasos intermedios**: no hay "descargar → abrir web → navegar → subir → llenar formulario". Es "guardar → clasificar cuando quieras".

### Para el sistema

- **Todos los documentos entran**: al ser tan fácil capturar, los usuarios efectivamente suben todo. No hay documentos que se quedan en carpetas personales por pereza.
- **Datos estructurados**: cada documento queda vinculado a una entidad con metadata. No hay archivos sueltos sin contexto.
- **Trazabilidad completa**: se sabe quién subió qué, cuándo, y quién lo clasificó.
- **Almacenamiento centralizado**: PocketBase guarda todo.

---

## Consideraciones Técnicas de Alto Nivel

### Tauri como plataforma de la tray app

Tauri genera ejecutables nativos a partir de código Rust + web. Un tray app en Tauri pesa aproximadamente 3-5 MB (vs. ~150 MB de Electron). Permite vigilar el filesystem con APIs nativas del sistema operativo y acceder al keychain para almacenar credenciales de forma segura. Funciona en Windows, macOS y Linux.

### Limpieza de la carpeta local

Después de subir exitosamente un archivo, la tray app tiene dos opciones configurables:

1. **Eliminarlo de la carpeta** (comportamiento por defecto): la carpeta siempre está vacía, funciona como buzón.
2. **Moverlo a una subcarpeta "Subidos"**: para usuarios que quieren tener una copia local de respaldo. Esta subcarpeta se puede limpiar periódicamente.

### Instalación y onboarding

La primera vez que el usuario abre la tray app:

1. Se le pide la URL del servidor y sus credenciales (las mismas de la web).
2. Se le pregunta dónde quiere la carpeta Inbox (por defecto: ~/Documentos/MiApp Inbox).
3. Se crea la carpeta.
4. Se ofrece crear un acceso directo a la carpeta en la barra lateral de Finder/Explorer para acceso rápido.
5. Listo.

### Sin conexión

La tray app funciona sin conexión: los archivos se guardan en la carpeta y se encolan. Cuando vuelve la conexión, se suben automáticamente en orden. El usuario ve un indicador "3 archivos pendientes de subida" en la bandeja.

---

## Alcance de Implementación

### Fase 1 — Mínimo funcional

- Tray app: vigilar carpeta, subir archivos, ícono con estado, menú básico.
- Web: página de inbox, clasificación individual, buscador de entidades, tags.
- PocketBase: colecciones inbox y documents.

### Fase 2 — Productividad

- Clasificación en lote (selección múltiple).
- Notificaciones push en la web cuando hay archivos nuevos en el inbox.
- Vista previa inline de PDFs e imágenes en el inbox.
