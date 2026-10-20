import type { TaskImage } from './types'

export const MAX_IMAGES = 6

export async function prepareImage(file: File): Promise<TaskImage> {
  if (!['image/png', 'image/jpeg', 'image/webp', 'image/gif', 'image/bmp'].includes(file.type)) {
    throw new Error('支持 PNG、JPG、WebP、GIF 和 BMP 图片')
  }
  if (file.size > 12 * 1024 * 1024) throw new Error('单张原图不能超过 12 MB')
  const url = URL.createObjectURL(file)
  try {
    const img = new Image()
    img.src = url
    await img.decode()
    const scale = Math.min(1, 1600 / Math.max(img.width, img.height))
    const canvas = document.createElement('canvas')
    canvas.width = Math.max(1, Math.round(img.width * scale))
    canvas.height = Math.max(1, Math.round(img.height * scale))
    const context = canvas.getContext('2d')
    if (!context) throw new Error('无法处理图片')
    context.fillStyle = '#fff'
    context.fillRect(0, 0, canvas.width, canvas.height)
    context.drawImage(img, 0, 0, canvas.width, canvas.height)
    const dataUrl = canvas.toDataURL('image/jpeg', 0.78)
    if (dataUrl.length > 1024 * 1024) throw new Error('图片内容过大，请裁剪后重试')
    return { id: crypto.randomUUID(), name: file.name || '截图.jpg', dataUrl, createdAt: new Date().toISOString() }
  } finally {
    URL.revokeObjectURL(url)
  }
}
