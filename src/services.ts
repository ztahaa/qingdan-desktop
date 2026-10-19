import type { AppSettings, BackupInfo, Category, EmailStatus, Stats, Task, TaskFilter, TaskInput } from './types'

const TASK_KEY = 'qingdan.tasks.v1'
const CATEGORY_KEY = 'qingdan.categories.v1'
const SETTINGS_KEY = 'qingdan.settings.v1'
const BACKUP_KEY = 'qingdan.backups.v1'

const now = () => new Date().toISOString()
const id = () => crypto.randomUUID()
const isDesktop = () => Boolean(window.__TAURI_INTERNALS__)
const isPortableServer = () => location.hostname === '127.0.0.1' && location.port === '38421'
let portableReady: Promise<void> | null = null

const seedCategories: Category[] = [
  { id: 'cat-work', name: '工作', color: '#5B8FF9', sortOrder: 0 },
  { id: 'cat-life', name: '生活', color: '#57C6A1', sortOrder: 1 },
  { id: 'cat-study', name: '学习', color: '#A078E8', sortOrder: 2 }
]

const dateAt = (days: number, hour: number) => {
  const d = new Date()
  d.setDate(d.getDate() + days)
  d.setHours(hour, 0, 0, 0)
  return d.toISOString()
}

const seedTasks: Task[] = [
  {
    id: 'seed-1', title: '完成项目周报', description: '整理本周进展、风险和下周计划', status: 'todo',
    priority: 'high', dueAt: dateAt(0, 18), categoryId: 'cat-work', categoryName: '工作', categoryColor: '#5B8FF9',
    createdAt: now(), updatedAt: now(), completedAt: null, deletedAt: null
  },
  {
    id: 'seed-2', title: '预约体检', description: '确认周末可预约时间', status: 'todo',
    priority: 'medium', dueAt: dateAt(1, 10), categoryId: 'cat-life', categoryName: '生活', categoryColor: '#57C6A1',
    createdAt: now(), updatedAt: now(), completedAt: null, deletedAt: null
  },
  {
    id: 'seed-3', title: '学习 Vue 组合式 API', description: '完成响应式状态和组件通信章节', status: 'todo',
    priority: 'low', dueAt: dateAt(3, 20), categoryId: 'cat-study', categoryName: '学习', categoryColor: '#A078E8',
    createdAt: now(), updatedAt: now(), completedAt: null, deletedAt: null
  },
  {
    id: 'seed-4', title: '整理桌面文件', description: '', status: 'completed',
    priority: 'low', dueAt: null, categoryId: null, categoryName: null, categoryColor: null,
    createdAt: now(), updatedAt: now(), completedAt: now(), deletedAt: null
  }
]

const read = <T>(key: string, fallback: T): T => {
  const value = localStorage.getItem(key)
  if (!value) {
    localStorage.setItem(key, JSON.stringify(fallback))
    return structuredClone(fallback)
  }
  try { return JSON.parse(value) as T } catch { return structuredClone(fallback) }
}

const write = <T>(key: string, value: T) => localStorage.setItem(key, JSON.stringify(value))

async function ensurePortable() {
  if (!isPortableServer()) return
  if (!portableReady) {
    portableReady = fetch('/api/state', { cache: 'no-store' }).then(async response => {
      if (!response.ok) throw new Error(`本地数据服务返回 ${response.status}`)
      const data = await response.json()
      if (Array.isArray(data.tasks)) localStorage.setItem(TASK_KEY, JSON.stringify(data.tasks))
      if (Array.isArray(data.categories)) localStorage.setItem(CATEGORY_KEY, JSON.stringify(data.categories))
      if (data.settings) localStorage.setItem(SETTINGS_KEY, JSON.stringify(data.settings))
      if (Array.isArray(data.backups)) localStorage.setItem(BACKUP_KEY, JSON.stringify(data.backups))
    })
  }
  return portableReady
}

async function persistPortable() {
  if (!isPortableServer()) return
  const payload = {
    tasks: read<Task[]>(TASK_KEY, seedTasks),
    categories: read<Category[]>(CATEGORY_KEY, seedCategories),
    settings: read<AppSettings>(SETTINGS_KEY, { startView: 'today', theme: 'light', dailyBackup: true }),
    backups: read<(BackupInfo & { snapshot: string })[]>(BACKUP_KEY, [])
  }
  const response = await fetch('/api/state', {
    method: 'PUT',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(payload)
  })
  if (!response.ok) throw new Error(`本地数据保存失败：${response.status}`)
}

async function command<T>(name: string, args: Record<string, unknown> = {}): Promise<T> {
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<T>(name, args)
}

const categoriesLocal = () => read<Category[]>(CATEGORY_KEY, seedCategories)
const tasksLocal = () => read<Task[]>(TASK_KEY, seedTasks)

function hydrateCategory(task: Task, categories = categoriesLocal()): Task {
  const category = categories.find(item => item.id === task.categoryId)
  return { ...task, categoryName: category?.name ?? null, categoryColor: category?.color ?? null }
}

function isSameDay(a: Date, b: Date) {
  return a.getFullYear() === b.getFullYear() && a.getMonth() === b.getMonth() && a.getDate() === b.getDate()
}

function matchesFilter(task: Task, filter: TaskFilter) {
  const today = new Date()
  const due = task.dueAt ? new Date(task.dueAt) : null
  const active = !task.deletedAt && task.status === 'todo'
  if (filter.view === 'today' && !(active && due && isSameDay(due, today))) return false
  if (filter.view === 'inbox' && !(active && !task.categoryId)) return false
  if (filter.view === 'upcoming') {
    const end = new Date(today)
    end.setDate(end.getDate() + 7)
    end.setHours(23, 59, 59, 999)
    if (!(active && due && due > today && due <= end)) return false
  }
  if (filter.view === 'unfinished' && !active) return false
  if (filter.view === 'all' && task.deletedAt) return false
  if (filter.view === 'completed' && !(task.status === 'completed' && !task.deletedAt)) return false
  if (filter.view === 'trash' && !task.deletedAt) return false
  if (filter.view === 'category' && !(active && task.categoryId === filter.categoryId)) return false
  const q = filter.search.trim().toLocaleLowerCase()
  return !q || `${task.title} ${task.description} ${task.categoryName ?? ''}`.toLocaleLowerCase().includes(q)
}

export const api = {
  desktop: isDesktop(),
  portable: isPortableServer(),

  async listTasks(filter: TaskFilter): Promise<Task[]> {
    if (isDesktop()) return command('task_list', { filter })
    await ensurePortable()
    return tasksLocal().map(task => hydrateCategory(task)).filter(task => matchesFilter(task, filter))
      .sort((a, b) => Number(Boolean(a.completedAt)) - Number(Boolean(b.completedAt)) || (a.dueAt ?? '9999').localeCompare(b.dueAt ?? '9999'))
  },

  async createTask(input: TaskInput): Promise<Task> {
    if (isDesktop()) return command('task_create', { input })
    await ensurePortable()
    const task = hydrateCategory({ id: id(), ...input, status: 'todo', createdAt: now(), updatedAt: now(), completedAt: null, deletedAt: null, categoryName: null, categoryColor: null })
    write(TASK_KEY, [...tasksLocal(), task])
    await persistPortable()
    return task
  },

  async updateTask(taskId: string, input: TaskInput): Promise<Task> {
    if (isDesktop()) return command('task_update', { taskId, input })
    await ensurePortable()
    let updated!: Task
    write(TASK_KEY, tasksLocal().map(task => task.id === taskId ? (updated = hydrateCategory({ ...task, ...input, updatedAt: now() })) : task))
    await persistPortable()
    return updated
  },

  async setCompleted(taskId: string, completed: boolean): Promise<void> {
    if (isDesktop()) return command('task_complete', { taskId, completed })
    await ensurePortable()
    write(TASK_KEY, tasksLocal().map(task => task.id === taskId ? { ...task, status: completed ? 'completed' : 'todo', completedAt: completed ? now() : null, updatedAt: now() } : task))
    await persistPortable()
  },

  async deleteTask(taskId: string, permanent = false): Promise<void> {
    if (isDesktop()) return command('task_delete', { taskId, permanent })
    await ensurePortable()
    write(TASK_KEY, permanent ? tasksLocal().filter(task => task.id !== taskId) : tasksLocal().map(task => task.id === taskId ? { ...task, deletedAt: now(), updatedAt: now() } : task))
    await persistPortable()
  },

  async deleteTasks(taskIds: string[], permanent = false): Promise<void> {
    if (isDesktop()) return command('task_delete_many', { taskIds, permanent })
    await ensurePortable()
    const ids = new Set(taskIds)
    const stamp = now()
    write(TASK_KEY, tasksLocal().flatMap(task => {
      if (!ids.has(task.id)) return [task]
      if (permanent) {
        if (!task.deletedAt) throw new Error('只能永久删除回收站中的任务')
        return []
      }
      return [{ ...task, deletedAt: stamp, updatedAt: stamp }]
    }))
    await persistPortable()
  },

  async pickDirectory(): Promise<string | null> {
    if (!isDesktop()) throw new Error('请手动输入文件夹的完整路径')
    return command('directory_pick')
  },

  async openDirectory(path: string): Promise<void> {
    if (isDesktop()) return command('directory_open', { path })
    if (!isPortableServer()) throw new Error('请使用 Windows 桌面版打开本地文件夹')
    const response = await fetch('/api/open-directory', {
      method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ path })
    })
    if (!response.ok) throw new Error('文件夹不存在或无法打开')
  },

  async emailStatus(address = ''): Promise<EmailStatus> {
    if (!isDesktop()) return { configured: false, sent: 0, failed: 0, uncertain: 0, lastError: null }
    return command('email_status', { address })
  },

  async saveEmailCode(address: string, authCode: string): Promise<void> {
    if (!isDesktop()) throw new Error('请使用 Windows 桌面版配置邮件提醒')
    return command('email_auth_save', { address, authCode })
  },

  async clearEmailCode(address: string): Promise<void> {
    if (!isDesktop()) throw new Error('请使用 Windows 桌面版管理授权码')
    return command('email_auth_clear', { address })
  },

  async testEmail(): Promise<string> {
    if (!isDesktop()) throw new Error('请使用 Windows 桌面版发送测试邮件')
    return command('email_test')
  },

  async retryFailedEmails(): Promise<number> {
    if (!isDesktop()) throw new Error('请使用 Windows 桌面版重试邮件')
    return command('email_retry_failed')
  },

  async exportFile(content: string, format: 'csv' | 'json'): Promise<boolean> {
    if (isDesktop()) return Boolean(await command<string | null>('tasks_export_save', { content, format }))
    const blob = new Blob([content], { type: format === 'csv' ? 'text/csv;charset=utf-8' : 'application/json;charset=utf-8' })
    const url = URL.createObjectURL(blob)
    const link = document.createElement('a')
    link.href = url
    link.download = `清单事项-${new Date().toISOString().replace(/[:.]/g, '-')}.${format}`
    document.body.append(link)
    link.click()
    link.remove()
    window.setTimeout(() => URL.revokeObjectURL(url), 1000)
    return true
  },

  async restoreTask(taskId: string): Promise<void> {
    if (isDesktop()) return command('task_restore', { taskId })
    await ensurePortable()
    write(TASK_KEY, tasksLocal().map(task => task.id === taskId ? { ...task, deletedAt: null, updatedAt: now() } : task))
    await persistPortable()
  },

  async getStats(): Promise<Stats> {
    if (isDesktop()) return command('stats_get')
    await ensurePortable()
    const tasks = tasksLocal()
    const current = new Date()
    const end = new Date(current)
    end.setDate(end.getDate() + 7)
    end.setHours(23, 59, 59, 999)
    const active = tasks.filter(task => !task.deletedAt && task.status === 'todo')
    return {
      today: active.filter(task => task.dueAt && isSameDay(new Date(task.dueAt), current)).length,
      inbox: active.filter(task => !task.categoryId).length,
      upcoming: active.filter(task => task.dueAt && new Date(task.dueAt) > current && new Date(task.dueAt) <= end).length,
      all: tasks.filter(task => !task.deletedAt).length,
      unfinished: active.length,
      completed: tasks.filter(task => !task.deletedAt && task.status === 'completed').length,
      trash: tasks.filter(task => task.deletedAt).length,
      overdue: active.filter(task => task.dueAt && new Date(task.dueAt) < current && !isSameDay(new Date(task.dueAt), current)).length,
      completedToday: tasks.filter(task => task.completedAt && isSameDay(new Date(task.completedAt), current)).length
    }
  },

  async listCategories(): Promise<Category[]> {
    if (isDesktop()) return command('category_list')
    await ensurePortable()
    return categoriesLocal().sort((a, b) => a.sortOrder - b.sortOrder)
  },

  async saveCategory(category: Partial<Category> & Pick<Category, 'name' | 'color'>): Promise<Category> {
    if (isDesktop()) return command('category_save', { input: category })
    await ensurePortable()
    const all = categoriesLocal()
    const found = category.id ? all.find(item => item.id === category.id) : undefined
    const saved = { id: found?.id ?? id(), name: category.name, color: category.color, sortOrder: found?.sortOrder ?? all.length }
    write(CATEGORY_KEY, found ? all.map(item => item.id === saved.id ? saved : item) : [...all, saved])
    await persistPortable()
    return saved
  },

  async deleteCategory(categoryId: string): Promise<void> {
    if (isDesktop()) return command('category_delete', { categoryId })
    await ensurePortable()
    write(CATEGORY_KEY, categoriesLocal().filter(item => item.id !== categoryId))
    write(TASK_KEY, tasksLocal().map(task => task.categoryId === categoryId ? { ...task, categoryId: null, categoryName: null, categoryColor: null } : task))
    await persistPortable()
  },

  async getSettings(): Promise<AppSettings> {
    if (isDesktop()) return command('settings_get')
    await ensurePortable()
    return read(SETTINGS_KEY, { startView: 'today', theme: 'light', dailyBackup: true } as AppSettings)
  },

  async saveSettings(settings: AppSettings): Promise<void> {
    if (isDesktop()) return command('settings_save', { settings })
    await ensurePortable()
    write(SETTINGS_KEY, settings)
    await persistPortable()
  },

  async createBackup(): Promise<BackupInfo> {
    if (isDesktop()) return command('backup_create')
    await ensurePortable()
    const snapshot = JSON.stringify({ tasks: tasksLocal(), categories: categoriesLocal(), settings: await this.getSettings() })
    const info: BackupInfo & { snapshot: string } = { name: `清单备份-${new Date().toISOString().replace(/[:.]/g, '-')}.json`, path: '浏览器本地存储', createdAt: now(), size: new Blob([snapshot]).size, snapshot }
    const backups = read<(BackupInfo & { snapshot: string })[]>(BACKUP_KEY, [])
    write(BACKUP_KEY, [info, ...backups].slice(0, 10))
    await persistPortable()
    return info
  },

  async listBackups(): Promise<BackupInfo[]> {
    if (isDesktop()) return command('backup_list')
    await ensurePortable()
    return read<(BackupInfo & { snapshot: string })[]>(BACKUP_KEY, [])
  },

  async restoreBackup(path: string): Promise<void> {
    if (isDesktop()) return command('backup_restore', { path })
    await ensurePortable()
    const backup = read<(BackupInfo & { snapshot: string })[]>(BACKUP_KEY, []).find(item => item.name === path || item.path === path)
    if (!backup) throw new Error('找不到备份')
    const data = JSON.parse(backup.snapshot)
    await this.createBackup()
    write(TASK_KEY, data.tasks)
    write(CATEGORY_KEY, data.categories)
    write(SETTINGS_KEY, data.settings)
    await persistPortable()
  },

  async resetDemoData(): Promise<void> {
    if (isDesktop()) return command('demo_reset')
    await ensurePortable()
    write(TASK_KEY, seedTasks)
    write(CATEGORY_KEY, seedCategories)
    await persistPortable()
  }
}

