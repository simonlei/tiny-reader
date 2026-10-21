import raw from '../data/feedCatalog.json'
import type { CatalogFeed } from './recommendFeeds'

export const feedCatalog: CatalogFeed[] = raw.feeds
export const feedCatalogSource = raw.source
