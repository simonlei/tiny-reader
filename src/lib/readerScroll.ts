/**
 * 阅读区（文章正文）的滚动导航。
 *
 * 快捷键 j / ↓、k / ↑ 的行为：先在文章内部滚动，
 * 只有滚到边界（底部 / 顶部）时才切换上/下一篇文章。
 *
 * ArticleView 把正文滚动容器注册进来，App.vue 的全局快捷键再调用这里的方法。
 */

let bodyEl: HTMLElement | null = null

/** 边界判定容差（px）：页面缩放时 scrollTop 往往是小数，不会刚好等于极值 */
const EPS = 4

/** 每次按键滚动一屏的按比例（视口高度），留一点重叠避免读漏行 */
const STEP_RATIO = 0.85
/** 至少滚动的距离，避免视口极小时几乎不动 */
const MIN_STEP = 40

export function registerReaderBody(el: HTMLElement | null) {
  bodyEl = el
}

type Metrics = { el: HTMLElement; top: number; max: number }

function metrics(): Metrics | null {
  const el = bodyEl
  if (!el) return null
  return {
    el,
    top: el.scrollTop,
    max: el.scrollHeight - el.clientHeight,
  }
}

/** 正文已滚到顶部（没有阅读区时也按"到顶"处理，方便交给外层切上一篇） */
export function atTop(): boolean {
  const m = metrics()
  return !m || m.top <= EPS
}

/** 正文已滚到底部（没有阅读区 / 内容不足一屏时也认为到底，直接切下一篇） */
export function atBottom(): boolean {
  const m = metrics()
  return !m || m.top >= m.max - EPS
}

/**
 * 在文章内滚动一屏。
 * @param dir 1 向下，-1 向上
 * @returns true 表示这次按键被滚动消费掉了；false 表示已到边界，应该切换文章
 */
export function scrollArticle(dir: 1 | -1): boolean {
  const m = metrics()
  if (!m) return false
  // 内容不足一屏，没有可滚动的余地，直接切换
  if (m.max <= EPS) return false
  if (dir > 0 && atBottom()) return false
  if (dir < 0 && atTop()) return false

  const step = Math.max(m.el.clientHeight * STEP_RATIO, MIN_STEP)
  const next = Math.min(Math.max(m.top + dir * step, 0), m.max)
  if (Math.abs(next - m.top) <= EPS) return false
  m.el.scrollTop = next
  return true
}
