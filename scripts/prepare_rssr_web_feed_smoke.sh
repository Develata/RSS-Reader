#!/usr/bin/env bash
# Builds the static directory the rssr-web browser feed smoke serves: a copy of the web bundle
# plus, under /__smoke/, the smoke page and a feed fixture dated now. rssr-web has no test
# routes; these are ordinary static files behind its real login, and the app fetches the fixture
# from its own origin.
# Usage: prepare_rssr_web_feed_smoke.sh WEB_BUNDLE_PUBLIC_DIR OUTPUT_DIR
set -euo pipefail

public_dir="${1:?missing web bundle public directory}"
output_dir="${2:?missing output directory}"
script_path="${BASH_SOURCE[0]//\\//}"
script_dir="${script_path%/*}"
if [[ "$script_dir" == "$script_path" ]]; then script_dir=.; fi

if [[ ! -f "$public_dir/index.html" ]]; then
  echo "Web bundle not found: $public_dir" >&2
  exit 1
fi
if [[ -e "$output_dir" ]]; then
  echo "Output directory already exists: $output_dir" >&2
  exit 1
fi

mkdir -p "$output_dir/__smoke"
cp -R "$public_dir/." "$output_dir/"
cp "$script_dir/browser/rssr_web_feed_smoke.html" "$output_dir/__smoke/browser-feed-smoke.html"

# Dated now, so the entry stays inside the default archive window.
published_at="$(LC_ALL=C date -u '+%a, %d %b %Y %H:%M:%S +0000')"
cat >"$output_dir/__smoke/feed-fixture.xml" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0">
  <channel>
    <title>Codex Smoke Feed</title>
    <link>https://example.com/</link>
    <description>Codex smoke feed fixture.</description>
    <item>
      <guid>https://example.com/posts/codex-smoke-entry</guid>
      <title>Codex Smoke Entry</title>
      <link>https://example.com/posts/codex-smoke-entry</link>
      <pubDate>${published_at}</pubDate>
      <description><![CDATA[<p>Codex smoke feed entry body.</p>]]></description>
    </item>
  </channel>
</rss>
EOF
