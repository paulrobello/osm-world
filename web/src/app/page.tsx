export default function Home() {
  return (
    <main className="app-shell">
      <section className="panel control-panel" aria-labelledby="page-title">
        <div>
          <p className="eyebrow">osm-world web picker</p>
          <h1 id="page-title">
            Area <span className="accent">Picker</span>
          </h1>
          <p className="lede">
            Temporary scaffold for preparing real-world OSM and elevation cache inputs from a browser-based bounding box workflow.
          </p>
        </div>

        <div className="console-card" aria-label="Scaffold status">
          <div className="console-line">
            <strong>api</strong>
            <span>http://127.0.0.1:3030</span>
          </div>
          <div className="console-line">
            <strong>ui</strong>
            <span>OpenLayers picker arrives in Task 2</span>
          </div>
        </div>
      </section>

      <section className="map-stage" aria-label="Map preview placeholder" />
    </main>
  );
}
