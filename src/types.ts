export type Priority = 'low' | 'medium' | 'high'
export type TaskStatus = 'todo' | 'completed'
export type ViewKey = 'today' | 'inbox' | 'upcoming' | 'all' | 'unfinished' | 'completed' | 'trash'

export interface Category {
  id: string
  name: string
  color: string
  sortOrder: number
}

export interface TaskImage {
  id: string
  name: string
  dataUrl: string
  createdAt: string
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
  images?: TaskImage[]
  localPath?: string | null
  smsReminder?: boolean
  emailReminder?: boolean
}

export interface TaskInput {
  title: string
  description: string
  priority: Priority
  dueAt: string | null
  categoryId: string | null
  images?: TaskImage[]
  localPath?: string | null
  smsReminder?: boolean
  emailReminder?: boolean
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
  unfinished: number
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
  emailEnabled?: boolean
  emailAddress?: string
  smsEnabled?: boolean
  smsPhone?: string
  smsWebhookUrl?: string
}


export interface EmailStatus {
  configured: boolean
  sent: number
  failed: number
  uncertain: number
  lastError: string | null
}
