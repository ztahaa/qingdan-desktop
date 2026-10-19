import { test } from 'node:test'
import assert from 'node:assert/strict'
import { mkdir, writeFile, rm } from 'node:fs/promises'
import { Window } from 'happy-dom'

test('old data, filters, selection deletion and attachment backup round-trip', async () => {
  const directory = new URL('../.qa/', import.meta.url)
  await mkdir(directory, { recursive: true })
  await writeFile(new URL('entry.ts', directory), "export { default as App } from '../src/App.vue'; export { api } from '../src/services'; export { tasksCsv, tasksJson } from '../src/export';")
  const window = new Window({ url: 'http://localhost:1420' })
  for (const key of ['window', 'document', 'localStorage', 'HTMLElement', 'Element', 'Document', 'ShadowRoot', 'SVGElement', 'Node', 'Event', 'MouseEvent', 'location']) {
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
  const { App, api, tasksCsv, tasksJson } = await import(new URL('output/app.mjs', directory).href)
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
    assert.ok(container.textContent.includes('截止时邮件提醒'))
    assert.ok(!container.textContent.includes('截止时短信提醒'))
    const csvTask = { ...restored, title: '=HYPERLINK("bad")', description: '含逗号,和换行\n第二行', emailReminder: true }
    const csv = tasksCsv([csvTask])
    assert.equal(csv.charCodeAt(0), 0xFEFF)
    assert.ok(csv.includes("'=HYPERLINK("))
    assert.ok(csv.includes('"含逗号,和换行\n第二行"'))
    assert.ok(!csv.includes(image.dataUrl))
    const exported = JSON.parse(tasksJson([restored]))
    assert.equal(exported.count, 1)
    assert.deepEqual(exported.tasks[0].images, [image])
    assert.ok(!Object.hasOwn(exported, 'settings'))
    container.querySelector('.task-editor .modal-head button').click()
    await flush()
    let captured = null
    api.exportFile = async (content, format) => { captured = { content, format }; return true }
    await click('一键导出事项')
    assert.equal(captured.format, 'csv')
    // All nondeleted tasks are exported regardless of the currently selected unfinished view.
    assert.ok(captured.content.includes('旧版已完成'))
    assert.ok(!captured.content.includes('旧版回收站'))
    container.querySelector('button[title="设置"]').click()
    await flush()
    assert.ok(container.textContent.includes('QQ 邮箱提醒'))
    assert.ok(!container.textContent.includes('手机短信服务'))
    assert.ok(container.querySelector('input[type="password"]').disabled)
    // Exercise the desktop configuration UI with a fake sender; no credentials or emails leave this test.
    container.querySelector('.small-modal .modal-head button').click()
    await flush()
    api.desktop = true
    let savedCode = null
    let tested = 0
    api.emailStatus = async () => ({ configured: Boolean(savedCode), sent: 0, failed: 0, uncertain: 0, lastError: null })
    api.saveEmailCode = async (address, code) => { savedCode = { address, code } }
    api.testEmail = async () => { tested++; return '测试服务已受理' }
    container.querySelector('button[title="设置"]').click()
    await flush()
    const addressInput = container.querySelector('input[type="email"]')
    addressInput.value = '123456@qq.com'
    addressInput.dispatchEvent(new window.Event('input', { bubbles: true }))
    const secret = 'unitTESTcodeONLY1'
    const passwordInput = container.querySelector('input[type="password"]')
    passwordInput.value = secret
    passwordInput.dispatchEvent(new window.Event('input', { bubbles: true }))
    const enableMail = container.querySelector('.email-settings input[type="checkbox"]')
    enableMail.checked = true
    enableMail.dispatchEvent(new window.Event('change', { bubbles: true }))
    await flush()
    await click('保存并发送测试邮件')
    assert.equal(tested, 1)
    assert.deepEqual(savedCode, { address: '123456@qq.com', code: secret })
    assert.equal(container.querySelector('input[type="password"]').value, '')
    assert.ok(!localStorage.getItem('qingdan.settings.v1').includes(secret))
    assert.ok(!tasksJson([restored]).includes(secret))
    api.desktop = false


  } finally {
    app.unmount()
    window.happyDOM.abort()
    await rm(directory, { recursive: true, force: true })
  }
})
