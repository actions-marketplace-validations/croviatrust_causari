// Weekly measurements of AI code survival. Counts, not grades: no rank, no
// colour, no verdict. Rows are alphabetical; the reader judges.
(async function () {
  const tbody = document.querySelector("#lb tbody");
  const meta = document.getElementById("lb-meta");
  const SAMPLE_FLOOR = 5; // below this many AI-tagged commits a ratio is not shown

  try {
    const sources = [
      "https://raw.githubusercontent.com/croviatrust/causari/leaderboard-data/site/survival-data.json",
      "/survival-data.json",
    ];
    let data = null;
    for (const src of sources) {
      try {
        const res = await fetch(src, { cache: "no-cache" });
        if (res.ok) { data = await res.json(); break; }
      } catch (_) { /* try next source */ }
    }
    if (!data) throw new Error("no data source");

    const rows = (data.rows || [])
      .filter(r => r.total_commits > 0)
      .sort((a, b) => a.repo.localeCompare(b.repo));
    if (!rows.length) throw new Error("empty");

    const fmt = n => n.toLocaleString("en-US");
    const ratio = r => {
      const v = r.verified;
      if (v.introduced === 0) return '<span class="lb-none">— no git signal</span>';
      if (v.commits < SAMPLE_FLOOR) {
        return `<span class="lb-none" title="Fewer than ${SAMPLE_FLOOR} AI-tagged commits: a single commit can dominate, so no ratio is reported">— n &lt; ${SAMPLE_FLOOR}</span>`;
      }
      const pct = Math.round((v.survival_rate ?? 0) * 1000) / 10;
      return `<span class="lb-ratio">${pct}%</span>`;
    };
    const largest = r => {
      // The per-agent split is the only decomposition the current data carries;
      // when one agent holds nearly every introduced line, say so.
      const agents = Object.values(r.by_agent || {});
      if (!agents.length || r.verified.introduced === 0) return "";
      const max = Math.max(...agents.map(a => a.introduced || 0));
      const share = max / r.verified.introduced;
      return share >= 0.9 && agents.length > 1 ? `<span class="lb-note" title="${Math.round(share * 100)}% of introduced lines come from one agent">one agent ≥ 90%</span>` : "";
    };

    tbody.innerHTML = rows.map(r => `
      <tr>
        <td><a href="https://github.com/${r.repo}" rel="noopener">${r.repo}</a></td>
        <td>${fmt(r.total_commits)}</td>
        <td>${fmt(r.verified.commits)}${r.probable.commits ? ` <span class="muted">(+${fmt(r.probable.commits)} probable)</span>` : ""}</td>
        <td>${fmt(r.verified.introduced)}</td>
        <td>${fmt(r.verified.surviving)}</td>
        <td>${ratio(r)} ${largest(r)}</td>
        <td><code class="lb-repro" title="Click to copy" data-repo="${r.repo}">re audit ${r.repo}</code></td>
      </tr>`).join("");

    tbody.addEventListener("click", (e) => {
      const el = e.target.closest(".lb-repro");
      if (el) navigator.clipboard.writeText(`re audit ${el.dataset.repo}`);
    });

    if (data.generated_at) {
      meta.textContent = `Measured ${new Date(data.generated_at).toUTCString()} · ${rows.length} repositories · method v1 · alphabetical, unranked`;
    }
  } catch (e) {
    tbody.innerHTML = '<tr><td colspan="7" style="text-align:center;color:#64748b;">No measurement published yet. Run <code>re audit owner/repo</code> yourself.</td></tr>';
  }
})();
document.getElementById("year").textContent = new Date().getFullYear();
