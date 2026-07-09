# T2.2 Feed 三模式 - Review

## 验收
- explore：composite 降序 ✅ (test_feed_explore_default)
- consolidate：last_boosted_at 升序（NULL 最前）✅ (test_feed_consolidate_mode + null_first)
- strategic：strategy 降序 ✅ (test_feed_strategic_mode)

## 改动
- EntitySummary 加 strategy/last_boosted_at 字段
- feed handler 按 mode 分支排序
- entities_top map 同步加字段

## 109 测试通过（原 105 + 新 4）