<p align="center">
  <a href="#english">English</a> • <a href="#繁體中文">繁體中文</a>
</p>

---

<a id="english"></a>

# Zero Preview (Zed Extension) ⚡

> **Lightweight, reliable HTML live preview, built for [Zed](https://zed.dev/).**  
> *(macOS • Windows • Linux)*

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Zed Extension](https://img.shields.io/badge/Zed-Extension-blueviolet.svg)](https://zed.dev/extensions)
[![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Windows%20%7C%20Linux-lightgrey.svg)](#)

---

## 🌟 Why Zero Preview?

**Zero Preview** is a lightweight HTML live preview for Zed: open in the browser, reload on save, and hot-update CSS—without starting a separate dev server or taking over the terminal panel.

It is **not** positioned as “a better live-server.” The point is **zero setup**, **highly flexible path compatibility** (spaces, `#`, `%`, Chinese paths, deep folders), **single-file instant preview**, and **no editing interference**.

| Differences | Typical dev server / `live-server` | Zero Preview |
| :--- | :--- | :--- |
| Setup | Start a server by hand, pick a root | Open HTML → code action |
| URL model | Workspace root + websocket | `/raw/` absolute-path addressing + SSE |
| Path compatibility | Special characters often break (multiple open issues) | Segment URL encoding (already proven) |
| Root-relative assets | “Serves incorrect root” class of problems | Referer fallback |
| Editor interference | Reports of LSP pollution / text mixups | Thin LSP shell—no completions or diagnostics |
| One-key shortcut | Usually none | Optional `curl` + task/keymap (see Advanced) |

---

## ✨ Features

- 🔕 **Zero terminal clutter** — Runs in the background as a Zed language server.
- 📦 **Self-contained binary** — Zed downloads the matching platform file from Releases; no Node.js required.
- 🔄 **Live reload and CSS hot update** — Saving HTML refreshes the page; saving CSS hot-swaps styles over SSE.
- 🛡️ **Highly flexible path compatibility** — Correct segment encoding for spaces, `#`, `%`, `&`, `+`, parentheses, and Unicode / Chinese paths.
- 🧠 **Smart Referer fallback** — Root-relative assets such as `/images/hero.jpg` resolve from the file currently being previewed.
- 🌐 **CORS ready** — `Access-Control-Allow-Origin: *` for ES modules and Web Workers.
- 🔗 **Multi-worktree safe** — Uses port **52331**; if another Zed window already started the server, this instance enters client mode.

---

## 🚀 Install

1. Open **Zed → Extensions** (`Cmd/Ctrl + Shift + X`).
2. Search for **"Zero Preview"**.
3. Install the extension.

The first time you use it on an HTML file, Zed downloads the platform binary from [GitHub Releases](https://github.com/kuozher/zed-zero-preview/releases).

### Dev install (contributors)

```bash
git clone https://github.com/kuozher/zed-zero-preview.git
cd zed-zero-preview
```

In Zed: **Extensions → Install Dev Extension** → select the repo directory.

Optional local override (`settings.json`):

```json
{
  "lsp": {
    "zero-preview": {
      "binary": {
        "path": "/absolute/path/to/target/release/zero-preview-server"
      }
    }
  }
}
```

---

## ⌨️ How to Use

1. Open any `.html` file in Zed.
2. Open code actions: **`Cmd + .`** (macOS) or **`Ctrl + .`** (Windows/Linux), or right-click → **Code Actions**.
3. Choose:
   - **Zero Preview: Open in Browser**
   - **Zero Preview: Stop Preview Server**

Saving HTML reloads the page; saving a linked CSS file hot-updates styles without a full refresh.

---

## 🛠️ Advanced: One-key shortcut via `curl`

The default flow uses code actions. If you want the old **Cmd/Ctrl + Alt + V** one-key experience, bind a Zed task that calls the control endpoint (the server must already be running—trigger a code action on any HTML file once so the LSP starts):

### Step 1 — `tasks.json`

Open with **Command Palette → `zed: open tasks`** and add:

```json
[
  {
    "label": "Zero Preview: Open Current File",
    "command": "curl",
    "args": [
      "-Gsf",
      "http://127.0.0.1:52331/__open",
      "--data-urlencode",
      "file=$ZED_FILE"
    ],
    "use_new_terminal": false,
    "allow_concurrent_runs": true,
    "reveal": "never",
    "hide": "always",
    "save": "current"
  },
  {
    "label": "Zero Preview: Stop Server",
    "command": "curl",
    "args": ["-sf", "http://127.0.0.1:52331/__stop"],
    "use_new_terminal": false,
    "allow_concurrent_runs": true,
    "reveal": "never",
    "hide": "always"
  }
]
```

Manual test (replace with your absolute path):

```bash
curl -Gsf "http://127.0.0.1:52331/__open" --data-urlencode "file=/Users/you/project/index.html"
```

### Step 2 — `keymap.json`

Open with **Command Palette → `zed: open keymap`** and add:

```json
[
  {
    "context": "Workspace",
    "bindings": {
      "ctrl-alt-v": ["task::Spawn", { "task_name": "Zero Preview: Open Current File" }],
      "cmd-alt-v": ["task::Spawn", { "task_name": "Zero Preview: Open Current File" }],
      "ctrl-alt-shift-v": ["task::Spawn", { "task_name": "Zero Preview: Stop Server" }],
      "cmd-alt-shift-v": ["task::Spawn", { "task_name": "Zero Preview: Stop Server" }]
    }
  }
]
```

### Optional — Rebind the code actions menu

To open the same menu as **`Cmd/Ctrl + .`** with another key:

```json
[
  {
    "context": "Editor",
    "bindings": {
      "cmd-shift-a": "editor::ToggleCodeActions",
      "ctrl-shift-a": "editor::ToggleCodeActions"
    }
  }
]
```

---

## 🔄 Migrating from the Node.js version

The old [kuozher/zero-live-preview](https://github.com/kuozher/zero-live-preview) Node.js install path is replaced by this extension.

1. **Uninstall the old setup** (removes copied scripts and merged task/keymap entries):

   ```bash
   node install.js --uninstall
   ```

2. **Install this extension** (see the Zed Extensions steps above).

3. **Port change** — The Rust server uses **52331** (the Node version used **52330**), so both can coexist during migration.

4. **Task names** — The old `Live Preview: …` tasks and the optional `Zero Preview: …` curl tasks above do not conflict; you can switch over gradually.

---

## 📄 License

This project is licensed under the [MIT License](LICENSE).

- **Disclaimer**: An independent project; not affiliated with Zed Industries and not officially endorsed.
- **AI disclosure**: Conceived and designed by kuozher. Implementation used AI coding assistants in Cursor (Fable 5 for planning; Grok 4.6 and Composer 2.5 mixed according to task difficulty).

---

<br/>

<a id="繁體中文"></a>

# Zero Preview（Zed 擴充功能）⚡

> **輕巧穩定的 HTML 即時預覽，專為 [Zed](https://zed.dev/) 設計。**  
> *(macOS • Windows • Linux)*

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Zed Extension](https://img.shields.io/badge/Zed-Extension-blueviolet.svg)](https://zed.dev/extensions)
[![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Windows%20%7C%20Linux-lightgrey.svg)](#)

---

## 🌟 為什麼選 Zero Preview？

**Zero Preview** 是 Zed 上的輕量 HTML 即時預覽：可在瀏覽器開啟、存檔自動重載、CSS 熱更新——不需要另外起 dev server，也不會佔用終端機版面。

定位**不是**「更好的 live-server」，而是 **零設定**、**高彈性路徑相容**（空格、`#`、`%`、中文路徑、深層目錄）、**單檔即開**、**不干擾編輯**。

| 差異比較 | 一般 dev server / `live-server` | Zero Preview |
| :--- | :--- | :--- |
| 設定 | 手動起 server、指定根目錄 | 開 HTML → code action |
| URL 模型 | workspace 根目錄 + websocket | `/raw/` 絕對路徑定址 + SSE |
| 路徑相容 | 特殊字元常出問題（多個 open issue） | 分段 URL 編碼（已驗證邏輯） |
| 根相對資源 | 「serves incorrect root」類問題 | Referer fallback |
| 編輯干擾 | 曾有 LSP 污染 / 文字混亂回報 | 薄 LSP 殼——不回傳補全或診斷 |
| 一鍵快捷鍵 | 通常沒有 | 可選 `curl` + task/keymap（見進階章節） |

---

## ✨ 主要特色

- 🔕 **終端機零干擾** — 以 Zed language server 在背景執行。
- 📦 **自含式 binary** — Zed 自動從 Release 下載對應平台檔案，無需 Node.js。
- 🔄 **即時重載與 CSS 熱更新** — HTML 存檔刷新頁面；CSS 存檔透過 SSE 熱抽換樣式。
- 🛡️ **高彈性路徑相容** — 正確分段編碼，支援空格、`#`、`%`、`&`、`+`、括號及 Unicode / 中文路徑。
- 🧠 **智慧 Referer 補全** — 根相對資源如 `/images/hero.jpg` 依目前預覽檔定位。
- 🌐 **CORS 就緒** — `Access-Control-Allow-Origin: *`，支援 ES modules 與 Web Workers。
- 🔗 **多 worktree 安全** — 使用 port **52331**；若另一 Zed 視窗已啟動 server 則進入 client 模式。

---

## 🚀 安裝

1. 開啟 **Zed → Extensions**（`Cmd/Ctrl + Shift + X`）。
2. 搜尋 **「Zero Preview」**。
3. 安裝擴充功能。

首次在 HTML 檔上使用時，Zed 會從 [GitHub Releases](https://github.com/kuozher/zed-zero-preview/releases) 下載平台 binary。

### 開發版安裝（貢獻者）

```bash
git clone https://github.com/kuozher/zed-zero-preview.git
cd zed-zero-preview
```

在 Zed：**Extensions → Install Dev Extension** → 選取 repo 目錄。

可選的本機覆寫（`settings.json`）：

```json
{
  "lsp": {
    "zero-preview": {
      "binary": {
        "path": "/絕對路徑/target/release/zero-preview-server"
      }
    }
  }
}
```

---

## ⌨️ 如何使用

1. 在 Zed 開啟任一 `.html` 檔。
2. 叫出 code actions：**`Cmd + .`**（macOS）或 **`Ctrl + .`**（Windows/Linux），或右鍵 → **Code Actions**。
3. 選擇：
   - **Zero Preview: Open in Browser**
   - **Zero Preview: Stop Preview Server**

存 HTML 會重載頁面；存連結的 CSS 會熱更新樣式，無需整頁刷新。

---

## 🛠️ 進階：用 `curl` 一鍵快捷鍵

預設流程使用 code actions。若想要舊版 **Cmd/Ctrl + Alt + V** 的一鍵體驗，可綁定 Zed task 呼叫控制端點（server 須已在執行——先對任一 HTML 觸發一次 code action 以啟動 LSP）：

### 步驟 1 — `tasks.json`

以 **命令面板 → `zed: open tasks`** 開啟，並加入：

```json
[
  {
    "label": "Zero Preview: Open Current File",
    "command": "curl",
    "args": [
      "-Gsf",
      "http://127.0.0.1:52331/__open",
      "--data-urlencode",
      "file=$ZED_FILE"
    ],
    "use_new_terminal": false,
    "allow_concurrent_runs": true,
    "reveal": "never",
    "hide": "always",
    "save": "current"
  },
  {
    "label": "Zero Preview: Stop Server",
    "command": "curl",
    "args": ["-sf", "http://127.0.0.1:52331/__stop"],
    "use_new_terminal": false,
    "allow_concurrent_runs": true,
    "reveal": "never",
    "hide": "always"
  }
]
```

手動測試（請換成你的絕對路徑）：

```bash
curl -Gsf "http://127.0.0.1:52331/__open" --data-urlencode "file=/Users/you/project/index.html"
```

### 步驟 2 — `keymap.json`

以 **命令面板 → `zed: open keymap`** 開啟，並加入：

```json
[
  {
    "context": "Workspace",
    "bindings": {
      "ctrl-alt-v": ["task::Spawn", { "task_name": "Zero Preview: Open Current File" }],
      "cmd-alt-v": ["task::Spawn", { "task_name": "Zero Preview: Open Current File" }],
      "ctrl-alt-shift-v": ["task::Spawn", { "task_name": "Zero Preview: Stop Server" }],
      "cmd-alt-shift-v": ["task::Spawn", { "task_name": "Zero Preview: Stop Server" }]
    }
  }
]
```

### 選用 — 重新綁定 code actions 選單

若希望用其他鍵叫出 **`Cmd/Ctrl + .`** 同款選單：

```json
[
  {
    "context": "Editor",
    "bindings": {
      "cmd-shift-a": "editor::ToggleCodeActions",
      "ctrl-shift-a": "editor::ToggleCodeActions"
    }
  }
]
```

---

## 🔄 從 Node.js 版遷移

舊版 [kuozher/zero-live-preview](https://github.com/kuozher/zero-live-preview) Node.js 安裝方式已由本擴充功能取代。

1. **解除舊版安裝**（移除複製的腳本與合併過的 task/keymap）：

   ```bash
   node install.js --uninstall
   ```

2. **安裝本擴充功能**（見上方 Zed Extensions 步驟）。

3. **Port 變更** — Rust server 使用 **52331**（Node 版為 **52330**），遷移期可並存。

4. **Task 名稱** — 舊的 `Live Preview: …` 與上方可選的 `Zero Preview: …` curl task 互不衝突，可逐步替換。

---

## 📄 開源授權

本專案採用 [MIT License](LICENSE) 授權。

- **免責聲明**：獨立開發專案，不隸屬於 Zed Industries，亦未獲官方背書。
- **AI 揭露**：由 kuozher 構思與設計；實作過程使用 AI 程式輔助工具（Cursor 中的模型 Fable 5 規劃、Grok 4.6 和 Composer 2.5 按照任務困難程度混合使用、執行）。
