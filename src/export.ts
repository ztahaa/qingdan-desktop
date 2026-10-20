import type { Task } from './types'

function cell(value: unknown): string {
  let text = value == null ? '' : String(value)
  // Prevent task text being interpreted as a spreadsheet formula, including prefixed whitespace.
  if (/^\s*[=+@-]/.test(text) || /^[\t\r\n]/.test(text)) text = "'" + text
  return '"' + text.replace(/"/g, '""') + '"'
}

export function tasksCsv(tasks: Task[]): string {
  const headers = ['事项ID', '事项名称', '事项说明', '完成状态', '优先级', '截止时间（ISO）', '分类', '分类ID', '分类颜色', '创建时间（ISO）', '更新时间（ISO）', '完成时间（ISO）', '删除时间（ISO）', '邮件提醒', '本地目录', '图片数量', '图片名称']
  const rows = tasks.map(task => [task.id, task.title, task.description, task.status === 'completed' ? '已完成' : '未完成', ({ high: '高', medium: '中', low: '低' })[task.priority], task.dueAt, task.categoryName || '收集箱', task.categoryId, task.categoryColor, task.createdAt, task.updatedAt, task.completedAt, task.deletedAt, task.emailReminder ? '是' : '否', task.localPath, task.images?.length ?? 0, task.images?.map(image => image.name).join('；') ?? ''])
  return '\uFEFF' + [headers, ...rows].map(row => row.map(cell).join(',')).join('\r\n') + '\r\n'
}

export function tasksJson(tasks: Task[]): string {
  // Only tasks are exported: no application settings, mailbox credentials, or authorization codes.
  return JSON.stringify({ format: 'qingdan.tasks', version: 1, exportedAt: new Date().toISOString(), count: tasks.length, tasks }, null, 2)
}
