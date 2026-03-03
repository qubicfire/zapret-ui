import { invoke } from "@tauri-apps/api/core";

let isConnected: boolean = false;
let lastPreset: string = "";

const sitesEditor = document.getElementById("sites-editor") as HTMLTextAreaElement;
const saveBtn = document.getElementById("save-sites-btn");
const saveStatus = document.getElementById("save-status");

const presetSelect = document.getElementById("preset-select") as HTMLSelectElement;
const connectBtn = document.getElementById("connect-btn") as HTMLButtonElement;

const gameFilterCheck = document.getElementById("settings-game-filter") as HTMLInputElement;
const autostartCheck = document.getElementById("settings-auto-start") as HTMLInputElement;

async function enableZapret(selectedPreset: string) {
  if (connectBtn == null) {
    console.error("Connect button not found!");
    return;
  }
  try {
    // Меняем состояние кнопки
    connectBtn.disabled = true;
    connectBtn.style.backgroundColor = "#cfc429";
    connectBtn.textContent = "Запуск...";

    // ВЫЗОВ ВАШЕЙ КОМАНДЫ RUST
    // Название должно строго совпадать с названием функции в Rust
    const result = await invoke<{ message: string; success: boolean }>("enable_zapret", { selectedPreset: selectedPreset });

    if (result.success) {
      connectBtn.disabled = false;
      connectBtn.textContent = "Отключить";
      connectBtn.style.backgroundColor = "#b12323";
      console.log(result.message);
    }
  } catch (error) {
    console.error("Ошибка при запуске:", error);
    connectBtn.disabled = false;
    connectBtn.textContent = "Ошибка. Повторить?";
    connectBtn.style.backgroundColor = "#dc3545";
  }
}

async function disableZapret() {
  if (connectBtn == null) {
    console.error("Connect button not found!");
    return;
  }

  const result = await invoke<{ message: string; success: boolean }>("disable_zapret");

  if (result.success) {
    console.log(result.message);
  }

  connectBtn.style.backgroundColor = "#309ed4";
  connectBtn.textContent = "Включить";
}

async function changeZapretStatus() {
  if (isConnected == false) {
    const selectedPreset = presetSelect.value;
  
    if (!selectedPreset) {
      alert("Сначала выберите пресет!");
      return;
    }

    // Визуальный эффект
    connectBtn.classList.add("processing");
    enableZapret(selectedPreset);
    isConnected = true;
  } else {
    disableZapret();
    isConnected = false;
  }
}

async function loadSites() {
  try {
    const content = await invoke<string>("read_sites_list");
    sitesEditor.value = content;
  } catch (err) {
    console.error("Не удалось прочитать файл:", err);
  }
}

async function saveSites() {
  if (saveBtn == null || saveStatus == null) {
    console.error("Save button or status element not found!");
    return;
  }

  try {
    await invoke("save_sites_list", { content: sitesEditor.value });
    
    // Визуальное подтверждение
    if (saveStatus) {
      saveStatus.textContent = "Сохранено!";
      saveStatus.style.opacity = "1";
      setTimeout(() => { saveStatus.style.opacity = "0"; }, 2000);
    }
  } catch (err) {
    alert("Ошибка при сохранении: " + err);
  } finally {
    saveBtn.textContent = "Сохранить изменения";
  }
}

async function loadPresets() {
  try {
    const presets = await invoke<string[]>("get_presets");
    presetSelect.innerHTML = ""; // Очищаем
    presetSelect.onclick = async () => {
      lastPreset = presetSelect.value.replace(".bat", ""); 
      await invoke("save_last_preset", { preset: lastPreset });
      console.log("Выбран пресет:", lastPreset);
    }

    if (presets.length === 0) {
      presetSelect.innerHTML = '<option value="">Пресеты не найдены</option>';
      return;
    }

    let index: number = 0;

    presets.forEach(file => {
      const option = document.createElement("option");
      option.value = file;
      option.textContent = file.replace(".bat", ""); // Убираем расширение для красоты

      if (lastPreset && option.textContent === lastPreset) {
        option.selected = true;
        index = presetSelect.options.length; // Запоминаем индекс для последующего выбора
      }

      presetSelect.appendChild(option);
    });

    presetSelect.selectedIndex = index;
    lastPreset = presetSelect.value.replace(".bat", ""); 
  } catch (err) {
    console.error("Ошибка загрузки пресетов:", err);
  }
}

async function saveConfig() {
  const newConfig = {
    last_version: "",
    last_preset: lastPreset,
    game_filter: gameFilterCheck.checked,
    auto_start: autostartCheck.checked,
  };

  await invoke("save_config", { config: newConfig });
}
  
async function loadConfig() {
  const config = await invoke<any>("get_config");

  if (config == null) {
    console.warn("No config found, using defaults");
    return;
  }

  lastPreset = config.last_preset || "";
  gameFilterCheck.checked = config.game_filter;
  autostartCheck.checked = config.auto_start;
}

function setupNavigation() {
  // get all .nav-btn and .view elements in div-class app-container
  const navButtons = document.querySelectorAll(".nav-btn");
  const views = document.querySelectorAll(".view");
  let previousTargetId: any = null;

  navButtons.forEach((btn) => {
    btn.addEventListener("click", () => {
      const targetId = btn.getAttribute("data-target");

      navButtons.forEach((b) => b.classList.remove("active"));
      views.forEach((v) => v.classList.remove("active"));

      btn.classList.add("active");
      if (targetId) {
        document.getElementById(targetId)?.classList.add("active");

        if (previousTargetId && previousTargetId === "settings") {
          console.log("Saving config before leaving settings");
          saveConfig();
        }

        if (targetId === "sites") {
          loadSites();
        }
      }

      previousTargetId = targetId;
    });
  });
}

window.addEventListener("DOMContentLoaded", () => {
  loadConfig();
  loadPresets();
  setupNavigation();

  connectBtn?.addEventListener("click", () => {
    console.log("Connect button clicked");
    changeZapretStatus();
  });

  saveBtn?.addEventListener("click", () => {
    console.log("Save button clicked");
    saveSites();
  });

  window.addEventListener("keydown", (event) => {
    if (event.ctrlKey && event.key.toLocaleLowerCase() === "s") {
      event.preventDefault();

      const activeView = document.querySelector(".view.active");
      if (activeView?.id === "sites") {
        console.log("Ctrl+S detected in Sites view");
        saveSites();
      }
    } 
  });
});
