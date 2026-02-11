const { invoke } = window.__TAURI__.core;

// ---- DOM Elements ----
const viewLogin = document.getElementById("view-login");
const viewSettings = document.getElementById("view-settings");
const loginForm = document.getElementById("login-form");
const loginError = document.getElementById("login-error");
const btnLogin = document.getElementById("btn-login");
const btnSave = document.getElementById("btn-save");
const btnLogout = document.getElementById("btn-logout");
const btnOpenFolder = document.getElementById("btn-open-folder");
const btnChangeFolder = document.getElementById("btn-change-folder");
const toggleAutostart = document.getElementById("toggle-autostart");
const toggleDelete = document.getElementById("toggle-delete");

// ---- State ----
let currentConfig = null;

// ---- Initialize ----
async function init() {
    try {
        const auth = await invoke("check_auth");
        if (auth) {
            await loadSettings();
            showView("settings");
        }
    } catch {
        showView("login");
    }

    // Start status polling
    setInterval(updateStatus, 5000);
}

// ---- View Management ----
function showView(view) {
    viewLogin.classList.toggle("hidden", view !== "login");
    viewSettings.classList.toggle("hidden", view !== "settings");
}

// ---- Login ----
loginForm.addEventListener("submit", async (e) => {
    e.preventDefault();
    loginError.classList.add("hidden");

    const serverUrl = document.getElementById("server-url").value.trim();
    const email = document.getElementById("email").value.trim();
    const password = document.getElementById("password").value;

    if (!serverUrl || !email || !password) return;

    setLoading(true);

    try {
        await invoke("login", {
            email,
            password,
            serverUrl,
        });

        await loadSettings();
        showView("settings");
    } catch (err) {
        loginError.textContent = typeof err === "string" ? err : "Error de conexión. Verificá la URL y las credenciales.";
        loginError.classList.remove("hidden");
    } finally {
        setLoading(false);
    }
});

function setLoading(loading) {
    btnLogin.disabled = loading;
    btnLogin.querySelector(".btn-text").classList.toggle("hidden", loading);
    btnLogin.querySelector(".btn-loading").classList.toggle("hidden", !loading);
}

// ---- Settings ----
async function loadSettings() {
    try {
        const config = await invoke("get_config");
        currentConfig = config;

        document.getElementById("settings-folder").textContent = shortenPath(config.inbox_path);
        document.getElementById("settings-folder").title = config.inbox_path;
        document.getElementById("settings-server").textContent = config.server_url;
        document.getElementById("settings-server").title = config.server_url;

        toggleAutostart.checked = config.auto_start;
        toggleDelete.checked = config.delete_after_upload;

        // Load email from status
        const status = await invoke("get_status");
        if (status.email) {
            document.getElementById("settings-email").textContent = status.email;
        }
    } catch (err) {
        console.error("Failed to load settings:", err);
    }
}

// ---- Save Settings ----
btnSave.addEventListener("click", async () => {
    if (!currentConfig) return;

    const newConfig = {
        ...currentConfig,
        auto_start: toggleAutostart.checked,
        delete_after_upload: toggleDelete.checked,
    };

    try {
        await invoke("save_config", { config: newConfig });
        currentConfig = newConfig;

        // Update autostart with the plugin
        if (newConfig.auto_start) {
            try {
                const { enable } = await import("@tauri-apps/plugin-autostart");
                await enable();
            } catch { }
        } else {
            try {
                const { disable } = await import("@tauri-apps/plugin-autostart");
                await disable();
            } catch { }
        }

        // Brief visual feedback
        btnSave.textContent = "✓ Guardado";
        setTimeout(() => {
            btnSave.textContent = "Guardar cambios";
        }, 1500);
    } catch (err) {
        console.error("Failed to save config:", err);
    }
});

// ---- Logout ----
btnLogout.addEventListener("click", async () => {
    try {
        await invoke("logout");
        showView("login");
        // Clear login form
        document.getElementById("server-url").value = "";
        document.getElementById("email").value = "";
        document.getElementById("password").value = "";
        loginError.classList.add("hidden");
    } catch (err) {
        console.error("Failed to logout:", err);
    }
});

// ---- Open Folder ----
btnOpenFolder.addEventListener("click", async () => {
    try {
        await invoke("open_inbox_folder");
    } catch (err) {
        console.error("Failed to open folder:", err);
    }
});

// ---- Change Folder ----
btnChangeFolder.addEventListener("click", async () => {
    try {
        const { open } = await import("@tauri-apps/plugin-dialog");
        const selected = await open({
            directory: true,
            multiple: false,
            title: "Seleccionar carpeta Inbox",
        });

        if (selected && currentConfig) {
            currentConfig.inbox_path = selected;
            document.getElementById("settings-folder").textContent = shortenPath(selected);
            document.getElementById("settings-folder").title = selected;
        }
    } catch (err) {
        console.error("Failed to select folder:", err);
    }
});

// ---- Status Updates ----
async function updateStatus() {
    try {
        const status = await invoke("get_status");
        const badge = document.getElementById("connection-status");
        const statusText = badge.querySelector(".status-text");
        const queueCount = document.getElementById("queue-count");

        badge.className = "status-badge";
        if (!status.online) {
            badge.classList.add("offline");
            statusText.textContent = "Sin conexión";
        } else if (status.uploading) {
            badge.classList.add("syncing");
            statusText.textContent = "Subiendo...";
        } else {
            badge.classList.add("connected");
            statusText.textContent = "Conectado";
        }

        const count = status.queue_size;
        queueCount.textContent = count === 0
            ? "Sin archivos pendientes"
            : `${count} archivo${count > 1 ? "s" : ""} pendiente${count > 1 ? "s" : ""}`;
    } catch {
        // View might not be active
    }
}

// ---- Helpers ----
function shortenPath(path) {
    if (!path) return "—";
    const home = path.replace(/^\/Users\/[^/]+/, "~");
    if (home.length > 45) {
        return "..." + home.slice(-42);
    }
    return home;
}

// ---- Start ----
document.addEventListener("DOMContentLoaded", init);
