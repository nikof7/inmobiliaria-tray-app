# Inmobiliaria Inbox — Tray App

Aplicación de escritorio que vive en la bandeja del sistema. Vigila una carpeta local y sube automáticamente los archivos a PocketBase para que después los clasifiques desde la web.

## Requisitos

- [Rust](https://rustup.rs/) (1.70+)
- [Node.js](https://nodejs.org/) (18+)
- [pnpm](https://pnpm.io/) (8+)
- Una instancia de PocketBase con la colección `files_inbox` configurada (campos: `file`, `name`, `user`, `status`)

## Instalación

```bash
git clone <repo-url>
cd inmobiliaria-tray-app
pnpm install
```

## Desarrollo

```bash
pnpm dev
```

## Compilar instalador

```bash
pnpm build
```

El instalador se genera en `src-tauri/target/release/bundle/`.

## Uso

1. **Primera vez** — Al abrir la app aparece una ventana de configuración. Ingresá la URL de tu servidor PocketBase y tus credenciales.
2. **Carpeta Inbox** — Se crea automáticamente en `~/Documents/Inmobiliaria Inbox` (podés cambiarla desde Configuración).
3. **Guardar archivos** — Guardá o mové cualquier archivo a la carpeta Inbox. La app lo detecta y lo sube al servidor en segundo plano.
4. **Notificación** — Recibís una notificación del sistema cuando el archivo se subió correctamente.
5. **Post-subida** — Por defecto el archivo se elimina de la carpeta (funciona como buzón). Podés cambiar esto para que se mueva a una subcarpeta `Subidos`.
6. **Sin conexión** — Los archivos se encolan y se suben automáticamente cuando vuelve la conexión.
7. **Clasificar** — Desde la aplicación web, entrá a la bandeja de entrada y clasificá los archivos asignándolos a una propiedad, inquilino o propietario.

## Menú del tray

- **Abrir carpeta** — Abre la carpeta en Finder/Explorer
- **Abrir Inmobiliaria Web** — Abre el servidor en el navegador
- **Estado** — Conectado / Sin conexión / Subiendo...
- **Archivos recientes** — Últimos archivos subidos con su estado
- **Configuración** — Cambiar carpeta, autostart, comportamiento post-subida
- **Salir**

## Archivos ignorados

`.DS_Store`, `Thumbs.db`, `desktop.ini`, `~$*`, `*.tmp`, `*.swp`, archivos ocultos.

## Stack

- **Tauri v2** — Framework desktop (~5 MB vs ~150 MB de Electron)
- **Rust** — File watcher (`notify`), uploads (`reqwest`), keychain (`keyring`)
- **Vanilla HTML/CSS/JS** — Ventana de login y configuración
- **PocketBase** — Backend (colección `files_inbox`)
