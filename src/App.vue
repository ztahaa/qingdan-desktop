<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { api } from './services'
import type { AppSettings, BackupInfo, Category, Priority, Stats, Task, TaskInput, ViewKey } from './types'

const tasks = ref<Task[]>([])
const categories = ref<Category[]>([])
const stats = ref<Stats>({ today: 0, inbox: 0, upcoming: 0, all: 0, completed: 0, trash: 0, overdue: 0, completedToday: 0 })
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
const categoryDraft = ref({ id: '', name: '', color: '#5B8FF9' })
const form = ref<TaskInput>({ title: '', description: '', priority: 'medium', dueAt: null, categoryId: null })

const navItems: { key: ViewKey; label: string; icon: string; count: keyof Stats }[] = [
  { key: 'today', label: '今日任务', icon: '☀', count: 'today' },
  { key: 'inbox', label: '收集箱', icon: '▣', count: 'inbox' },
  { key: 'upcoming', label: '即将到期', icon: '◷', count: 'upcoming' },
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
  activeView.value = view
  activeCategoryId.value = null
}

function changeCategory(categoryId: string) {
  activeView.value = 'category'
  activeCategoryId.value = categoryId
}

function openCreate() {
  editingTaskId.value = null
  form.value = { title: '', description: '', priority: 'medium', dueAt: activeView.value === 'today' ? new Date(new Date().setHours(18, 0, 0, 0)).toISOString() : null, categoryId: activeView.value === 'category' ? activeCategoryId.value : null }
  editorOpen.value = true
}

function openEdit(task: Task) {
  editingTaskId.value = task.id
  form.value = { title: task.title, description: task.description, priority: task.priority, dueAt: task.dueAt, categoryId: task.categoryId }
  editorOpen.value = true
}

async function submitTask() {
  if (!form.value.title.trim()) return flash('请输入任务名称')
  const input = { ...form.value, title: form.value.title.trim(), description: form.value.description.trim() }
  if (editingTaskId.value) await api.updateTask(editingTaskId.value, input)
  else await api.createTask(input)
  editorOpen.value = false
  flash(editingTaskId.value ? '任务已更新' : '任务已创建')
  await refresh()
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

async function saveSettings() {
  await api.saveSettings(settings.value)
  settingsOpen.value = false
  flash('设置已保存')
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
        <button class="icon-button" title="设置" @click="settingsOpen = true">⚙</button>
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
        <div><strong>数据只在此设备</strong><p>不上传云端，隐私更安心</p></div>
      </div>
    </aside>

    <main class="workspace">
      <section class="task-panel">
        <div class="page-head">
          <div><h1>{{ title }}</h1><p>{{ subtitle }}</p></div>
          <button class="secondary-button" @click="openCreate">＋ 添加任务</button>
        </div>

        <div v-if="activeView === 'today'" class="stats-row">
          <article><span>今日待办</span><strong>{{ stats.today }}</strong><small>项任务</small></article>
          <article><span>已完成</span><strong>{{ stats.completedToday }}</strong><small>保持节奏</small></article>
          <article class="progress-card"><span>今日进度</span><strong>{{ progress }}%</strong><div><i :style="{ width: progress + '%' }"></i></div></article>
        </div>

        <div class="list-toolbar">
          <span>{{ tasks.length }} 项</span>
          <span v-if="stats.overdue && activeView !== 'trash'" class="overdue-summary">{{ stats.overdue }} 项已逾期</span>
        </div>

        <div class="task-list" :class="{ loading }">
          <article v-for="task in tasks" :key="task.id" class="task-row" :class="{ selected: selectedId === task.id, done: task.status === 'completed' }" @click="selectedId = task.id">
            <button v-if="activeView !== 'trash'" class="check" :class="{ checked: task.status === 'completed' }" :aria-label="task.status === 'completed' ? '恢复任务' : '完成任务'" @click.stop="toggleTask(task)">{{ task.status === 'completed' ? '✓' : '' }}</button>
            <span v-else class="trash-dot">♲</span>
            <div class="task-copy">
              <strong>{{ task.title }}</strong>
              <p v-if="task.description">{{ task.description }}</p>
              <div class="task-meta">
                <span v-if="task.categoryName" class="category-tag"><i :style="{ background: task.categoryColor ?? '#999' }"></i>{{ task.categoryName }}</span>
                <span v-if="task.dueAt" :class="{ overdue: isOverdue(task) }">◷ {{ dueLabel(task.dueAt) }}</span>
                <span :class="`priority ${task.priority}`">◆ {{ priorityLabel(task.priority) }}</span>
              </div>
            </div>
            <div class="row-actions">
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
      <form class="modal task-editor" @submit.prevent="submitTask">
        <div class="modal-head"><div><small>{{ editingTaskId ? '编辑任务' : '新建任务' }}</small><h2>{{ editingTaskId ? '完善任务内容' : '记录要做的事情' }}</h2></div><button type="button" @click="editorOpen = false">×</button></div>
        <label><span>任务名称 *</span><input v-model="form.title" autofocus maxlength="120" placeholder="例如：完成项目周报" /></label>
        <label><span>任务说明</span><textarea v-model="form.description" rows="4" maxlength="1000" placeholder="补充一些细节…"></textarea></label>
        <div class="form-grid">
          <label><span>截止时间</span><input type="datetime-local" :value="toLocalInput(form.dueAt)" @input="form.dueAt = fromLocalInput(($event.target as HTMLInputElement).value)" /></label>
          <label><span>分类</span><select v-model="form.categoryId"><option :value="null">收集箱</option><option v-for="category in categories" :key="category.id" :value="category.id">{{ category.name }}</option></select></label>
        </div>
        <label><span>优先级</span><div class="priority-picker"><button v-for="item in (['low','medium','high'] as Priority[])" :key="item" type="button" :class="[item, { active: form.priority === item }]" @click="form.priority = item">{{ priorityLabel(item) }}</button></div></label>
        <div class="modal-actions"><button type="button" class="plain-button" @click="editorOpen = false">取消</button><button class="primary-button">{{ editingTaskId ? '保存更改' : '创建任务' }}</button></div>
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

    <div v-if="settingsOpen" class="modal-backdrop" @click.self="settingsOpen = false">
      <section class="modal small-modal">
        <div class="modal-head"><div><small>偏好设置</small><h2>应用设置</h2></div><button @click="settingsOpen = false">×</button></div>
        <label><span>启动时打开</span><select v-model="settings.startView"><option value="today">今日任务</option><option value="inbox">收集箱</option><option value="upcoming">即将到期</option><option value="all">全部任务</option></select></label>
        <label class="switch-row"><div><strong>每日自动备份</strong><small>桌面版每天首次启动时创建本地备份</small></div><input v-model="settings.dailyBackup" type="checkbox" /></label>
        <div class="about-card"><strong>清单 v1.0.0</strong><span>离线优先 · 本地存储 · 无需账号</span></div>
        <div class="modal-actions"><button class="plain-button" @click="settingsOpen = false">取消</button><button class="primary-button" @click="saveSettings">保存设置</button></div>
      </section>
    </div>

    <div v-if="backupOpen" class="modal-backdrop" @click.self="backupOpen = false">
      <section class="modal backup-modal">
        <div class="modal-head"><div><small>数据安全</small><h2>备份与恢复</h2></div><button @click="backupOpen = false">×</button></div>
        <div class="backup-callout"><span>⌂</span><p><strong>所有备份都保存在本机</strong><br />恢复前会先保存当前数据，桌面版备份为独立 SQLite 文件。</p><button @click="makeBackup">立即备份</button></div>
        <div class="backup-list"><div v-for="backup in backups" :key="backup.name"><div><strong>{{ backup.name }}</strong><span>{{ new Date(backup.createdAt).toLocaleString('zh-CN') }} · {{ Math.max(1, Math.round(backup.size / 1024)) }} KB</span></div><button @click="restoreBackup(backup)">恢复</button></div><p v-if="!backups.length">还没有备份。</p></div>
      </section>
    </div>

    <transition name="toast"><div v-if="toast" class="toast">✓ {{ toast }}</div></transition>
  </div>
</template>
