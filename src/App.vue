<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { api } from './services'
import { MAX_IMAGES, prepareImage } from './images'
import { tasksCsv, tasksJson } from './export'
import type { AppSettings, BackupInfo, Category, EmailStatus, Priority, Stats, Task, TaskInput, ViewKey } from './types'

const tasks = ref<Task[]>([])
const categories = ref<Category[]>([])
const stats = ref<Stats>({ today: 0, inbox: 0, upcoming: 0, all: 0, unfinished: 0, completed: 0, trash: 0, overdue: 0, completedToday: 0 })
const settings = ref<AppSettings>({ startView: 'today', theme: 'light', dailyBackup: true })
const activeView = ref<ViewKey | 'category'>('today')
const activeCategoryId = ref<string | null>(null)
const search = ref('')
const selectedId = ref<string | null>(null)
const loading = ref(true)
const editorOpen = ref(false)
const categoryOpen = ref(false)
const settingsOpen = ref(false)
const backupOpen = ref(false)
const editingTaskId = ref<string | null>(null)
const backups = ref<BackupInfo[]>([])
const toast = ref('')
const batchMode = ref(false)
const batchIds = ref<string[]>([])
const busy = ref(false)
const processingImages = ref(false)
const previewImage = ref<string | null>(null)
const emailStatus = ref<EmailStatus>({ configured: false, sent: 0, failed: 0, uncertain: 0, lastError: null })
const emailCode = ref('')
const mailBusy = ref(false)
const exportBusy = ref(false)
const exportMenuOpen = ref(false)
const allChecked = computed(() => tasks.value.length > 0 && tasks.value.every(task => batchIds.value.includes(task.id)))

const categoryDraft = ref({ id: '', name: '', color: '#5B8FF9' })
const form = ref<TaskInput>({ title: '', description: '', priority: 'medium', dueAt: null, categoryId: null })

const navItems: { key: ViewKey; label: string; icon: string; count: keyof Stats }[] = [
  { key: 'today', label: '今日任务', icon: '☀', count: 'today' },
  { key: 'inbox', label: '收集箱', icon: '▣', count: 'inbox' },
  { key: 'upcoming', label: '即将到期', icon: '◷', count: 'upcoming' },
  { key: 'unfinished', label: '未完成', icon: '☐', count: 'unfinished' },
  { key: 'all', label: '全部任务', icon: '☷', count: 'all' },
  { key: 'completed', label: '已完成', icon: '✓', count: 'completed' },
  { key: 'trash', label: '回收站', icon: '♲', count: 'trash' }
]

const title = computed(() => activeView.value === 'category'
  ? categories.value.find(item => item.id === activeCategoryId.value)?.name ?? '分类'
  : navItems.find(item => item.key === activeView.value)?.label ?? '任务')

const subtitle = computed(() => {
  const today = new Intl.DateTimeFormat('zh-CN', { month: 'long', day: 'numeric', weekday: 'long' }).format(new Date())
  if (activeView.value === 'today') return `${today} · 安排好今天的每一件事`
  if (activeView.value === 'trash') return '已删除任务可恢复或永久清除'
  return `${tasks.value.length} 项任务`
})

const selectedTask = computed(() => tasks.value.find(item => item.id === selectedId.value) ?? null)
const progress = computed(() => {
  const total = stats.value.today + stats.value.completedToday
  return total ? Math.round(stats.value.completedToday / total * 100) : 0
})

function flash(message: string) {
  toast.value = message
  window.setTimeout(() => { if (toast.value === message) toast.value = '' }, 2400)
}

function toLocalInput(iso: string | null) {
  if (!iso) return ''
  const date = new Date(iso)
  const local = new Date(date.getTime() - date.getTimezoneOffset() * 60000)
  return local.toISOString().slice(0, 16)
}

function fromLocalInput(value: string) {
  return value ? new Date(value).toISOString() : null
}

function dueLabel(iso: string | null) {
  if (!iso) return '无截止时间'
  const due = new Date(iso)
  const today = new Date()
  const tomorrow = new Date(today)
  tomorrow.setDate(today.getDate() + 1)
  const time = new Intl.DateTimeFormat('zh-CN', { hour: '2-digit', minute: '2-digit', hour12: false }).format(due)
  if (due.toDateString() === today.toDateString()) return `今天 ${time}`
  if (due.toDateString() === tomorrow.toDateString()) return `明天 ${time}`
  return new Intl.DateTimeFormat('zh-CN', { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit', hour12: false }).format(due)
}

function isOverdue(task: Task) {
  return task.status === 'todo' && task.dueAt && new Date(task.dueAt) < new Date() && new Date(task.dueAt).toDateString() !== new Date().toDateString()
}

function priorityLabel(priority: Priority) {
  return ({ low: '低优先级', medium: '中优先级', high: '高优先级' })[priority]
}

async function refresh() {
  loading.value = true
  try {
    const [nextTasks, nextStats, nextCategories] = await Promise.all([
      api.listTasks({ view: activeView.value, search: search.value, categoryId: activeCategoryId.value }),
      api.getStats(),
      api.listCategories()
    ])
    tasks.value = nextTasks
    batchIds.value = batchIds.value.filter(id => nextTasks.some(task => task.id === id))
    stats.value = nextStats
    categories.value = nextCategories
    if (!nextTasks.some(item => item.id === selectedId.value)) selectedId.value = nextTasks[0]?.id ?? null
  } catch (error) {
    flash(`加载失败：${String(error)}`)
  } finally {
    loading.value = false
  }
}

function changeView(view: ViewKey) {
  batchMode.value = false
  batchIds.value = []
  activeView.value = view
  activeCategoryId.value = null
}

function changeCategory(categoryId: string) {
  batchMode.value = false
  batchIds.value = []
  activeView.value = 'category'
  activeCategoryId.value = categoryId
}

function openCreate() {
  editingTaskId.value = null
  form.value = { title: '', description: '', priority: 'medium', dueAt: activeView.value === 'today' ? new Date(new Date().setHours(18, 0, 0, 0)).toISOString() : null, categoryId: activeView.value === 'category' ? activeCategoryId.value : null, images: [], localPath: null, emailReminder: false }
  editorOpen.value = true
}

function openEdit(task: Task) {
  editingTaskId.value = task.id
  form.value = { title: task.title, description: task.description, priority: task.priority, dueAt: task.dueAt, categoryId: task.categoryId, images: (task.images ?? []).map(image => ({ ...image })), localPath: task.localPath ?? null, smsReminder: task.smsReminder ?? false, emailReminder: task.emailReminder ?? false }
  editorOpen.value = true
}

async function submitTask() {
  if (!form.value.title.trim()) return flash('请输入任务名称')
  if (busy.value || processingImages.value) return
  if (form.value.emailReminder && !form.value.dueAt) return flash('邮件提醒需要设置截止时间')
  busy.value = true
  try {
    const input = { ...form.value, title: form.value.title.trim(), description: form.value.description.trim(), localPath: form.value.localPath?.trim() || null }
    if (editingTaskId.value) await api.updateTask(editingTaskId.value, input)
    else await api.createTask(input)
    editorOpen.value = false
    flash(editingTaskId.value ? '任务已更新' : '任务已创建')
    await refresh()
  } catch (error) { flash(`保存失败：${String(error)}`) }
  finally { busy.value = false }
}

async function toggleTask(task: Task) {
  await api.setCompleted(task.id, task.status !== 'completed')
  flash(task.status === 'completed' ? '任务已恢复' : '做得好，任务已完成')
  await refresh()
}

async function removeTask(task: Task) {
  const permanent = activeView.value === 'trash'
  if (permanent && !confirm(`永久删除“${task.title}”？此操作无法撤销。`)) return
  await api.deleteTask(task.id, permanent)
  flash(permanent ? '任务已永久删除' : '任务已移入回收站')
  await refresh()
}

function toggleBatch(id: string) {
  batchIds.value = batchIds.value.includes(id) ? batchIds.value.filter(value => value !== id) : [...batchIds.value, id]
}

async function deleteBatch() {
  const ids = [...batchIds.value]
  if (!ids.length || busy.value) return
  const permanent = activeView.value === 'trash'
  if (!confirm(permanent ? `永久删除选中的 ${ids.length} 项任务？无法撤销。` : `将选中的 ${ids.length} 项任务移入回收站？`)) return
  busy.value = true
  try {
    await api.deleteTasks(ids, permanent)
    batchIds.value = []
    flash(permanent ? '选中任务已永久删除' : '选中任务已移入回收站')
    await refresh()
  } catch (error) { flash(`批量删除失败：${String(error)}`) }
  finally { busy.value = false }
}

async function addImages(files: File[]) {
  if (processingImages.value) return
  const draft = form.value
  const room = MAX_IMAGES - (draft.images?.length ?? 0)
  if (room < 1) return flash(`每项任务最多 ${MAX_IMAGES} 张图片`)
  processingImages.value = true
  try {
    for (const file of files.slice(0, room)) {
      const image = await prepareImage(file)
      draft.images = [...(draft.images ?? []), image]
    }
    if (files.length > room) flash(`每项任务最多 ${MAX_IMAGES} 张图片`)
  } catch (error) { flash(String(error)) }
  finally { processingImages.value = false }
}

function pasteImage(event: ClipboardEvent) {
  const files = Array.from(event.clipboardData?.files ?? []).filter(file => file.type.startsWith('image/'))
  if (files.length) { event.preventDefault(); void addImages(files) }
}

function uploadImages(event: Event) {
  const input = event.target as HTMLInputElement
  void addImages(Array.from(input.files ?? []))
  input.value = ''
}

async function pickDirectory() {
  try { const path = await api.pickDirectory(); if (path) form.value.localPath = path }
  catch (error) { flash(String(error)) }
}

async function openDirectory(path: string) {
  try { await api.openDirectory(path) } catch (error) { flash(String(error)) }
}

async function openSettings() {
  settings.value = await api.getSettings()
  emailCode.value = ''
  emailStatus.value = await api.emailStatus(settings.value.emailAddress)
  settingsOpen.value = true
}

async function restoreTask(task: Task) {
  await api.restoreTask(task.id)
  flash('任务已恢复')
  await refresh()
}

function openCategories() {
  categoryDraft.value = { id: '', name: '', color: '#5B8FF9' }
  categoryOpen.value = true
}

function editCategory(category: Category) {
  categoryDraft.value = { id: category.id, name: category.name, color: category.color }
}

async function saveCategory() {
  if (!categoryDraft.value.name.trim()) return flash('请输入分类名称')
  await api.saveCategory({ ...categoryDraft.value, name: categoryDraft.value.name.trim(), id: categoryDraft.value.id || undefined })
  categoryDraft.value = { id: '', name: '', color: '#5B8FF9' }
  await refresh()
  flash('分类已保存')
}

async function removeCategory(category: Category) {
  if (!confirm(`删除分类“${category.name}”？该分类中的任务会移到收集箱。`)) return
  await api.deleteCategory(category.id)
  if (activeCategoryId.value === category.id) changeView('inbox')
  await refresh()
}

async function openBackups() {
  backups.value = await api.listBackups()
  backupOpen.value = true
}

async function makeBackup() {
  await api.createBackup()
  backups.value = await api.listBackups()
  flash('备份已创建')
}

async function restoreBackup(backup: BackupInfo) {
  if (!confirm(`从“${backup.name}”恢复？当前数据会先自动备份。`)) return
  await api.restoreBackup(api.desktop ? backup.path : backup.name)
  backupOpen.value = false
  await refresh()
  flash('数据已恢复')
}

async function persistSettings() {
  if (emailCode.value.trim()) {
    await api.saveEmailCode(settings.value.emailAddress ?? '', emailCode.value)
    emailCode.value = ''
  }
  await api.saveSettings({ ...settings.value, emailAddress: settings.value.emailAddress?.trim().toLowerCase() ?? '' })
  emailStatus.value = await api.emailStatus(settings.value.emailAddress)
}

async function saveSettings() {
  if (mailBusy.value) return
  mailBusy.value = true
  try {
    await persistSettings()
    emailCode.value = ''
    settingsOpen.value = false
    flash('设置已保存')
  } catch (error) { flash(`设置保存失败：${String(error)}`) }
  finally { mailBusy.value = false }
}

function closeSettings() {
  if (mailBusy.value) return
  emailCode.value = ''
  settingsOpen.value = false
}

async function testEmail() {
  if (mailBusy.value) return
  mailBusy.value = true
  try {
    await persistSettings()
    flash(await api.testEmail())
  } catch (error) { flash(`测试邮件未确认成功：${String(error)}`) }
  finally { mailBusy.value = false }
}

async function clearEmailCode() {
  if (mailBusy.value || !confirm('删除此邮箱在本机保存的授权码？需要重新填写后才能发邮件。')) return
  mailBusy.value = true
  try {
    await api.clearEmailCode(settings.value.emailAddress ?? '')
    emailCode.value = ''
    settings.value.emailEnabled = false
    await persistSettings()
    flash('授权码已删除，邮件提醒已关闭')
  } catch (error) { flash(String(error)) }
  finally { mailBusy.value = false }
}

async function retryFailedEmails() {
  if (mailBusy.value) return
  mailBusy.value = true
  try {
    await persistSettings()
    const count = await api.retryFailedEmails()
    emailStatus.value = await api.emailStatus(settings.value.emailAddress)
    flash(`${count} 条失败提醒已加入重试队列`)
  } catch (error) { flash(String(error)) }
  finally { mailBusy.value = false }
}

async function exportTasks(scope: 'all' | 'current' = 'all', format: 'csv' | 'json' = 'csv') {
  if (exportBusy.value) return
  exportBusy.value = true
  exportMenuOpen.value = false
  try {
    const items = scope === 'current' ? [...tasks.value] : await api.listTasks({ view: 'all', search: '' })
    const content = format === 'csv' ? tasksCsv(items) : tasksJson(items)
    const saved = await api.exportFile(content, format)
    if (saved) flash(`已导出 ${items.length} 项事项`)
  } catch (error) { flash(`导出失败：${String(error)}`) }
  finally { exportBusy.value = false }
}

let debounce: number | undefined
watch([activeView, activeCategoryId], refresh)
watch(search, () => {
  window.clearTimeout(debounce)
  debounce = window.setTimeout(refresh, 220)
})

onMounted(async () => {
  settings.value = await api.getSettings()
  activeView.value = settings.value.startView
  await refresh()
})
</script>

<template>
  <div class="app-shell">
    <header class="topbar">
      <div class="brand-mark">✓</div>
      <div class="brand-copy"><strong>清单</strong><span>专注当下，轻松完成</span></div>
      <label class="search-box">
        <span>⌕</span>
        <input v-model="search" aria-label="搜索任务" placeholder="搜索任务、描述或分类…" />
        <kbd>Ctrl K</kbd>
      </label>
      <div class="top-actions">
        <span class="local-badge"><i></i>{{ api.desktop ? 'SQLite 本地存储' : api.portable ? 'D盘本地存储' : '浏览器本地预览' }}</span>
        <button class="icon-button" title="备份" @click="openBackups">↧</button>
        <button class="icon-button" title="设置" @click="openSettings">⚙</button>
      </div>
    </header>

    <aside class="sidebar">
      <button class="create-button" @click="openCreate"><span>＋</span> 新建任务</button>
      <nav class="main-nav" aria-label="任务视图">
        <button v-for="item in navItems" :key="item.key" :class="{ active: activeView === item.key }" @click="changeView(item.key)">
          <span class="nav-icon">{{ item.icon }}</span><span>{{ item.label }}</span><b>{{ stats[item.count] }}</b>
        </button>
      </nav>
      <div class="category-head"><span>分类</span><button title="管理分类" @click="openCategories">＋</button></div>
      <nav class="category-nav">
        <button v-for="category in categories" :key="category.id" :class="{ active: activeView === 'category' && activeCategoryId === category.id }" @click="changeCategory(category.id)">
          <i :style="{ background: category.color }"></i><span>{{ category.name }}</span>
        </button>
      </nav>
      <div class="privacy-card">
        <span>⌂</span>
        <div><strong>数据只在此设备</strong><p>{{ settings.emailEnabled ? '邮件启用时会发送事项与截止时间' : '不上传云端，隐私更安心' }}</p></div>
      </div>
    </aside>

    <main class="workspace">
      <section class="task-panel">
        <div class="page-head">
          <div><h1>{{ title }}</h1><p>{{ subtitle }}</p></div>
          <div class="page-actions">
            <div class="export-actions"><button class="secondary-button" :disabled="exportBusy" title="导出全部未删除事项，包含已完成，CSV 可用 Excel 打开" @click="exportTasks()">{{ exportBusy ? '导出中…' : '一键导出事项' }}</button><button class="plain-button" aria-label="更多导出选项" :aria-expanded="exportMenuOpen" @click="exportMenuOpen = !exportMenuOpen">⌄</button><div v-if="exportMenuOpen" class="export-menu"><button @click="exportTasks('current')">导出当前列表 CSV（{{ tasks.length }} 项）</button><button @click="exportTasks('all', 'json')">导出全部事项 JSON（含图片）</button><p>默认导出全部未删除事项；CSV 包含图片名称，JSON 包含完整图片。</p></div></div>
            <button class="secondary-button" @click="openCreate">＋ 添加任务</button>
          </div>
        </div>

        <div v-if="activeView === 'today'" class="stats-row">
          <article><span>今日待办</span><strong>{{ stats.today }}</strong><small>项任务</small></article>
          <article><span>已完成</span><strong>{{ stats.completedToday }}</strong><small>保持节奏</small></article>
          <article class="progress-card"><span>今日进度</span><strong>{{ progress }}%</strong><div><i :style="{ width: progress + '%' }"></i></div></article>
        </div>

        <div class="list-toolbar">
          <span>{{ tasks.length }} 项</span>
          <button class="plain-button batch-entry" @click="batchMode = !batchMode; batchIds = []">{{ batchMode ? '取消多选' : '批量删除' }}</button>
          <span v-if="stats.overdue && activeView !== 'trash'" class="overdue-summary">{{ stats.overdue }} 项已逾期</span>
        </div>

        <div v-if="batchMode" class="batch-toolbar">
          <label><input type="checkbox" :checked="allChecked" @change="batchIds = allChecked ? [] : tasks.map(task => task.id)" /> 全选当前结果</label>
          <span>已选 {{ batchIds.length }} 项</span>
          <button class="danger-button" :disabled="!batchIds.length || busy" @click="deleteBatch">{{ activeView === 'trash' ? '永久删除选中' : '删除选中' }}</button>
        </div>

        <div class="task-list" :class="{ loading }">
          <article v-for="task in tasks" :key="task.id" class="task-row" :class="[`priority-${task.priority}`, { selected: selectedId === task.id, done: task.status === 'completed' }]" @click="batchMode ? toggleBatch(task.id) : selectedId = task.id">
            <input v-if="batchMode" class="batch-check" type="checkbox" :aria-label="`选择 ${task.title}`" :checked="batchIds.includes(task.id)" @click.stop @change="toggleBatch(task.id)" />
            <button v-else-if="activeView !== 'trash'" class="check" :class="{ checked: task.status === 'completed' }" :aria-label="task.status === 'completed' ? '恢复任务' : '完成任务'" @click.stop="toggleTask(task)">{{ task.status === 'completed' ? '✓' : '' }}</button>
            <span v-else class="trash-dot">♲</span>
            <div class="task-copy">
              <strong>{{ task.title }}</strong>
              <p v-if="task.description">{{ task.description }}</p>
              <div class="task-meta">
                <span v-if="task.categoryName" class="category-tag"><i :style="{ background: task.categoryColor ?? '#999' }"></i>{{ task.categoryName }}</span>
                <span v-if="task.dueAt" :class="{ overdue: isOverdue(task) }">◷ {{ dueLabel(task.dueAt) }}</span>
                <span v-if="task.images?.length">▧ {{ task.images.length }}</span>
                <span v-if="task.localPath">▱ 文件夹</span>
                <span v-if="task.emailReminder">邮件提醒</span>
                <span :class="`priority ${task.priority}`">◆ {{ priorityLabel(task.priority) }}</span>
              </div>
            </div>
            <div v-if="!batchMode" class="row-actions">
              <button v-if="activeView === 'trash'" title="恢复" @click.stop="restoreTask(task)">↶</button>
              <button v-else title="编辑" @click.stop="openEdit(task)">✎</button>
              <button title="删除" @click.stop="removeTask(task)">×</button>
            </div>
          </article>

          <div v-if="!loading && !tasks.length" class="empty-state">
            <span>✓</span><h3>这里很清爽</h3><p>{{ search ? '没有找到匹配的任务' : '暂时没有任务，添加一项开始吧' }}</p>
            <button v-if="activeView !== 'trash'" @click="openCreate">新建任务</button>
          </div>
        </div>
      </section>

      <aside class="focus-panel">
        <template v-if="selectedTask">
          <div class="focus-head"><span>任务详情</span><button @click="selectedId = null">×</button></div>
          <div class="focus-body">
            <span class="focus-status">{{ selectedTask.status === 'completed' ? '已完成' : '待处理' }}</span>
            <h2>{{ selectedTask.title }}</h2>
            <p>{{ selectedTask.description || '暂无任务说明。' }}</p>
            <dl>
              <div><dt>截止时间</dt><dd>{{ dueLabel(selectedTask.dueAt) }}</dd></div>
              <div><dt>优先级</dt><dd>{{ priorityLabel(selectedTask.priority) }}</dd></div>
              <div><dt>分类</dt><dd>{{ selectedTask.categoryName || '收集箱' }}</dd></div>
            </dl>
            <div v-if="selectedTask.images?.length" class="image-grid detail-images">
              <button v-for="image in selectedTask.images" :key="image.id" :title="image.name" @click="previewImage = image.dataUrl"><img :src="image.dataUrl" :alt="image.name" /></button>
            </div>
            <button v-if="selectedTask.localPath" class="directory-link" :title="selectedTask.localPath" @click="openDirectory(selectedTask.localPath)">▱ 打开本地文件夹<br /><small>{{ selectedTask.localPath }}</small></button>
            <p v-if="selectedTask.emailReminder" class="helper">{{ settings.emailEnabled ? '截止时邮件提醒已勾选，需保持电脑和清单运行。' : '任务已勾选邮件提醒，请先在设置中启用 QQ 邮箱提醒。' }}</p>
            <button v-if="activeView !== 'trash'" class="primary-wide" @click="openEdit(selectedTask)">编辑任务</button>
            <button v-if="selectedTask.status !== 'completed' && activeView !== 'trash'" class="ghost-wide" @click="toggleTask(selectedTask)">标记为已完成</button>
            <button v-if="activeView === 'trash'" class="primary-wide" @click="restoreTask(selectedTask)">恢复任务</button>
          </div>
          <div class="focus-tip"><span>✦</span><p><strong>小提示</strong><br />把大任务拆成小步骤，更容易开始。</p></div>
        </template>
        <div v-else class="no-selection"><span>☑</span><h3>选择一个任务</h3><p>在这里查看详情并快速编辑</p></div>
      </aside>
    </main>

    <div v-if="editorOpen" class="modal-backdrop" @click.self="editorOpen = false">
      <form class="modal task-editor" @submit.prevent="submitTask" @paste="pasteImage">
        <div class="modal-head"><div><small>{{ editingTaskId ? '编辑任务' : '新建任务' }}</small><h2>{{ editingTaskId ? '完善任务内容' : '记录要做的事情' }}</h2></div><button type="button" @click="editorOpen = false">×</button></div>
        <label><span>任务名称 *</span><input v-model="form.title" autofocus maxlength="120" placeholder="例如：完成项目周报" /></label>
        <label><span>任务说明</span><textarea v-model="form.description" rows="4" maxlength="1000" placeholder="补充一些细节…"></textarea></label>
        <div class="form-grid">
          <label><span>截止时间</span><input type="datetime-local" :value="toLocalInput(form.dueAt)" @input="form.dueAt = fromLocalInput(($event.target as HTMLInputElement).value)" /></label>
          <label><span>分类</span><select v-model="form.categoryId"><option :value="null">收集箱</option><option v-for="category in categories" :key="category.id" :value="category.id">{{ category.name }}</option></select></label>
        </div>
        <label><span>优先级</span><div class="priority-picker"><button v-for="item in (['low','medium','high'] as Priority[])" :key="item" type="button" :class="[item, { active: form.priority === item }]" @click="form.priority = item">{{ priorityLabel(item) }}</button></div></label>
        <section class="attachment-section">
          <strong>图片 / 进度快照</strong>
          <p class="helper">记录做到一半的状态：上传图片，或截图后在此窗口按 Ctrl+V。最多 6 张，自动压缩保存。</p>
          <label class="upload-button">{{ processingImages ? '正在处理…' : '＋ 添加图片' }}<input type="file" accept="image/png,image/jpeg,image/webp,image/gif,image/bmp" multiple :disabled="processingImages" @change="uploadImages" /></label>
          <div class="image-grid">
            <div v-for="image in form.images" :key="image.id"><button type="button" @click="previewImage = image.dataUrl"><img :src="image.dataUrl" :alt="image.name" /></button><button type="button" class="remove-image" :aria-label="`移除 ${image.name}`" @click="form.images = form.images?.filter(item => item.id !== image.id)">×</button></div>
          </div>
        </section>
        <label><span>关联本地文件夹</span><div class="path-input"><input v-model="form.localPath" placeholder="例如 D:\项目资料" /><button v-if="api.desktop" type="button" class="plain-button" @click="pickDirectory">选择目录</button></div></label>
        <label class="switch-row"><div><strong>截止时邮件提醒</strong><small>{{ api.desktop ? '需在设置中配置 QQ 邮箱；电脑和应用关闭时不发送' : '需要 Windows 桌面版配置 QQ 邮箱' }}</small></div><input v-model="form.emailReminder" type="checkbox" :disabled="!api.desktop" /></label>
        <div class="modal-actions"><button type="button" class="plain-button" @click="editorOpen = false">取消</button><button class="primary-button" :disabled="busy || processingImages">{{ editingTaskId ? '保存更改' : '创建任务' }}</button></div>
      </form>
    </div>

    <div v-if="categoryOpen" class="modal-backdrop" @click.self="categoryOpen = false">
      <section class="modal small-modal">
        <div class="modal-head"><div><small>任务整理</small><h2>分类管理</h2></div><button @click="categoryOpen = false">×</button></div>
        <div class="category-form"><input v-model="categoryDraft.color" type="color" aria-label="分类颜色" /><input v-model="categoryDraft.name" maxlength="30" placeholder="分类名称" @keyup.enter="saveCategory" /><button @click="saveCategory">{{ categoryDraft.id ? '保存' : '添加' }}</button></div>
        <div class="category-list">
          <div v-for="category in categories" :key="category.id"><i :style="{ background: category.color }"></i><span>{{ category.name }}</span><button @click="editCategory(category)">编辑</button><button class="danger" @click="removeCategory(category)">删除</button></div>
        </div>
      </section>
    </div>

    <div v-if="settingsOpen" class="modal-backdrop" @click.self="closeSettings">
      <section class="modal small-modal">
        <div class="modal-head"><div><small>偏好设置</small><h2>应用设置</h2></div><button @click="closeSettings">×</button></div>
        <label><span>启动时打开</span><select v-model="settings.startView"><option value="today">今日任务</option><option value="inbox">收集箱</option><option value="upcoming">即将到期</option><option value="unfinished">未完成</option><option value="all">全部任务</option></select></label>
        <label class="switch-row"><div><strong>每日自动备份</strong><small>桌面版每天首次启动时创建本地备份</small></div><input v-model="settings.dailyBackup" type="checkbox" /></label>
        <section class="email-settings">
          <label class="switch-row"><div><strong>QQ 邮箱提醒</strong><small>用自己的邮箱发给自己；截止后每 30 秒检查一项任务</small></div><input v-model="settings.emailEnabled" type="checkbox" :disabled="!api.desktop || mailBusy" /></label>
          <label><span>QQ 邮箱地址</span><input v-model="settings.emailAddress" type="email" placeholder="你的QQ号@qq.com" :disabled="!api.desktop || mailBusy" @change="emailStatus = { configured: false, sent: 0, failed: 0, uncertain: 0, lastError: null }" /></label>
          <label><span>SMTP 授权码（不是 QQ 登录密码）</span><input v-model="emailCode" type="password" autocomplete="new-password" :placeholder="emailStatus.configured ? '已保存；留空保留，填写则更新' : '在 QQ 邮箱设置中开启 SMTP 并生成授权码'" :disabled="!api.desktop || mailBusy" /></label>
          <p class="helper">电脑打开 mail.qq.com，在设置中查找 POP3/IMAP/SMTP 服务并生成授权码。只在这里填写，不要发到聊天里。授权码由 Windows 凭据管理器保存，不写进任务数据库或导出文件。</p>
          <p class="helper">保持电脑和清单运行；重开仅补发 24 小时内到期事项。手机需在 QQ 邮箱 App 中开启通知。旧短信开关不会自动改为邮件提醒，请给需要的事项重新勾选。</p>
          <div class="email-actions"><button class="secondary-button" :disabled="!api.desktop || mailBusy" @click="testEmail">{{ mailBusy ? '处理中…' : '保存并发送测试邮件' }}</button><button class="plain-button" :disabled="!api.desktop || mailBusy" @click="clearEmailCode">删除已存授权码</button><button v-if="emailStatus.failed" class="plain-button" :disabled="mailBusy" @click="retryFailedEmails">重试失败提醒</button></div>
          <p class="helper" role="status">{{ api.desktop ? (emailStatus.configured ? '授权码已保存' : '此邮箱授权码未配置') : '请使用 Windows 桌面版配置邮件' }}；已受理 {{ emailStatus.sent }} 条，失败 {{ emailStatus.failed }} 条，结果不确定 {{ emailStatus.uncertain }} 条。受理不保证手机提示，请检查收件箱和垃圾邮件。</p>
          <p v-if="emailStatus.lastError" class="helper mail-error">{{ emailStatus.lastError }}</p>
        </section>
        <div class="about-card"><strong>清单 v1.2.1</strong><span>离线优先 · 本地存储 · 无需账号</span></div>
        <div class="modal-actions"><button class="plain-button" @click="closeSettings">取消</button><button class="primary-button" :disabled="mailBusy" @click="saveSettings">保存设置</button></div>
      </section>
    </div>

    <div v-if="backupOpen" class="modal-backdrop" @click.self="backupOpen = false">
      <section class="modal backup-modal">
        <div class="modal-head"><div><small>数据安全</small><h2>备份与恢复</h2></div><button @click="backupOpen = false">×</button></div>
        <div class="backup-callout"><span>⌂</span><p><strong>所有备份都保存在本机</strong><br />恢复前会先保存当前数据，桌面版备份为独立 SQLite 文件。</p><button @click="makeBackup">立即备份</button></div>
        <div class="backup-list"><div v-for="backup in backups" :key="backup.name"><div><strong>{{ backup.name }}</strong><span>{{ new Date(backup.createdAt).toLocaleString('zh-CN') }} · {{ Math.max(1, Math.round(backup.size / 1024)) }} KB</span></div><button @click="restoreBackup(backup)">恢复</button></div><p v-if="!backups.length">还没有备份。</p></div>
      </section>
    </div>

    <div v-if="previewImage" class="image-preview" role="dialog" aria-label="图片预览" @click.self="previewImage = null"><button aria-label="关闭图片预览" @click="previewImage = null">×</button><img :src="previewImage" alt="任务图片大图" /></div>
    <transition name="toast"><div v-if="toast" class="toast">✓ {{ toast }}</div></transition>
  </div>
</template>

