# Playground

Try GRW directly in your browser. Type `graph!`, `modify!`, or `search!` commands and see the results live.

<div id="playground-app">
<div id="pg-controls">
  <div id="pg-examples">
    <button class="pg-btn" id="btn-graph">graph!</button>
    <button class="pg-btn" id="btn-modify">modify!</button>
    <button class="pg-btn" id="btn-search">search!</button>
    <button class="pg-btn pg-btn-dim" id="btn-reset">reset</button>
  </div>
  <div id="pg-editor-wrap">
    <textarea id="pg-input" spellcheck="false" rows="4">g = graph![N(0) ^ (N(1) ^ (N(2) ^ n(0)))]</textarea>
    <button class="pg-btn pg-btn-run" id="btn-run">run</button>
  </div>
</div>
<div id="pg-output-wrap">
  <div id="pg-graph-panel">
    <svg id="pg-svg" width="100%" height="300"></svg>
  </div>
  <pre id="pg-result"></pre>
</div>
</div>

<style>
#playground-app { border: 1px solid var(--sidebar-bg); border-radius: 8px; padding: 16px; margin: 16px 0; }
#pg-controls { margin-bottom: 12px; }
#pg-examples { margin-bottom: 8px; }
.pg-btn { background: var(--sidebar-bg); color: var(--fg); border: 1px solid var(--sidebar-non-existant); padding: 4px 12px; border-radius: 4px; cursor: pointer; font-family: monospace; font-size: 13px; margin-right: 4px; }
.pg-btn:hover { background: var(--sidebar-active); }
.pg-btn-run { background: #2171b5; color: white; border: none; font-weight: bold; }
.pg-btn-dim { opacity: 0.6; }
#pg-editor-wrap { display: flex; gap: 8px; align-items: flex-start; }
#pg-input { flex: 1; background: var(--sidebar-bg); color: var(--fg); border: 1px solid var(--sidebar-non-existant); padding: 8px; font-family: 'Source Code Pro', monospace; font-size: 13px; border-radius: 4px; resize: vertical; }
#pg-output-wrap { display: flex; gap: 12px; min-height: 200px; }
#pg-graph-panel { flex: 1; background: white; border-radius: 6px; padding: 8px; min-height: 200px; }
#pg-result { flex: 1; font-size: 12px; margin: 0; padding: 8px; overflow: auto; max-height: 400px; background: var(--sidebar-bg); border-radius: 6px; }
#pg-svg text { font-family: monospace; }
</style>

<script type="module">
import init, { _eval as grwEval, list_graphs, get_graph, reset } from './playground/grw_playground.js';
await init('./playground/grw_playground_bg.wasm');

const EXAMPLES = {
    graph: 'g = graph![N(0) ^ (N(1) ^ (N(2) ^ n(0)))]',
    modify: 'modify!(g, [X(0) ^ N(3)])',
    search: 'search!(g, get(Mono) { N(0) ^ N(1) })',
};

function renderGraph(data) {
    const svg = document.getElementById('pg-svg');
    if (!data || !data.nodes) { svg.innerHTML = ''; return; }

    const nodes = data.nodes;
    const edges = data.edges;
    const n = nodes.length;
    if (n === 0) { svg.innerHTML = '<text x="50%" y="50%" text-anchor="middle" fill="#999">empty graph</text>'; return; }

    const w = svg.clientWidth || 400;
    const h = 300;
    const cx = w / 2, cy = h / 2;
    const r = Math.min(w, h) / 2 - 40;
    const nr = 18;

    const pos = {};
    nodes.forEach((nd, i) => {
        const angle = (2 * Math.PI * i / n) - Math.PI / 2;
        pos[nd.id] = {
            x: n === 1 ? cx : cx + r * Math.cos(angle),
            y: n === 1 ? cy : cy + r * Math.sin(angle)
        };
    });

    let html = '<defs><marker id="pg-arrow" markerWidth="10" markerHeight="7" refX="10" refY="3.5" orient="auto"><polygon points="0 0, 10 3.5, 0 7" fill="#2171b5"/></marker></defs>';

    edges.forEach(e => {
        const s = pos[e.src], t = pos[e.tgt];
        if (!s || !t) return;
        const isDir = e.dir === '>>';
        const col = isDir ? '#2171b5' : '#333';
        if (isDir) {
            const dx = t.x - s.x, dy = t.y - s.y;
            const len = Math.sqrt(dx*dx + dy*dy);
            if (len < 1) return;
            const ux = dx/len, uy = dy/len;
            html += `<line x1="${s.x + ux*nr}" y1="${s.y + uy*nr}" x2="${t.x - ux*(nr+6)}" y2="${t.y - uy*(nr+6)}" stroke="${col}" stroke-width="2" marker-end="url(#pg-arrow)"/>`;
        } else {
            html += `<line x1="${s.x}" y1="${s.y}" x2="${t.x}" y2="${t.y}" stroke="${col}" stroke-width="2"/>`;
        }
        if (e.val && e.val !== 'Unit' && e.val !== null) {
            html += `<text x="${(s.x+t.x)/2}" y="${(s.y+t.y)/2 - 6}" text-anchor="middle" font-size="10" fill="#666">${JSON.stringify(e.val)}</text>`;
        }
    });

    nodes.forEach(nd => {
        const p = pos[nd.id];
        html += `<circle cx="${p.x}" cy="${p.y}" r="${nr}" fill="#f0f0f0" stroke="#333" stroke-width="2"/>`;
        html += `<text x="${p.x}" y="${p.y + 4}" text-anchor="middle" font-size="12" fill="#333">${nd.id}</text>`;
        if (nd.val && nd.val !== 'Unit' && nd.val !== null) {
            html += `<text x="${p.x}" y="${p.y + nr + 14}" text-anchor="middle" font-size="9" fill="#666">${JSON.stringify(nd.val)}</text>`;
        }
    });

    svg.innerHTML = html;
}

function formatResult(parsed) {
    if (parsed.error) return `ERROR: ${parsed.error}`;
    if (parsed.matches) {
        return `${parsed.match_count} match(es)\n\n` +
            parsed.matches.slice(0, 20).map((m, i) =>
                `#${i}: ` + m.map(p => `${p.pattern}\u2192${p.graph}`).join(' ')
            ).join('\n') +
            (parsed.match_count > 20 ? `\n... (${parsed.match_count - 20} more)` : '');
    }
    if (parsed.result) {
        return `${parsed.result.node_count} nodes, ${parsed.result.edge_count} edges (${parsed.result.edge_type})`;
    }
    return JSON.stringify(parsed, null, 2);
}

function pgRun() {
    const input = document.getElementById('pg-input').value.trim();
    if (!input) return;
    const raw = grwEval(input);
    const parsed = JSON.parse(raw);
    document.getElementById('pg-result').textContent = formatResult(parsed);
    if (parsed.result) {
        renderGraph(parsed.result);
    } else if (parsed.ok && parsed.graph) {
        const gRaw = JSON.parse(get_graph(parsed.graph));
        if (gRaw.result) renderGraph(gRaw.result);
    }
}

document.getElementById('btn-run').addEventListener('click', pgRun);
document.getElementById('btn-reset').addEventListener('click', () => {
    reset();
    document.getElementById('pg-result').textContent = 'state reset';
    document.getElementById('pg-svg').innerHTML = '';
});
document.getElementById('btn-graph').addEventListener('click', () => {
    document.getElementById('pg-input').value = EXAMPLES.graph;
});
document.getElementById('btn-modify').addEventListener('click', () => {
    document.getElementById('pg-input').value = EXAMPLES.modify;
});
document.getElementById('btn-search').addEventListener('click', () => {
    document.getElementById('pg-input').value = EXAMPLES.search;
});
document.getElementById('pg-input').addEventListener('keydown', (e) => {
    if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) pgRun();
});

pgRun();

// disable mdbook keyboard navigation when editing
const input = document.getElementById('pg-input');
input.addEventListener('keydown', (e) => { e.stopPropagation(); });
input.addEventListener('keyup', (e) => { e.stopPropagation(); });
input.addEventListener('keypress', (e) => { e.stopPropagation(); });
</script>
