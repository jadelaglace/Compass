# T2.4 Web 极薄页 - Review

## 验收
- web/index.html：HTMX + D3 CDN，单页（引力场 + Feed 排行）✅
- web/app.js：D3 force simulation（节点大小=composite，颜色=layer）+ fetch /graph + /feed ✅
- web/style.css：暗色主题基础样式 ✅
- main.rs：ServeDir serve web/ 静态目录 ✅
- Feed 三模式切换（explore/consolidate/strategic）✅
- 节点可拖拽 ✅
- 无构建链（无 npm/Vite）✅

## 设计
- D3 v7 CDN（不打包）
- layer 颜色映射：direction=红/knowledge=青/case=黄/log=紫/insight=粉
- 节点半径按 composite 线性缩放 [5,30]
- 力导向：forceLink + forceManyBody + forceCenter
- Feed 列表右侧 350px 栏

## 112 测试通过（无回归）