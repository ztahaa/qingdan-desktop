import { test } from 'node:test'
import assert from 'node:assert/strict'
import { mkdir, writeFile, rm } from 'node:fs/promises'
import { Window } from 'happy-dom'

test('old data, filters, selection deletion and attachment backup round-trip', async () => {
  const directory = new URL('../.qa/', import.meta.url)
  await mkdir(directory, { recursive: true })
  await writeFile(new URL('entry.ts', directory), "export { default as App } from '../src/App.vue'; export { api } from '../src/services';")
  const window = new Window({ url: 'http://localhost:1420' })
  for (const key of ['window', 'document', 'localStorage', 'HTMLElement', 'Element', 'SVGElement', 'Node', 'Event', 'MouseEvent', 'location']) {
    Object.defineProperty(globalThis, key, { value: key === 'window' ? window : window[key], configurable: true })
  }
  globalThis.requestAnimationFrame = window.requestAnimationFrame.bind(window)
  globalThis.cancelAnimationFrame = window.cancelAnimationFrame.bind(window)
  globalThis.getComputedStyle = window.getComputedStyle.bind(window)
  const { build } = await import('vite')
  await build({
    logLevel: 'error',
    build: {
      outDir: '.qa/output',
      lib: { entry: '.qa/entry.ts', formats: ['es'], fileName: () => 'app.mjs' },
      rollupOptions: { external: ['vue'] }
    }
  })
  window.confirm = () => true
  globalThis.confirm = window.confirm
  const stamp = new Date().toISOString()
  const oldTask = (id, title, status = 'todo', deletedAt = null) => ({
    id, title, description: '', status, priority: id === 'one' ? 'high' : 'low',
    dueAt: null, categoryId: null, categoryName: null, categoryColor: null,
    createdAt: stamp, updatedAt: stamp, completedAt: status === 'completed' ? stamp : null, deletedAt
  })
  localStorage.setItem('qingdan.tasks.v1', JSON.stringify([
    oldTask('one', '旧版任务一'), oldTask('two', '旧版任务二'),
    oldTask('done', '旧版已完成', 'completed'), oldTask('deleted', '旧版回收站', 'todo', stamp)
  ]))
  localStorage.setItem('qingdan.categories.v1', '[]')
  localStorage.setItem('qingdan.settings.v1', JSON.stringify({ startView: 'unfinished', theme: 'light', dailyBackup: true }))
  const { createApp, nextTick } = await import('vue')
  const { App, api } = await import(new URL('output/app.mjs', directory).href)
  const container = document.createElement('div')
  document.body.append(container)
  const app = createApp(App)
  app.mount(container)
  const flush = async () => { for (let i = 0; i < 12; i++) { await Promise.resolve(); await nextTick() } }
  const click = async text => {
    const button = [...container.querySelectorAll('button')].find(button => button.textContent.trim() === text)
    assert.ok(button, `button ${text}`)
    button.click()
    await flush()
  }
  const nav = async label => {
    const button = [...container.querySelectorAll('.main-nav button')].find(button => button.textContent.includes(label))
    assert.ok(button, `navigation ${label}`)
    button.click()
    await flush()
  }
  try {
    await flush()
    assert.equal(container.querySelector('h1').textContent, '未完成')
    assert.equal(container.querySelectorAll('.task-row').length, 2)
    assert.ok(container.querySelector('.task-row.priority-high'))
    await nav('全部任务')
    assert.equal(container.querySelectorAll('.task-row').length, 3)
    await click('批量删除')
    const first = container.querySelector('.batch-check')
    first.checked = true
    first.dispatchEvent(new window.Event('change', { bubbles: true }))
    await flush()
    await click('删除选中')
    assert.equal(container.querySelectorAll('.task-row').length, 2)
    assert.equal((await api.listTasks({ view: 'trash', search: '' })).length, 2)
    await api.restoreTask('one')
    assert.equal((await api.listTasks({ view: 'unfinished', search: '' })).length, 2)
    assert.equal((await api.listTasks({ view: 'all', search: '旧版已完成' })).length, 1)
    const image = { id: 'snapshot', name: '中途快照.jpg', dataUrl: 'data:image/jpeg;base64,aA==', createdAt: stamp }
    const task = await api.createTask({ title: '带附件任务', description: '记录进度', priority: 'medium', dueAt: null, categoryId: null, images: [image], localPath: 'D:\\项目资料', smsReminder: false })
    const backup = await api.createBackup()
    await api.deleteTasks([task.id])
    assert.equal((await api.listTasks({ view: 'all', search: '带附件任务' })).length, 0)
    await api.restoreBackup(backup.name)
    const restored = (await api.listTasks({ view: 'all', search: '带附件任务' }))[0]
    assert.deepEqual(restored.images, [image])
    assert.equal(restored.localPath, 'D:\\项目资料')
    assert.equal((await api.getStats()).unfinished, 3)
    // The v1 storage namespace is retained, and no example data was inserted into empty user categories.
    assert.ok(JSON.parse(localStorage.getItem('qingdan.tasks.v1')).some(task => task.id === 'one'))
    assert.deepEqual(await api.listCategories(), [])
    await nav('未完成')
    container.querySelector('.create-button').click()
    await flush()
    assert.ok(container.textContent.includes('图片 / 进度快照'))
    assert.ok(container.querySelector('input[type=file][multiple]'))
  } finally {
    app.unmount()
    window.happyDOM.abort()
    await rm(directory, { recursive: true, force: true })
  }
})
