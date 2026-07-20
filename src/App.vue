<script setup>
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { ElMessage } from "element-plus";
import { Setting, Coin, FolderOpened } from "@element-plus/icons-vue";
import { SettingsDialog } from "@gebinee/components";

const inputText = ref("");
const segments = ref([]);
const transforming = ref(false);
const settingsVisible = ref(false);
const saving = ref(false);
const useCustomDb = ref(false);
const dbPath = ref("");
const dbInfo = ref(null);

const settingsTabs = [{ name: "database", label: "数据库", icon: Coin }];

async function handleTransform() {
  transforming.value = true;
  try {
    segments.value = await invoke("transform", { input: inputText.value });
  } catch (e) {
    ElMessage.error(String(e));
  } finally {
    transforming.value = false;
  }
}

function clearLeft() {
  inputText.value = "";
}

function clearRight() {
  segments.value = [];
}

function clearAll() {
  inputText.value = "";
  segments.value = [];
}

async function openSettings() {
  settingsVisible.value = true;
  await Promise.all([loadConfig(), loadDatabaseInfo()]);
}

async function loadConfig() {
  try {
    const config = await invoke("get_config");
    useCustomDb.value = config.use_custom_db;
    dbPath.value = config.db_path;
  } catch (e) {
    ElMessage.error(String(e));
  }
}

async function loadDatabaseInfo() {
  try {
    dbInfo.value = await invoke("get_database_info");
  } catch (e) {
    dbInfo.value = null;
    ElMessage.error(String(e));
  }
}

async function browseDbPath() {
  try {
    const selected = await open({
      filters: [{ name: "SQLite 数据库", extensions: ["sqlite", "db"] }],
    });
    if (selected) {
      dbPath.value = selected;
    }
  } catch (e) {
    ElMessage.error(String(e));
  }
}

async function handleSave() {
  if (useCustomDb.value && !dbPath.value.trim()) {
    ElMessage.error("数据库文件路径不能为空");
    return;
  }
  saving.value = true;
  try {
    await invoke("set_config", {
      useCustomDb: useCustomDb.value,
      dbPath: dbPath.value,
    });
    ElMessage.success("配置已保存");
    settingsVisible.value = false;
    await loadDatabaseInfo();
  } catch (e) {
    ElMessage.error(String(e));
  } finally {
    saving.value = false;
  }
}
</script>

<template>
  <div class="app">
    <header class="toolbar">
      <div class="toolbar-left">
        <el-button @click="clearLeft">清空左边</el-button>
        <el-button @click="clearRight">清空右边</el-button>
        <el-button @click="clearAll">清空全部</el-button>
        <el-button
          type="primary"
          :loading="transforming"
          @click="handleTransform"
        >
          开始注音
        </el-button>
      </div>
      <el-button @click="openSettings">
        <el-icon><Setting /></el-icon>
        <span>设置</span>
      </el-button>
    </header>

    <main class="main-area">
      <div class="pane pane-left">
        <textarea
          v-model="inputText"
          class="input-area"
          placeholder="请输入英文文本..."
        ></textarea>
      </div>
      <div class="pane pane-right">
        <div class="output-area">
          <template v-for="(seg, i) in segments" :key="i">
            <span
              v-if="seg.is_word && !seg.found"
              class="word-not-found"
              >{{ seg.text }}</span
            >
            <span v-else>{{ seg.text }}</span>
          </template>
        </div>
      </div>
    </main>

    <SettingsDialog
      v-model:visible="settingsVisible"
      app-name="简而明国际英语注音软件"
      :tabs="settingsTabs"
      :saving="saving"
      @save="handleSave"
    >
      <template #tab-database>
        <div class="db-tab">
          <div class="db-switch-row">
            <el-switch v-model="useCustomDb" />
            <span class="db-switch-label">使用自定义数据库</span>
          </div>
          <div class="db-hint" v-if="!useCustomDb">当前使用内置数据库</div>
          <div class="db-path-row" v-else>
            <el-input
              v-model="dbPath"
              disabled
              placeholder="请选择数据库文件"
            />
            <el-button @click="browseDbPath">
              <el-icon><FolderOpened /></el-icon>
              <span>浏览</span>
            </el-button>
          </div>
          <div class="db-info" v-if="dbInfo">
            <div class="db-info-row">单词总数：{{ dbInfo.word_count }}</div>
            <div class="db-info-row">数据库路径：{{ dbInfo.db_path }}</div>
            <div class="db-info-row">连接模式：{{ dbInfo.mode }}</div>
            <div
              class="db-info-row db-error"
              v-if="!dbInfo.connected || dbInfo.error"
            >
              {{ dbInfo.error || "数据库未连接" }}
            </div>
          </div>
        </div>
      </template>
    </SettingsDialog>
  </div>
</template>

<style scoped>
.app {
  display: flex;
  flex-direction: column;
  height: 100vh;
  --pane-bg: #ffffff;
  --pane-border: #dcdfe6;
}

@media (prefers-color-scheme: dark) {
  .app {
    --pane-bg: #1a1a1a;
    --pane-border: #3a3a3a;
  }
}

.toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  height: 56px;
  padding: 0 12px;
  box-sizing: border-box;
  border-bottom: 1px solid var(--pane-border);
  flex-shrink: 0;
}

.toolbar-left {
  display: flex;
  align-items: center;
  gap: 8px;
}

.main-area {
  flex: 1;
  display: flex;
  overflow: hidden;
  min-height: 0;
}

.pane {
  flex: 1;
  display: flex;
  overflow: hidden;
  min-width: 0;
}

.pane-left {
  border-right: 1px solid var(--pane-border);
}

.input-area {
  flex: 1;
  width: 100%;
  border: none;
  resize: none;
  padding: 12px;
  font-size: 18px;
  font-family: 'aaae', Inter, Avenir, Helvetica, Arial, sans-serif;
  outline: none;
  background: var(--pane-bg);
  color: inherit;
  box-sizing: border-box;
}

.input-area::placeholder {
  color: #a8abb2;
}

.output-area {
  flex: 1;
  padding: 12px;
  font-size: 18px;
  font-family: 'aaae', Inter, Avenir, Helvetica, Arial, sans-serif;
  overflow-y: auto;
  white-space: pre-wrap;
  background: var(--pane-bg);
  color: inherit;
  box-sizing: border-box;
}

.word-not-found {
  color: #e6a23c;
  text-decoration: underline dashed;
}

.db-tab {
  padding: 8px 0;
}

.db-switch-row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.db-switch-label {
  font-size: 15px;
}

.db-hint {
  margin-top: 12px;
  color: var(--el-text-color-secondary, #909399);
  font-size: 14px;
}

.db-path-row {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 12px;
}

.db-path-row :deep(.el-input) {
  flex: 1;
}

.db-info {
  margin-top: 20px;
  padding: 12px;
  background: var(--el-fill-color-light, #f5f7fa);
  border-radius: 4px;
  font-size: 14px;
}

.db-info-row {
  line-height: 1.8;
  word-break: break-all;
}

.db-error {
  color: var(--el-color-danger, #f56c6c);
}
</style>
