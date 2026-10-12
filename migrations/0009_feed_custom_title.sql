-- Name chosen by the user or carried by an import (CLI --title, OPML, config package). Refreshes
-- only update `title`, which now holds the title the feed itself declares; readers show
-- COALESCE(custom_title, title, url). Existing titles are not copied here: they stay source
-- titles and keep following the feed.
ALTER TABLE feeds ADD COLUMN custom_title TEXT;
