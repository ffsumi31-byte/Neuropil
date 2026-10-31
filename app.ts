import ForceGraph3D from '3d-force-graph';

interface NodeData {
  id: string;
  group: number;
  summary: string;
}

interface LinkData {
  source: string;
  target: string;
  weight: number;
}

interface BrainGraph {
  nodes: NodeData[];
  links: LinkData[];
}

const container = document.getElementById('brain-viewport') as HTMLElement;
const statusLabel = document.getElementById('node-info') as HTMLElement;

const Graph = ForceGraph3D()(container)
  .backgroundColor('#05050a')
  .nodeAutoColorBy('group')
  .nodeLabel((node: object) => {
    const n = node as NodeData;
    return `${n.id}: ${n.summary}`;
  })
  .linkDirectionalParticles(2)
  .linkDirectionalParticleSpeed(0.006)
  .onNodeClick((node: object) => {
    const n = node as NodeData;
    statusLabel.textContent = `${n.id} — ${n.summary}`;
  });

async function refreshBrain(): Promise<void> {
  const response = await fetch('http://127.0.0.1:8080/api/brain');
  const data: BrainGraph = await response.json();
  Graph.graphData(data);
}

refreshBrain();
setInterval(refreshBrain, 5000);
