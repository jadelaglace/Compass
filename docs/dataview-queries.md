# Dataview 查询模板

> 在 Obsidian 中用 Dataview 插件读取 frontmatter `score.composite` 排序/过滤。
> 将以下代码块粘贴到任意 .md 笔记中即可。

## Top 10 高分实体

```dataview
TABLE score.composite AS "综合分", score.interest AS "兴趣", score.strategy AS "战略", score.consensus AS "共识", layer
FROM ""
WHERE score.composite
SORT score.composite DESC
LIMIT 10
```

## 待复习（consolidate）

```dataview
TABLE score.composite AS "综合分", score.last_boosted_at AS "上次boost", score.access_count AS "访问"
FROM ""
WHERE score.composite AND score.last_boosted_at
SORT score.last_boosted_at ASC
LIMIT 20
```

## 战略焦点（strategic）

```dataview
TABLE score.strategy AS "战略分", score.composite AS "综合分", layer
FROM ""
WHERE score.strategy
SORT score.strategy DESC
LIMIT 10
```

## 按层聚合

```dataview
TABLE layer, count(score.composite) AS "数量", round(avg(score.composite), 1) AS "平均分"
FROM ""
WHERE score.composite
GROUP BY layer
```

## 最近创建

```dataview
TABLE score.composite AS "综合分", layer, created_at
FROM ""
WHERE created_at
SORT created_at DESC
LIMIT 10
```