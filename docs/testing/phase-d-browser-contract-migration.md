# 阶段 D：旧浏览器契约迁移对照

核对基线：`fde07ac` 的三个 wasm harness 及 `support/browser_storage_cases.rs`。先迁移产品语义，再删除实现；表中的存储机制退役是契约变更，不计作旧行为通过。

现用 SQL 案例通过 `cfg_attr` 在 native Tokio 与真实浏览器 wasm-bindgen-test 中运行同一份源码；CI 的清单是 `crates/rssr-infra/Cargo.toml` 的 `package.metadata.rssr.wasm-contracts`。原生文件锁、跨进程与磁盘故障仍只在原生测试，不拿同页内存连接冒充这些证据。

Pages 改为不持久化，每次重载重建。旧站点数据不会自动迁移或清除。原来捕捉 localStorage 字符量的测试没有转述成 SQLite 性能证据；Pages 的包体积、启动、WASM 线性内存、JS heap 与 long-task 数据单独采集。

| 旧模块 / 用例 | 去向 | 验证位置或边界 |
|---|---|---|
| `wasm_subscription_contract_harness::browser_write_failures_are_storage_errors` | 存储机制退役 | localStorage 配额异常不再存在；SQL 约束、写失败、截止中断分类由 test_library_store、wasm_memory_store 与原子性案例覆盖，不声称两种故障相同 |
| `wasm_subscription_contract_harness::browser_activation_port_captures_generation_and_rejects_active_duplicate` | 共用 SQL | test_library_persistence_atomicity::activation_port_captures_generation_and_rejects_active_duplicate |
| `wasm_subscription_contract_harness::browser_subscription_add_normalizes_and_deduplicates_urls` | 共用 SQL | test_library_subscription_contract_harness：对应 add/remove 契约 |
| `wasm_subscription_contract_harness::browser_subscription_remove_purges_entries_soft_deletes_feed_and_clears_matching_state` | 共用 SQL | test_library_subscription_contract_harness：对应 add/remove 契约 |
| `wasm_subscription_contract_harness::browser_subscription_remove_preserves_other_last_opened_feed` | 共用 SQL | test_library_subscription_contract_harness：对应 add/remove 契约 |
| `wasm_subscription_contract_harness::library_queries_leave_out_removed_subscriptions` | 共用 SQL | test_library_queries：reader_snapshot、startup_state；test_library_persistence_atomicity：删除/恢复 |
| `wasm_subscription_contract_harness::reader_metadata_and_global_unread_counts_survive_failed_flag_writes` | 共用 SQL | test_library_queries::reader_snapshot_has_the_record_body_neighbours_and_active_feed；test_library_entry_state_and_search::unread_summaries_are_global_idempotent_and_unchanged_on_failure、repeated_flag_requests_keep_the_original_time |
| `wasm_subscription_contract_harness::navigation_matches_list_order_with_missing_dates_ties_deleted_feeds_and_sparse_flags` | 共用 SQL | test_library_queries::navigation_matches_list_order_with_missing_dates_ties_and_deleted_feeds（孤儿 feed 由外键排除） |
| `wasm_subscription_contract_harness::browser_bulk_read_matches_sqlite_cases_and_rolls_back_failed_storage` | 共用 SQL | test_library_contracts::bulk_read_scope_confirmation_atomic_failure_and_zero |
| `wasm_subscription_contract_harness::browser_bulk_read_refuses_a_selection_over_its_bound` | 共用 SQL | test_library_contracts::bulk_read_refuses_a_selection_over_its_bound |
| `wasm_subscription_contract_harness::browser_titles_in_lists_are_bounded` | 共用 SQL | test_library_contracts::titles_in_lists_are_bounded |
| `wasm_subscription_contract_harness::browser_discovery_parser_matches_native` | 保留原生 | test_subscription_discovery（同一 discovery_cases）；Pages 不再提供在线探测 |
| `wasm_subscription_contract_harness::browser_discovery_is_bounded_while_the_page_is_read` | 保留原生 | test_subscription_discovery（同一 discovery_cases）；Pages 不再提供在线探测 |
| `wasm_subscription_contract_harness::browser_app_state_narrow_updates_do_not_lose_each_other` | 共用 SQL | test_library_queries::app_state_narrow_updates_do_not_lose_each_other |
| `wasm_subscription_contract_harness::browser_keeps_chosen_names_apart_from_source_titles` | 共用 SQL | test_library_contracts::chosen_names_stay_apart_from_source_titles |
| `wasm_config_exchange_contract_harness::browser_settings_updates_merge_in_one_write` | 共用 SQL | test_library_settings_repository::settings_updates_merge_in_one_write |
| `wasm_config_exchange_contract_harness::browser_config_exchange_export_json_captures_active_feeds_and_settings` | 共用 SQL | test_library_config_exchange_contract_harness：JSON、删除清理、remote roundtrip；test_library_queries::config_snapshot_has_the_active_subscriptions_and_settings |
| `wasm_config_exchange_contract_harness::browser_config_exchange_import_cleans_removed_feed_entries_and_last_opened_state` | 共用 SQL | test_library_config_exchange_contract_harness：JSON、删除清理、remote roundtrip；test_library_queries::config_snapshot_has_the_active_subscriptions_and_settings |
| `wasm_config_exchange_contract_harness::browser_config_exchange_remote_pull_roundtrip_restores_feed_and_settings` | 共用 SQL | test_library_config_exchange_contract_harness：JSON、删除清理、remote roundtrip；test_library_queries::config_snapshot_has_the_active_subscriptions_and_settings |
| `wasm_config_exchange_contract_harness::browser_opml_uses_the_shared_codec_and_its_name_rules` | 共用 SQL | test_library_config_exchange_contract_harness::opml_keeps_chosen_names_apart_from_source_titles_and_rejects_outside_body；截断拒绝案例 |
| `wasm_refresh_contract_harness::a_webdav_download_that_never_finishes_times_out` | 删除浏览器网络实现 | 原生 test_feed_response_limits、test_audit_20261009_refresh_invariants、test_library_feed_refresh_flow、test_library_webdav_local_roundtrip 保留；Pages 直接用例拒绝网络及 UI add/refresh 禁用另验 |
| `wasm_refresh_contract_harness::a_store_held_by_another_tab_is_unavailable` | 存储机制退役 | 页面不再共享文章库或 Web Lock；wasm_memory_store 验准入超时与取消；原生文件竞争继续由 test_library_store 覆盖 |
| `wasm_refresh_contract_harness::browser_fractional_published_times_are_archived_by_their_value` | 共用 SQL | test_library_contracts::fractional_published_times_are_archived_by_their_value |
| `wasm_refresh_contract_harness::browser_refresh_only_updates_changed_content_records` | 共用 SQL | test_library_contracts::refresh_rewrites_only_changed_bodies |
| `wasm_refresh_contract_harness::browser_refresh_bounds_streams_and_preserves_decoding_and_failures` | 删除浏览器网络实现 | 原生 test_feed_response_limits、test_audit_20261009_refresh_invariants、test_library_feed_refresh_flow、test_library_webdav_local_roundtrip 保留；Pages 直接用例拒绝网络及 UI add/refresh 禁用另验 |
| `wasm_refresh_contract_harness::browser_refresh_source_classifies_not_modified_status` | 删除浏览器网络实现 | 原生 test_feed_response_limits、test_audit_20261009_refresh_invariants、test_library_feed_refresh_flow、test_library_webdav_local_roundtrip 保留；Pages 直接用例拒绝网络及 UI add/refresh 禁用另验 |
| `wasm_refresh_contract_harness::browser_refresh_source_classifies_non_success_status_as_failure` | 删除浏览器网络实现 | 原生 test_feed_response_limits、test_audit_20261009_refresh_invariants、test_library_feed_refresh_flow、test_library_webdav_local_roundtrip 保留；Pages 直接用例拒绝网络及 UI add/refresh 禁用另验 |
| `wasm_refresh_contract_harness::browser_refresh_source_allows_success_status_to_continue_to_body_classification` | 删除浏览器网络实现 | 原生 test_feed_response_limits、test_audit_20261009_refresh_invariants、test_library_feed_refresh_flow、test_library_webdav_local_roundtrip 保留；Pages 直接用例拒绝网络及 UI add/refresh 禁用另验 |
| `wasm_refresh_contract_harness::browser_refresh_source_classifies_valid_xml_body_as_updated` | 删除浏览器网络实现 | 原生 test_feed_response_limits、test_audit_20261009_refresh_invariants、test_library_feed_refresh_flow、test_library_webdav_local_roundtrip 保留；Pages 直接用例拒绝网络及 UI add/refresh 禁用另验 |
| `wasm_refresh_contract_harness::browser_refresh_source_classifies_html_shell_body_as_parse_failure` | 删除浏览器网络实现 | 原生 test_feed_response_limits、test_audit_20261009_refresh_invariants、test_library_feed_refresh_flow、test_library_webdav_local_roundtrip 保留；Pages 直接用例拒绝网络及 UI add/refresh 禁用另验 |
| `wasm_refresh_contract_harness::browser_refresh_source_classifies_bad_xml_body_as_parse_failure` | 删除浏览器网络实现 | 原生 test_feed_response_limits、test_audit_20261009_refresh_invariants、test_library_feed_refresh_flow、test_library_webdav_local_roundtrip 保留；Pages 直接用例拒绝网络及 UI add/refresh 禁用另验 |
| `wasm_refresh_contract_harness::browser_refresh_store_lists_only_active_targets` | 共用 SQL | test_library_refresh_contract_harness::refresh_targets_include_only_active_normalized_feeds |
| `wasm_refresh_contract_harness::browser_refresh_store_get_target_normalizes_url_and_skips_deleted_feeds` | 共用 SQL | test_library_refresh_contract_harness::refresh_targets_include_only_active_normalized_feeds |
| `wasm_refresh_contract_harness::browser_refresh_store_commit_not_modified_updates_state_and_storage` | 共用 SQL | test_library_refresh_contract_harness：304 状态与校验器；test_library_refresh_tickets：304 基线 |
| `wasm_refresh_contract_harness::browser_refresh_store_not_modified_keeps_validators_it_did_not_receive` | 共用 SQL | test_library_refresh_contract_harness：304 状态与校验器；test_library_refresh_tickets：304 基线 |
| `wasm_refresh_contract_harness::browser_refresh_store_commit_updated_persists_feed_metadata_and_entries` | 共用 SQL | test_library_refresh_contract_harness、test_library_application_refresh_store_adapter、test_library_refresh_tickets：成功/失败与诊断时间、校验器 |
| `wasm_refresh_contract_harness::browser_refresh_store_commit_updated_clears_previous_fetch_error` | 共用 SQL | test_library_refresh_contract_harness、test_library_application_refresh_store_adapter、test_library_refresh_tickets：成功/失败与诊断时间、校验器 |
| `wasm_refresh_contract_harness::browser_refresh_store_commit_failed_persists_error_without_success_timestamp` | 共用 SQL | test_library_refresh_contract_harness、test_library_application_refresh_store_adapter、test_library_refresh_tickets：成功/失败与诊断时间、校验器 |
| `wasm_refresh_contract_harness::browser_refresh_store_commit_failed_preserves_previous_success_timestamp` | 共用 SQL | test_library_refresh_contract_harness、test_library_application_refresh_store_adapter、test_library_refresh_tickets：成功/失败与诊断时间、校验器 |
| `wasm_refresh_contract_harness::browser_refresh_store_commits_are_durable_before_the_batch_ends` | 共用 SQL | test_library_contracts::committed_refreshes_survive_batch_boundaries（每次提交即生效） |
| `wasm_refresh_contract_harness::browser_refresh_store_batch_without_commits_writes_nothing` | 共用 SQL | test_library_contracts::committed_refreshes_survive_batch_boundaries（每次提交即生效） |
| `wasm_refresh_contract_harness::browser_refresh_store_reopening_a_batch_preserves_durable_results` | 共用 SQL | test_library_contracts::committed_refreshes_survive_batch_boundaries（每次提交即生效） |
| `wasm_refresh_contract_harness::browser_refresh_store_commit_outside_a_batch_still_writes_immediately` | 共用 SQL | test_library_contracts::committed_refreshes_survive_batch_boundaries（每次提交即生效） |
| `wasm_refresh_contract_harness::browser_refresh_store_abort_preserves_commits_and_subsequent_writes` | 共用 SQL | test_library_contracts::committed_refreshes_survive_batch_boundaries（每次提交即生效） |
| `wasm_refresh_contract_harness::browser_refresh_store_abort_after_end_batch_writes_nothing` | 共用 SQL | test_library_contracts::committed_refreshes_survive_batch_boundaries（每次提交即生效） |
| `wasm_refresh_contract_harness::browser_counts_only_real_inserts_including_same_batch_duplicates` | 共用 SQL | test_library_contracts::counts_only_real_inserts_including_same_batch_duplicates |
| `wasm_refresh_contract_harness::failed_refresh_at_each_publication_stage_keeps_old_content_and_index` | 共用 SQL | test_library_application_refresh_store_adapter：内容/索引写失败后重试；test_library_persistence_atomicity：事务回滚 |
| `wasm_refresh_contract_harness::refresh_after_other_tab_deletes_feed_does_not_resurrect_it` | 共用 SQL | test_library_refresh_tickets::a_refresh_from_before_a_removal_and_re_add_writes_nothing；test_library_application_refresh_store_adapter::old_library_refresh_generation_cannot_commit_after_delete_and_same_url_readd |
| `wasm_refresh_contract_harness::old_refresh_generation_cannot_commit_after_delete_and_same_url_readd` | 共用 SQL | test_library_refresh_tickets::a_refresh_from_before_a_removal_and_re_add_writes_nothing；test_library_application_refresh_store_adapter::old_library_refresh_generation_cannot_commit_after_delete_and_same_url_readd |
| `wasm_refresh_contract_harness::metadata_only_refreshes_do_not_rewrite_flags_or_bodies` | 共用 SQL | test_library_contracts::metadata_only_refreshes_do_not_rewrite_flags_or_bodies（SQL trigger 检测写入） |
| `wasm_refresh_contract_harness::multiple_content_commits_preserve_all_feeds_and_report_write_volume` | 共用 SQL | test_library_contracts::concurrent_handles_preserve_other_entries_and_other_flag_bits、refresh_rewrites_only_changed_bodies；localStorage 字符写放大量随机制退役 |
| `wasm_refresh_contract_harness::repeated_content_commits_report_write_volume` | 共用 SQL | test_library_contracts::concurrent_handles_preserve_other_entries_and_other_flag_bits、refresh_rewrites_only_changed_bodies；localStorage 字符写放大量随机制退役 |
| `wasm_refresh_contract_harness::browser_quota_errors_are_reported_clearly` | 存储机制退役 | localStorage 配额异常不再存在；SQL 约束、写失败、截止中断分类由 test_library_store、wasm_memory_store 与原子性案例覆盖，不声称两种故障相同 |
| `support/browser_storage_cases::stale_stores_preserve_other_entries_and_other_bits_with_concurrent_writes` | 共用 SQL | test_library_contracts::concurrent_handles_preserve_other_entries_and_other_flag_bits |
| `support/browser_storage_cases::stale_settings_and_app_state_do_not_resurrect_deleted_entries_or_flags` | 共用 SQL | test_library_persistence_atomicity：删除/配置替换原子性；test_library_queries::app_state_narrow_updates_do_not_lose_each_other；仓储不保存文章缓存 |
| `support/browser_storage_cases::concurrent_subscription_inserts_allocate_distinct_ids_and_bulk_preview_is_rechecked` | 共用 SQL | test_library_contracts::concurrent_handles_preserve_other_entries_and_other_flag_bits、bulk_read_scope_confirmation_atomic_failure_and_zero |
| `support/browser_storage_cases::flag_writes_and_warm_reads_do_not_read_or_write_article_bodies` | 共用 SQL | test_library_contracts::listing_and_flag_writes_do_not_need_article_bodies |
| `support/browser_storage_cases::resetting_storage_changes_epoch_and_invalidates_existing_caches` | 存储机制退役 | localStorage 版本头、双槽、迁移、缓存、epoch、Web Locks 已删除；Pages 每次建立独立内存库，旧键不读取、不覆盖；由 pages_smoke 与 wasm_memory_store 验证新契约 |
| `support/browser_storage_cases::corrupt_legacy_or_committed_data_is_preserved_and_never_reset_to_empty` | 存储机制退役 | localStorage 版本头、双槽、迁移、缓存、epoch、Web Locks 已删除；Pages 每次建立独立内存库，旧键不读取、不覆盖；由 pages_smoke 与 wasm_memory_store 验证新契约 |
| `support/browser_storage_cases::orphan_staging_is_ignored_and_cleanup_failure_does_not_undo_a_commit` | 存储机制退役 | localStorage 版本头、双槽、迁移、缓存、epoch、Web Locks 已删除；Pages 每次建立独立内存库，旧键不读取、不覆盖；由 pages_smoke 与 wasm_memory_store 验证新契约 |
| `support/browser_storage_cases::cancelled_queued_write_never_runs_after_lock_release` | 替换执行器 | wasm_memory_store::dropped_queued_caller_discards_capture_and_never_executes；原生 test_library_store |
| `support/browser_storage_cases::lock_timeout_and_missing_capability_fail_without_writing` | 存储机制退役 | localStorage 版本头、双槽、迁移、缓存、epoch、Web Locks 已删除；Pages 每次建立独立内存库，旧键不读取、不覆盖；由 pages_smoke 与 wasm_memory_store 验证新契约 |
| `support/browser_storage_cases::failed_rollback_keeps_cache_unavailable_until_committed_data_can_be_reloaded` | 存储机制退役 | localStorage 版本头、双槽、迁移、缓存、epoch、Web Locks 已删除；Pages 每次建立独立内存库，旧键不读取、不覆盖；由 pages_smoke 与 wasm_memory_store 验证新契约 |
| `support/browser_storage_cases::missing_head_with_newer_slots_is_not_mistaken_for_an_empty_legacy_database` | 存储机制退役 | localStorage 版本头、双槽、迁移、缓存、epoch、Web Locks 已删除；Pages 每次建立独立内存库，旧键不读取、不覆盖；由 pages_smoke 与 wasm_memory_store 验证新契约 |
| `support/browser_storage_cases::initial_state_is_atomic_and_never_replaces_existing_data` | 存储机制退役 | localStorage 版本头、双槽、迁移、缓存、epoch、Web Locks 已删除；Pages 每次建立独立内存库，旧键不读取、不覆盖；由 pages_smoke 与 wasm_memory_store 验证新契约 |
| `support/browser_storage_cases::a_version_one_head_is_upgraded_in_place_without_rewriting_slices` | 存储机制退役 | localStorage 版本头、双槽、迁移、缓存、epoch、Web Locks 已删除；Pages 每次建立独立内存库，旧键不读取、不覆盖；由 pages_smoke 与 wasm_memory_store 验证新契约 |
| `support/browser_storage_cases::a_newer_head_version_is_rejected_without_touching_storage` | 存储机制退役 | localStorage 版本头、双槽、迁移、缓存、epoch、Web Locks 已删除；Pages 每次建立独立内存库，旧键不读取、不覆盖；由 pages_smoke 与 wasm_memory_store 验证新契约 |
| `support/browser_storage_cases::a_failed_head_upgrade_keeps_version_one_and_the_same_handle_retries` | 存储机制退役 | localStorage 版本头、双槽、迁移、缓存、epoch、Web Locks 已删除；Pages 每次建立独立内存库，旧键不读取、不覆盖；由 pages_smoke 与 wasm_memory_store 验证新契约 |
| `support/browser_storage_cases::a_version_one_head_with_a_corrupt_slice_is_not_upgraded` | 存储机制退役 | localStorage 版本头、双槽、迁移、缓存、epoch、Web Locks 已删除；Pages 每次建立独立内存库，旧键不读取、不覆盖；由 pages_smoke 与 wasm_memory_store 验证新契约 |

共 69 项（包含旧 support 模块注册的浏览器测试）。结果随本轮 handoff 更新；此表本身不是测试回执。

## application 层旧进程序号测试

阶段 A 明确要求在删除 BrowserStore 后移除进程内刷新序号。现在每订阅的互斥锁仍保留，发布顺序统一交给 SQL 的 I/P/D；`PreparedUpdateTicket` 与两项针对旧非原子存储桩的测试随之删除：

- `a_prepared_update_never_overwrites_a_later_refresh`：由 `test_library_refresh_tickets` 的 `the_prepared_fetch_at_activation_loses_to_a_later_refresh` 与 `a_prepared_fetch_survives_a_failed_refresh_in_the_same_process` 验证真实 SQL 中的新结果覆盖和失败后保留预抓取结果。
- `a_refresh_interrupted_while_committing_still_supersedes_an_older_prepared_update`：由 `a_prepared_response_cannot_overwrite_a_commit_whose_caller_was_cancelled` 验证已提交但调用方放弃等待后的防旧写；执行器取消边界另由 `test_library_store` / `wasm_memory_store` 覆盖。旧桩的“分步写了一半再失败”不再作为 SQLite 事务语义；提交前失败会回滚，不应凭空让旧响应失效。
