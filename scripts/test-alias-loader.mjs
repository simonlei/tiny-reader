// node:test 不走 Vite，把源码里的 @/ 别名解析成 src 下的 .ts 文件。
export async function resolve(specifier, context, nextResolve) {
  if (specifier.startsWith('@/')) {
    const url = new URL(`../src/${specifier.slice(2)}.ts`, import.meta.url)
    return nextResolve(url.href, context)
  }
  return nextResolve(specifier, context)
}
