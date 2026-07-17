# Dataview 查询模板库

> 在 Obsidian 中启用 Dataview 后，将任一代码块粘贴到普通 Markdown 笔记即可使用。查询直接读取 Vault frontmatter 中稳定的基础分 `score.composite`，不读取 SQLite，也不尝试复刻 API 的实时 `effective_composite`。

## 使用约定

- 这些查询面向 Compass 管理的笔记：`status: active`、`layer`、`category`、`tags` 与 `score` 字段的含义见 [PRD v3.0](PRD_v3.0.md)。
- 每条查询都会排除路径中任意层级的 `Templates/` 目录，避免 Templater 源模板进入结果。
- `score.composite` 是 0-100 的稳定基础分。知识时效只在 Compass API 读取时计算；Dataview 不会随着时间改写或显示有效分。
- `FROM ""` 搜索整个 Vault。若只想查询某个目录，将其改为 `FROM "Knowledge"` 等目标目录；仍保留下面的 `WHERE` 条件。

## 1. Top 10 高分实体

```dataview
TABLE file.link AS "笔记", score.composite AS "综合分", score.interest AS "兴趣", score.strategy AS "战略", score.consensus AS "共识", layer
FROM ""
WHERE status = "active"
  AND score.composite != null
  AND !contains(file.path, "Templates/")
SORT score.composite DESC
LIMIT 10
```

## 2. 待复习

按最近一次评分/访问 boost 的时间从早到晚排列，用于找出最久没有被重新触达的笔记。

```dataview
TABLE file.link AS "笔记", score.composite AS "综合分", score.last_boosted_at AS "上次 boost", score.access_count AS "访问次数", layer
FROM ""
WHERE status = "active"
  AND score.composite != null
  AND score.last_boosted_at != null
  AND !contains(file.path, "Templates/")
SORT score.last_boosted_at ASC
LIMIT 20
```

## 3. 战略焦点

```dataview
TABLE file.link AS "笔记", score.strategy AS "战略分", score.composite AS "综合分", layer, category
FROM ""
WHERE status = "active"
  AND score.composite != null
  AND score.strategy != null
  AND !contains(file.path, "Templates/")
SORT score.strategy DESC
LIMIT 10
```

## 4. 按层聚合

```dataview
TABLE WITHOUT ID key AS "层级", length(rows) AS "数量", round(average(rows.score.composite), 1) AS "平均综合分", round(max(rows.score.composite), 1) AS "最高综合分"
FROM ""
WHERE status = "active"
  AND score.composite != null
  AND !contains(file.path, "Templates/")
GROUP BY layer
SORT key ASC
```

## 5. 按分类聚合

一个笔记可属于多个分类，因此同一笔记会在其每个分类中各计一次。

```dataview
TABLE WITHOUT ID key AS "分类", length(rows) AS "数量", round(average(rows.score.composite), 1) AS "平均综合分"
FROM ""
WHERE status = "active"
  AND score.composite != null
  AND category != null
  AND !contains(file.path, "Templates/")
FLATTEN category AS category_item
GROUP BY category_item
SORT length(rows) DESC
```

## 6. 按标签聚合

```dataview
TABLE WITHOUT ID key AS "标签", length(rows) AS "数量", round(average(rows.score.composite), 1) AS "平均综合分"
FROM ""
WHERE status = "active"
  AND score.composite != null
  AND tags != null
  AND !contains(file.path, "Templates/")
FLATTEN tags AS tag_item
GROUP BY tag_item
SORT length(rows) DESC
```

## 7. 最近新增

```dataview
TABLE file.link AS "笔记", created_at AS "创建时间", score.composite AS "综合分", layer
FROM ""
WHERE status = "active"
  AND score.composite != null
  AND created_at != null
  AND !contains(file.path, "Templates/")
SORT created_at DESC
LIMIT 20
```

## 8. 最近更新的内容

`content_updated_at` 表示知识内容本身最近一次更新，不会因为评分、访问或索引重建而变化。

```dataview
TABLE file.link AS "笔记", content_updated_at AS "内容更新时间", score.composite AS "综合分", layer
FROM ""
WHERE status = "active"
  AND score.composite != null
  AND content_updated_at != null
  AND !contains(file.path, "Templates/")
SORT content_updated_at DESC
LIMIT 20
```

## 9. 孤儿笔记

```dataview
TABLE file.link AS "笔记", score.composite AS "综合分", category, tags, created_at AS "创建时间"
FROM ""
WHERE status = "orphan"
  AND score.composite != null
  AND !contains(file.path, "Templates/")
SORT score.composite DESC
```

## 10. 高战略、低共识的待验证事项

此视图不使用固定阈值，而是按战略分与共识分的差值排序，便于找出值得验证或补充证据的高价值判断。

```dataview
TABLE file.link AS "笔记", score.strategy AS "战略分", score.consensus AS "共识分", round(score.strategy - score.consensus, 1) AS "差值", score.composite AS "综合分", layer
FROM ""
WHERE status = "active"
  AND score.composite != null
  AND score.strategy != null
  AND score.consensus != null
  AND score.strategy > score.consensus
  AND !contains(file.path, "Templates/")
SORT (score.strategy - score.consensus) DESC
LIMIT 20
```

## 字段速查

| 字段 | 用途 | 更新者 |
|---|---|---|
| `score.composite` | 稳定基础综合分；上述排序和聚合的主信号 | Compass 评分引擎 |
| `score.interest` / `strategy` / `consensus` | 三个基础评分维度 | 用户或 Compass 显式评分/触发器 |
| `score.last_boosted_at` / `score.access_count` | 最近 boost 时间与累计访问次数 | Compass |
| `layer` / `category` / `tags` / `status` | 内容组织、筛选与状态 | 用户在 Obsidian 中维护 |
| `created_at` / `content_updated_at` | 创建时间与内容更新时间 | 用户或内容写入流程 |

对实时有效分、Feed 或搜索结果，请使用 Compass API/Skill；Dataview 的职责是让 Vault 中可审计的基础分和元数据在 Obsidian 内可见。
