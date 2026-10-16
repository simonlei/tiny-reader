/**
 * 渲染期的 URL 补全：把正文 HTML 里的相对地址补成绝对地址。
 *
 * 背景：很多 RSS 源（如 Hugo 生成的 tonybai.com/feed/）正文里的图片是
 * 根相对路径 `<img src="/images/x.png">`。后台按原样保存（不改动源内容），
 * 前端页面 origin 是 tauri.localhost，浏览器会把 `/images/x.png` 解析成
 * `http://tauri.localhost/images/x.png`，图片就挂了。
 *
 * 这里在渲染前做一次字符串层面的补全，源内容本身不被修改。
 */

/** 已经有协议或是特殊协议的地址，直接跳过，避免二次破坏 */
function isAbsoluteUrl(u: string): boolean {
  return (
    u.startsWith('http://') ||
    u.startsWith('https://') ||
    // 协议相对，如 //example.com/a.png（浏览器会自行补协议）
    u.startsWith('//') ||
    u.startsWith('data:') ||
    u.startsWith('blob:') ||
    u.startsWith('#') ||
    u.startsWith('mailto:') ||
    u.startsWith('javascript:')
  )
}

/**
 * 把相对 URL 补成绝对 URL。
 * @param base 站点根地址，如 https://tonybai.com/（必须带协议和域名）
 */
function resolveUrl(u: string, base: string): string {
  const raw = u.trim()
  if (!raw || isAbsoluteUrl(raw)) return raw
  try {
    // 用 URL 做标准解析，自动处理 /images/x.png 与 ./x.png 等情况
    return new URL(raw, base).toString()
    // eslint-disable-next-line @typescript-eslint/no-unused-vars
  } catch (_) {
    return raw
  }
}

/**
 * 补全 HTML 里 img/@src 和 a/@href 的相对地址。
 * 只做字符串替换，不解析整棵 DOM，开销很低。
 */
export function absolutizeHtml(html: string, base?: string | null): string {
  if (!html) return html
  if (!base) return html

  let baseUrl: URL
  try {
    baseUrl = new URL(base)
    // eslint-disable-next-line @typescript-eslint/no-unused-vars
  } catch (_) {
    // base 非法（如 null 或不是 URL）就原样返回，不瞎拼
    return html
  }
  const root = baseUrl.origin // 只取 origin，避免把 /feed/ 这种路径带进去

  // 匹配 <img ... src="...">，容忍单引号与属性顺序
  let out = html.replace(
    /(<img\b[^>]*?\bsrc\s*=\s*)(["'])(.*?)\2/gi,
    (m, prefix: string, quote: string, url: string) => {
      const resolved = resolveUrl(url, root)
      return resolved === url ? m : `${prefix}${quote}${resolved}${quote}`
    },
  )

  // 匹配 <a ... href="...">
  out = out.replace(
    /(<a\b[^>]*?\bhref\s*=\s*)(["'])(.*?)\2/gi,
    (m, prefix: string, quote: string, url: string) => {
      const resolved = resolveUrl(url, root)
      return resolved === url ? m : `${prefix}${quote}${resolved}${quote}`
    },
  )

  return out
}

/**
 * 配置里的一条域名规则。精确匹配；以 `.` 或 `*.` 开头表示只匹配子域名。
 */
export function hostMatches(pattern: string, host: string): boolean {
  const h = host.trim().replace(/\.+$/, '').toLowerCase()
  let p = pattern.trim().replace(/\.+$/, '').toLowerCase()
  if (!h || !p) return false
  if (p.startsWith('*.')) p = `.${p.slice(2)}`
  if (p.startsWith('.')) return p.length > 1 && h.endsWith(p) && h.length > p.length
  return h === p
}

export function isHotlinkHost(host: string, patterns: readonly string[]): boolean {
  return patterns.some((p) => hostMatches(p, host))
}

function hostOf(url: string): string | null {
  const raw = url.trim()
  if (!raw) return null
  try {
    const abs = raw.startsWith('//') ? `https:${raw}` : raw
    return new URL(abs).hostname
  } catch {
    return null
  }
}

/**
 * 把防盗链图片改写到服务端 `/api/media`。
 * 浏览器从阅读器页面加载图片时，发不出来 `Referer: https://jandan.net/`。
 */
export function rewriteHotlinkImages(
  html: string,
  serverUrl: string,
  token: string,
  patterns: readonly string[],
): string {
  const base = (serverUrl || '').trim().replace(/\/+$/, '')
  const access = (token || '').trim()
  if (!html || !base || !access || patterns.length === 0) return html

  return html.replace(
    /(<img\b[^>]*?\bsrc\s*=\s*)(["'])(.*?)\2/gi,
    (m, prefix: string, quote: string, url: string) => {
      const host = hostOf(url)
      if (!host || !isHotlinkHost(host, patterns) || url.includes('/api/media?')) return m
      const proxied =
        `${base}/api/media?url=${encodeURIComponent(url)}` +
        `&token=${encodeURIComponent(access)}`
      return `${prefix}${quote}${proxied}${quote}`
    },
  )
}
