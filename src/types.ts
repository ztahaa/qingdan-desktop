export type Priority = 'low' | 'medium' | 'high'
export type TaskStatus = 'todo' | 'completed'
export type ViewKey = 'today' | 'inbox' | 'upcoming' | 'all' | 'completed' | 'trash'

export interface Category {
  id: string
  name: string
  color: string
  sortOrder: number
}

export interface Task {
  id: string
  title: string
  description: string
  status: TaskStatus
  priority: Priority
  dueAt: string | null
  categoryId: string | null
  categoryName: string | null
  categoryColor: string | null
  createdAt: string
  updatedAt: string
  completedAt: string | null
  deletedAt: string | null
}

export interface TaskInput {
  title: string
  description: string
  priority: Priority
  dueAt: string | null
  categoryId: string | null
}

export interface TaskFilter {
  view: ViewKey | 'category'
  search: string
  categoryId?: string | null
}

export interface Stats {
  today: number
  inbox: number
  upcoming: number
  all: number
  completed: number
  trash: number
  overdue: number
  completedToday: number
}

export interface BackupInfo {
  name: string
  path: string
  createdAt: string
  size: number
}

export interface AppSettings {
  startView: ViewKey
  theme: 'light' | 'system'
  dailyBackup: boolean
}
