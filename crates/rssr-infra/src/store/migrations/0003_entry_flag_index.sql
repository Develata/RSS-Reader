-- Per-subscription counts read this index instead of every entry row: the unread and total
-- counts of the subscription list, and the archived count beside every article list under any
-- read / starred filter. Without it each of those reads all entries of all subscriptions.
CREATE INDEX idx_entries_feed_flags
    ON entries(feed_id, is_read, is_starred, is_archived, published_at);
