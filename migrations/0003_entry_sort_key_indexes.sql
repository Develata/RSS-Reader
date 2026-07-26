-- 文章列表与阅读导航统一按 COALESCE(published_at, created_at) DESC, id DESC 排序，
-- 但既有索引建在裸 published_at 上，这个排序表达式一条也命中不了，列表查询只能全表扫 + 排序。
-- SQLite 支持表达式索引，这里按实际排序键建索引。
--
-- 三个变体分别对应三类查询：
--   * 全局列表 / 全局“上一篇未读、下一篇未读”
--   * 单订阅列表 / 阅读页同源上一篇、下一篇
--   * 未读筛选（read_filter = UnreadOnly，也是阅读导航的默认路径）

CREATE INDEX IF NOT EXISTS idx_entries_sort_key
    ON entries(COALESCE(published_at, created_at) DESC, id DESC);

CREATE INDEX IF NOT EXISTS idx_entries_feed_sort_key
    ON entries(feed_id, COALESCE(published_at, created_at) DESC, id DESC);

CREATE INDEX IF NOT EXISTS idx_entries_unread_sort_key
    ON entries(is_read, COALESCE(published_at, created_at) DESC, id DESC);
