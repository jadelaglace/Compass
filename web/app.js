// Compass 引力场 + Feed 极薄前端

const layerColors = {
    direction: "#ff6b6b",
    knowledge: "#64ffda",
    case: "#ffd93d",
    log: "#a78bfa",
    insight: "#f0a8d0",
};

let currentMode = "explore";

// ---- Feed ----

async function loadFeed(mode) {
    currentMode = mode;
    document.querySelectorAll(".mode-btn").forEach(b => {
        b.classList.toggle("active", b.dataset.mode === mode);
    });

    const res = await fetch(`/feed?mode=${mode}&limit=20`);
    const items = await res.json();
    const list = document.getElementById("feed-list");
    list.innerHTML = items.map(e => `
        <div class="feed-item">
            <div class="title">${e.title || e.id}</div>
            <div class="meta">
                <span class="score">${e.composite != null ? e.composite.toFixed(1) : "-"}</span>
                · ${e.layer || ""}
            </div>
        </div>
    `).join("");
}

// ---- Graph (D3 force-directed) ----

async function loadGraph() {
    const res = await fetch("/graph");
    const data = await res.json();

    const svg = d3.select("#graph");
    const width = svg.node().clientWidth;
    const height = 500;

    svg.selectAll("*").remove();

    if (!data.nodes || data.nodes.length === 0) {
        svg.append("text")
            .attr("x", width / 2).attr("y", height / 2)
            .attr("text-anchor", "middle").attr("fill", "#666")
            .text("暂无数据");
        return;
    }

    const maxComp = d3.max(data.nodes, d => d.composite || 0) || 100;
    const radiusScale = d3.scaleLinear().domain([0, maxComp]).range([5, 30]);

    const sim = d3.forceSimulation(data.nodes)
        .force("link", d3.forceLink(data.edges).id(d => d.id).distance(80))
        .force("charge", d3.forceManyBody().strength(-200))
        .force("center", d3.forceCenter(width / 2, height / 2));

    const link = svg.selectAll(".link")
        .data(data.edges).enter().append("line")
        .attr("class", "link")
        .attr("stroke-width", 1);

    const node = svg.selectAll(".node")
        .data(data.nodes).enter().append("g")
        .attr("class", "node")
        .call(d3.drag()
            .on("start", (e, d) => {
                if (!e.active) sim.alphaTarget(0.3).restart();
                d.fx = d.x; d.fy = d.y;
            })
            .on("drag", (e, d) => { d.fx = e.x; d.fy = e.y; })
            .on("end", (e, d) => { d.fx = null; d.fy = null; })
        );

    node.append("circle")
        .attr("r", d => radiusScale(d.composite || 0))
        .attr("fill", d => layerColors[d.layer] || "#888");

    node.append("text")
        .attr("dx", d => radiusScale(d.composite || 0) + 3)
        .attr("dy", 3)
        .text(d => (d.title || d.id).slice(0, 12));

    sim.on("tick", () => {
        link
            .attr("x1", d => d.source.x).attr("y1", d => d.source.y)
            .attr("x2", d => d.target.x).attr("y2", d => d.target.y);
        node.attr("transform", d => `translate(${d.x},${d.y})`);
    });
}

// ---- 初始化 ----

document.querySelectorAll(".mode-btn").forEach(btn => {
    btn.addEventListener("click", () => loadFeed(btn.dataset.mode));
});

loadFeed("explore");
loadGraph();