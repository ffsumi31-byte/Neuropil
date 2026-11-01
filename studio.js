// Neuropil frontend: chat UI + a 3D viewer. All thinking (planning, geometry, OBJ export)
// happens in the Rust server; this file only draws what Rust returns.
import * as THREE from 'three';
import { OrbitControls } from 'three/addons/controls/OrbitControls.js';
import { TransformControls } from 'three/addons/controls/TransformControls.js';
import { GLTFExporter } from 'three/addons/exporters/GLTFExporter.js';

const $ = (s) => document.querySelector(s);
const $$ = (s) => [...document.querySelectorAll(s)];
const esc = (s) => String(s).replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));
const rich = (s) => esc(s).replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>').replace(/`(.+?)`/g, '<code>$1</code>').replace(/\n/g, '<br>');
const num = (v) => (Number.isFinite(v) ? Math.round(v * 1000) / 1000 : 0);
const isMobile = () => window.matchMedia('(max-width: 900px)').matches;

function toast(text) {
  const t = $('#toast');
  t.textContent = text;
  t.classList.add('show');
  clearTimeout(toast.timer);
  toast.timer = setTimeout(() => t.classList.remove('show'), 2200);
}

// ───────────────────────── Engine (Rust server) ─────────────────────────
class EngineError extends Error {}

async function engine(path, body, asText = false) {
  const ctrl = new AbortController();
  const timer = setTimeout(() => ctrl.abort(), 60000);
  let res;
  try {
    res = await fetch(path, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body), signal: ctrl.signal });
  } catch (e) {
    const where = location.protocol === 'file:' ? 'You opened index.html as a file — open the server URL instead.' : `Is the server running at ${location.origin}?`;
    throw new EngineError(e.name === 'AbortError'
      ? 'The Rust engine took too long (free Render servers sleep — wait ~30 s and retry).'
      : `Can't reach the Rust engine (${path}). ${where}`);
  } finally {
    clearTimeout(timer);
  }
  if (!res.ok) {
    const raw = await res.text();
    let detail = raw;
    try { const j = JSON.parse(raw); detail = j.details || j.error || raw; } catch { /* keep raw */ }
    throw new EngineError(`Engine returned HTTP ${res.status}. ${detail}`);
  }
  return asText ? res.text() : res.json();
}

async function pingEngine(tries = 0) {
  const dot = $('#engine-dot');
  const label = $('#engine-status');
  try {
    const r = await fetch('/api/health');
    const j = await r.json();
    dot.className = 'status-dot ok';
    label.textContent = `Rust engine v${j.version} · ${j.objects} objects`;
  } catch {
    dot.className = tries > 5 ? 'status-dot bad' : 'status-dot';
    label.textContent = tries > 5 ? 'Rust engine unreachable' : 'Rust engine waking up…';
    if (tries <= 5) setTimeout(() => pingEngine(tries + 1), 6000);
  }
}

// ───────────────────────── Sessions (chat history) ─────────────────────────
const KEY = 'neuropil.sessions.v2';
let sessions = [];
try { sessions = JSON.parse(localStorage.getItem(KEY) || '[]'); } catch { sessions = []; }
let cur = null;
const persist = () => { try { localStorage.setItem(KEY, JSON.stringify(sessions.slice(0, 30))); } catch { /* storage full or blocked */ } };

function newSession() {
  cur = { id: Date.now().toString(36), title: 'New build', messages: [], workspace: [] };
  sessions.unshift(cur);
}

function renderHistory() {
  $('#history').innerHTML = sessions.map((s) =>
    `<div class="h-item ${s === cur ? 'on' : ''}" data-id="${s.id}"><span>${esc(s.title)}</span><button class="x" data-del="${s.id}" title="Delete">✕</button></div>`).join('');
  $('#chat-title').textContent = cur.title;
  $('#vp-name').textContent = cur.title === 'New build' ? 'Model' : cur.title;
}

function renderChat() {
  const stream = $('#stream');
  $('#empty').classList.toggle('hidden', cur.messages.length > 0);
  stream.querySelectorAll('.msg').forEach((n) => n.remove());
  cur.messages.forEach((m, i) => {
    const el = document.createElement('div');
    el.className = `msg ${m.role}`;
    if (m.role === 'user') {
      el.innerHTML = `<div class="body">${rich(m.text)}</div>`;
    } else if (m.role === 'error') {
      el.innerHTML = `<div class="avatar">⚠</div><div class="body"><strong>Something went wrong</strong><br>${esc(m.text)}${m.detail ? `<pre>${esc(m.detail)}</pre>` : ''}</div>`;
    } else {
      const steps = (m.steps || []).map((s) => `<div class="${s.ok ? '' : 'bad'}">${s.ok ? '✓' : '✕'} ${esc(s.op)} — ${esc(s.detail)}</div>`).join('');
      const last = i === cur.messages.length - 1;
      const chips = last && m.suggestions ? `<div class="chips">${m.suggestions.map((s) => `<button class="chip" data-prompt="${esc(s)}">${esc(s)}</button>`).join('')}</div>` : '';
      el.innerHTML = `<div class="avatar"><span class="logo"></span></div><div class="body">${rich(m.text)}${steps ? `<details class="log"><summary>Rust engine log · ${m.steps.length} steps</summary>${steps}</details>` : ''}${chips}</div>`;
    }
    stream.appendChild(el);
  });
  stream.scrollTop = stream.scrollHeight;
}

function pushMsg(m) { cur.messages.push(m); renderChat(); persist(); }

function showTyping(on) {
  $('#typing')?.remove();
  if (!on) return;
  const el = document.createElement('div');
  el.id = 'typing';
  el.className = 'msg ai';
  el.innerHTML = '<div class="avatar"><span class="logo"></span></div><div class="body"><span class="typing"><i></i><i></i><i></i></span></div>';
  $('#stream').appendChild(el);
  $('#stream').scrollTop = $('#stream').scrollHeight;
}

function showError(err, title) {
  pushMsg({ role: 'error', text: err.message || String(err), detail: err instanceof EngineError ? '' : (err.stack || title || '') });
}

// ───────────────────────── 3D viewer ─────────────────────────
const host = $('#canvas-host');
const scene = new THREE.Scene();
scene.background = new THREE.Color('#101412');
const exportGroup = new THREE.Group();
const handleGroup = new THREE.Group();
scene.add(exportGroup, handleGroup);

const camera = new THREE.PerspectiveCamera(45, 1, 0.1, 1000);
camera.position.set(7, 6, 9);
const renderer = new THREE.WebGLRenderer({ antialias: true });
renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
host.appendChild(renderer.domElement);

scene.add(new THREE.GridHelper(40, 40, '#12879B', '#17221e'));
scene.add(new THREE.AmbientLight(0xffffff, 0.85));
const sun = new THREE.DirectionalLight(0xffffff, 1.6);
sun.position.set(10, 20, 14);
scene.add(sun);

const orbit = new OrbitControls(camera, renderer.domElement);
orbit.enableDamping = true;
orbit.target.set(0, 1, 0);

const gizmo = new TransformControls(camera, renderer.domElement);
gizmo.setTranslationSnap(0.25);
gizmo.setRotationSnap(THREE.MathUtils.degToRad(15));
gizmo.setScaleSnap(0.25);
scene.add(gizmo);

function resize() {
  const w = host.clientWidth, h = host.clientHeight;
  if (!w || !h) return;
  renderer.setSize(w, h);
  camera.aspect = w / h;
  camera.updateProjectionMatrix();
}
new ResizeObserver(resize).observe(host);

const QUAD_FACES = [[0, 3, 2, 1], [4, 5, 6, 7], [0, 1, 5, 4], [3, 7, 6, 2], [0, 4, 7, 3], [1, 2, 6, 5]];
let cubes = [];
let selected = null;
let editMode = 'cube';
let handles = [];
let activeHandle = null;
let history = [];
let framing = null;

function geometryFrom(corners) {
  const pos = [];
  QUAD_FACES.forEach((f) => {
    const [a, b, c, d] = f.map((i) => corners[i]);
    pos.push(...a, ...b, ...c, ...a, ...c, ...d);
  });
  const g = new THREE.BufferGeometry();
  g.setAttribute('position', new THREE.Float32BufferAttribute(pos, 3));
  g.computeVertexNormals();
  return g;
}

function materialFrom(color, mat) {
  return new THREE.MeshStandardMaterial({
    color: new THREE.Color(color),
    emissive: new THREE.Color(mat === 'neon' ? color : '#000000'),
    emissiveIntensity: mat === 'neon' ? 1.1 : 0,
    metalness: mat === 'metal' ? 0.85 : 0.15,
    roughness: mat === 'metal' ? 0.2 : 0.45,
    flatShading: true,
    polygonOffset: true, polygonOffsetFactor: 1, polygonOffsetUnits: 1,
  });
}

function addCube(c, delay) {
  const corners = c.corners.map((p) => [...p]);
  const mesh = new THREE.Mesh(geometryFrom(corners), materialFrom(c.color, c.mat));
  mesh.position.set(...c.pos);
  mesh.scale.set(...c.size);
  mesh.rotation.set(...c.rot.map((d) => THREE.MathUtils.degToRad(d)));
  const edges = new THREE.LineSegments(new THREE.EdgesGeometry(mesh.geometry, 15), new THREE.LineBasicMaterial({ color: 0x2dd4bf, transparent: true, opacity: 0.5 }));
  mesh.add(edges);
  mesh.userData = { name: c.name, role: c.role || '', part: c.part || '', group: c.group || '', color: c.color, mat: c.mat, corners, edges };
  if (delay !== undefined) {
    mesh.userData.pop = { t0: performance.now() + delay, to: [...c.size] };
    mesh.scale.setScalar(0.001);
  }
  exportGroup.add(mesh);
  cubes.push(mesh);
}

function disposeMesh(m) {
  m.geometry.dispose(); m.material.dispose();
  m.userData.edges.geometry.dispose(); m.userData.edges.material.dispose();
}

const getState = () => cubes.map((m) => ({
  name: m.userData.name, role: m.userData.role, part: m.userData.part, group: m.userData.group,
  pos: [m.position.x, m.position.y, m.position.z].map((v) => num(v)),
  size: [m.scale.x, m.scale.y, m.scale.z].map((v) => num(v)),
  rot: [m.rotation.x, m.rotation.y, m.rotation.z].map((r) => num(THREE.MathUtils.radToDeg(r))),
  color: m.userData.color, mat: m.userData.mat, corners: m.userData.corners,
}));

function applyWorkspace(list, changed = []) {
  const keep = selected ? selected.userData.name : null;
  gizmo.detach();
  clearHandles();
  cubes.forEach((m) => { exportGroup.remove(m); disposeMesh(m); });
  cubes = [];
  selected = null;
  const fresh = new Set(changed);
  let k = 0;
  list.forEach((c) => addCube(c, fresh.has(c.name) ? Math.min(k++ * 35, 1200) : undefined));
  const again = keep && cubes.find((m) => m.userData.name === keep);
  select(again || null);
  cur.workspace = list;
  persist();
  refreshUI();
}

function refreshUI() {
  $('#cube-counter').textContent = `${cubes.length} ${cubes.length === 1 ? 'cube' : 'cubes'}`;
  $('#empty-view').classList.toggle('hidden', cubes.length > 0 && cur.messages.length > 0);
}

function frameAll() {
  const box = new THREE.Box3();
  cubes.forEach((m) => {
    // cubes still popping in are tiny — measure them at their final size
    const p = m.userData.pop;
    const saved = m.scale.clone();
    if (p) m.scale.set(...p.to);
    m.updateMatrixWorld(true);
    box.expandByObject(m);
    if (p) m.scale.copy(saved);
  });
  if (box.isEmpty()) return;
  const center = box.getCenter(new THREE.Vector3());
  const size = box.getSize(new THREE.Vector3());
  framing = { center, dist: Math.max(size.x, size.y, size.z) * 1.7 + 3, until: performance.now() + 1400 };
}

function snapshot() {
  history.push(JSON.stringify(getState()));
  if (history.length > 40) history.shift();
}

function undo() {
  if (!history.length) return toast('Nothing to undo');
  applyWorkspace(JSON.parse(history.pop()));
  toast('Undone');
}

// selection + edit modes (corners / faces)
function select(mesh) {
  if (selected) selected.userData.edges.material.color.set(0x2dd4bf);
  selected = mesh;
  if (mesh) mesh.userData.edges.material.color.set(0xffffff);
  rebuildHandles();
  if (editMode === 'cube') { mesh ? gizmo.attach(mesh) : gizmo.detach(); }
  $('#insp-title').textContent = mesh ? mesh.userData.name : 'Nothing selected';
  $('#insp-sub').textContent = mesh ? `${mesh.userData.part || 'cube'}${mesh.userData.group ? ' · ' + mesh.userData.group : ''}` : 'Click a cube in the viewport';
  if (mesh) { $('#inp-color').value = mesh.userData.color.slice(0, 7); $('#inp-mat').value = mesh.userData.mat; }
}

function clearHandles() {
  handles.forEach((h) => handleGroup.remove(h));
  handles = [];
  activeHandle = null;
}

function rebuildHandles() {
  clearHandles();
  if (!selected || editMode === 'cube') return;
  selected.updateMatrixWorld();
  const corners = selected.userData.corners;
  if (editMode === 'corner') {
    const geo = new THREE.BoxGeometry(0.14, 0.14, 0.14);
    corners.forEach((pt, idx) => {
      const h = new THREE.Mesh(geo, new THREE.MeshBasicMaterial({ color: 0xfacc15 }));
      h.position.set(...pt).applyMatrix4(selected.matrixWorld);
      h.userData = { mode: 'corner', idx };
      handleGroup.add(h); handles.push(h);
    });
  } else {
    const geo = new THREE.BoxGeometry(0.18, 0.18, 0.18);
    QUAD_FACES.forEach((f) => {
      const center = new THREE.Vector3();
      f.forEach((ci) => center.add(new THREE.Vector3(...corners[ci])));
      center.divideScalar(4);
      const h = new THREE.Mesh(geo, new THREE.MeshBasicMaterial({ color: 0xf43f5e }));
      h.position.copy(center).applyMatrix4(selected.matrixWorld);
      h.userData = { mode: 'face', corners: f, lastWorld: h.position.clone() };
      handleGroup.add(h); handles.push(h);
    });
  }
}

function regenGeometry(mesh) {
  mesh.geometry.dispose();
  mesh.geometry = geometryFrom(mesh.userData.corners);
  mesh.userData.edges.geometry.dispose();
  mesh.userData.edges.geometry = new THREE.EdgesGeometry(mesh.geometry, 15);
}

gizmo.addEventListener('objectChange', () => {
  if (!selected || editMode === 'cube' || !activeHandle) return;
  const inv = selected.matrixWorld.clone().invert();
  const d = activeHandle.userData;
  if (d.mode === 'corner') {
    const loc = activeHandle.position.clone().applyMatrix4(inv);
    selected.userData.corners[d.idx] = [loc.x, loc.y, loc.z];
  } else {
    const delta = activeHandle.position.clone().applyMatrix4(inv).sub(d.lastWorld.clone().applyMatrix4(inv));
    d.corners.forEach((ci) => {
      selected.userData.corners[ci][0] += delta.x;
      selected.userData.corners[ci][1] += delta.y;
      selected.userData.corners[ci][2] += delta.z;
    });
    d.lastWorld.copy(activeHandle.position);
  }
  regenGeometry(selected);
});

let dragSnapshot = null;
gizmo.addEventListener('dragging-changed', (e) => {
  orbit.enabled = !e.value;
  if (e.value) { dragSnapshot = JSON.stringify(getState()); }
  else if (dragSnapshot) {
    history.push(dragSnapshot); dragSnapshot = null;
    cur.workspace = getState(); persist();
  }
});

function setEditMode(m) {
  editMode = m;
  ['cube', 'corner', 'face'].forEach((k) => $(`#mode-${k}`).classList.toggle('on', k === m));
  gizmo.detach();
  clearHandles();
  if (m === 'cube') { if (selected) gizmo.attach(selected); return; }
  rebuildHandles();
  if (handles[0]) { activeHandle = handles[0]; gizmo.attach(activeHandle); }
}

const raycaster = new THREE.Raycaster();
const pointer = new THREE.Vector2();
renderer.domElement.addEventListener('pointerdown', (e) => {
  if (e.button !== 0 || gizmo.dragging) return;
  const r = renderer.domElement.getBoundingClientRect();
  pointer.set(((e.clientX - r.left) / r.width) * 2 - 1, -((e.clientY - r.top) / r.height) * 2 + 1);
  raycaster.setFromCamera(pointer, camera);
  if (editMode !== 'cube') {
    const hit = raycaster.intersectObjects(handles, false)[0];
    if (hit) { activeHandle = hit.object; gizmo.attach(activeHandle); return; }
  }
  const hit = raycaster.intersectObjects(cubes, false)[0];
  if (hit) select(hit.object);
});

(function loop() {
  requestAnimationFrame(loop);
  const now = performance.now();
  cubes.forEach((m) => {
    const p = m.userData.pop;
    if (!p) return;
    const t = (now - p.t0) / 260;
    if (t < 0) { m.scale.setScalar(0.001); return; }
    if (t >= 1) { m.scale.set(...p.to); delete m.userData.pop; return; }
    const e = 1 - Math.pow(1 - t, 3);
    m.scale.set(p.to[0] * e, p.to[1] * e, p.to[2] * e);
  });
  if (framing) {
    const dir = camera.position.clone().sub(orbit.target).normalize();
    orbit.target.lerp(framing.center, 0.1);
    camera.position.lerp(framing.center.clone().add(dir.multiplyScalar(framing.dist)), 0.1);
    if (now > framing.until) framing = null;
  }
  orbit.update();
  renderer.render(scene, camera);
})();

// ───────────────────────── Sending prompts ─────────────────────────
let busy = false;

function openView() {
  if (isMobile()) $('#app').classList.add('view-open');
  else $('#app').classList.remove('no-view');
}

async function send(text) {
  text = text.trim();
  if (!text || busy) return;
  busy = true;
  $('#chat-send').disabled = true;
  if (cur.title === 'New build') cur.title = text.length > 38 ? text.slice(0, 38) + '…' : text;
  pushMsg({ role: 'user', text });
  renderHistory();
  showTyping(true);
  try {
    const before = JSON.stringify(getState());
    const r = await engine('/api/ai', { prompt: text, workspace: getState(), selected: selected ? selected.userData.name : null });
    showTyping(false);
    if (r.action === 'undo') {
      undo();
    } else if (r.intent !== 'chat' && r.intent !== 'unknown') {
      history.push(before);
      applyWorkspace(r.workspace, r.changed);
      if (r.changed.length) { frameAll(); openView(); }
    }
    pushMsg({ role: 'ai', text: r.reply, steps: r.steps, suggestions: r.suggestions });
  } catch (err) {
    showTyping(false);
    showError(err, 'AI request');
  } finally {
    busy = false;
    $('#chat-send').disabled = false;
    $('#chat-input').focus();
    renderHistory();
  }
}

async function runOp(body) {
  if (!selected) return toast('Select a cube first');
  try {
    const before = JSON.stringify(getState());
    const r = await engine('/api/op', { ...body, target: selected.userData.name, workspace: getState() });
    history.push(before);
    applyWorkspace(r.workspace, r.changed);
    toast(r.message);
  } catch (err) { showError(err, 'Inspector operation'); }
}

// ───────────────────────── UI wiring ─────────────────────────
const input = $('#chat-input');
input.addEventListener('input', () => { input.style.height = 'auto'; input.style.height = Math.min(input.scrollHeight, 180) + 'px'; });
input.addEventListener('keydown', (e) => {
  if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); $('#chat-send').click(); }
});
$('#chat-send').onclick = () => { const t = input.value; input.value = ''; input.style.height = 'auto'; send(t); };
document.addEventListener('click', (e) => {
  const prompt = e.target.closest('[data-prompt]');
  if (prompt) send(prompt.dataset.prompt);
  const del = e.target.closest('[data-del]');
  if (del) {
    e.stopPropagation();
    sessions = sessions.filter((s) => s.id !== del.dataset.del);
    if (!sessions.length) newSession();
    if (!sessions.includes(cur)) loadSession(sessions[0]);
    persist(); renderHistory();
    return;
  }
  const item = e.target.closest('.h-item');
  if (item) { loadSession(sessions.find((s) => s.id === item.dataset.id)); if (isMobile()) $('#app').classList.remove('side-open'); }
});

function loadSession(s) {
  if (!s) return;
  cur = s;
  history = [];
  applyWorkspace(s.workspace || []);
  renderHistory();
  renderChat();
}

$('#btn-new').onclick = () => { newSession(); history = []; applyWorkspace([]); renderHistory(); renderChat(); persist(); input.focus(); };
$('#btn-side-close').onclick = () => $('#app').classList.add('no-side');
$('#btn-side-open').onclick = () => (isMobile() ? $('#app').classList.toggle('side-open') : $('#app').classList.toggle('no-side'));
$('#btn-toggle-view').onclick = () => (isMobile() ? $('#app').classList.add('view-open') : $('#app').classList.toggle('no-view'));
$('#btn-view-close').onclick = () => { $('#app').classList.remove('view-open'); if (!isMobile()) $('#app').classList.add('no-view'); };
$('#btn-inspector').onclick = () => $('#inspector').classList.toggle('hidden');
$('#btn-undo').onclick = undo;
$('#btn-frame').onclick = frameAll;

$('#mode-cube').onclick = () => setEditMode('cube');
$('#mode-corner').onclick = () => setEditMode('corner');
$('#mode-face').onclick = () => setEditMode('face');
const gzModes = { 'gz-move': 'translate', 'gz-rotate': 'rotate', 'gz-scale': 'scale' };
Object.keys(gzModes).forEach((id) => {
  $('#' + id).onclick = () => {
    gizmo.setMode(gzModes[id]);
    Object.keys(gzModes).forEach((k) => $('#' + k).classList.toggle('on', k === id));
  };
});

$$('[data-push]').forEach((b) => { b.onclick = () => { const [axis, amt] = b.dataset.push.split(','); runOp({ op: 'push', axis, amount: parseFloat(amt) }); }; });
$$('[data-ext]').forEach((b) => { b.onclick = () => runOp({ op: 'extrude', face: b.dataset.ext, dist: 0.8, inset: 0.9 }); });
$$('[data-taper]').forEach((b) => { b.onclick = () => runOp({ op: 'taper', mode: b.dataset.taper, factor: 0.45 }); });
$('#btn-mirror').onclick = () => runOp({ op: 'mirror' });

$('#inp-color').oninput = (e) => {
  if (!selected) return;
  selected.userData.color = e.target.value;
  selected.material.dispose();
  selected.material = materialFrom(e.target.value, selected.userData.mat);
  cur.workspace = getState(); persist();
};
$('#inp-mat').onchange = (e) => {
  if (!selected) return;
  selected.userData.mat = e.target.value;
  selected.material.dispose();
  selected.material = materialFrom(selected.userData.color, e.target.value);
  cur.workspace = getState(); persist();
};
$('#btn-del').onclick = () => {
  if (!selected) return;
  snapshot();
  const gone = selected;
  gizmo.detach(); clearHandles();
  exportGroup.remove(gone); disposeMesh(gone);
  cubes = cubes.filter((m) => m !== gone);
  selected = null;
  select(null);
  cur.workspace = getState(); persist(); refreshUI();
};

document.addEventListener('keydown', (e) => {
  if (e.target.matches('textarea, input, select')) return;
  if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'z') { e.preventDefault(); undo(); return; }
  const k = e.key.toLowerCase();
  if (k === 'w') $('#gz-move').click();
  if (k === 'e') $('#gz-rotate').click();
  if (k === 'r') $('#gz-scale').click();
});

// exports
function download(blob, name) {
  const url = URL.createObjectURL(blob);
  const a = Object.assign(document.createElement('a'), { href: url, download: name, style: 'display:none' });
  document.body.appendChild(a);
  a.click();
  setTimeout(() => { a.remove(); URL.revokeObjectURL(url); }, 200);
}

function cleanExportScene() {
  if (!cubes.length) throw new Error('The workspace is empty — nothing to export.');
  const root = new THREE.Group();
  cubes.forEach((c) => {
    c.updateMatrixWorld(true);
    const g = c.geometry.clone();
    g.applyMatrix4(c.matrixWorld);
    const m = new THREE.Mesh(g, c.material.clone());
    m.name = c.userData.name;
    root.add(m);
  });
  return root;
}

$('#exp-obj').onclick = async () => {
  try {
    if (!cubes.length) throw new Error('The workspace is empty — nothing to export.');
    const obj = await engine('/api/export/obj', { workspace: getState() }, true);
    download(new Blob([obj], { type: 'text/plain;charset=utf-8' }), 'neuropil_model.obj');
    toast('OBJ exported (generated by the Rust engine)');
  } catch (err) { showError(err, 'OBJ export'); }
  $('.menu').removeAttribute('open');
};
$('#exp-gltf').onclick = () => {
  try {
    new GLTFExporter().parse(cleanExportScene(), (json) => {
      download(new Blob([JSON.stringify(json)], { type: 'application/json' }), 'neuropil_model.gltf');
      toast('GLTF exported');
    }, (err) => showError(err, 'GLTF export'));
  } catch (err) { showError(err, 'GLTF export'); }
  $('.menu').removeAttribute('open');
};
$('#exp-json').onclick = () => {
  download(new Blob([JSON.stringify(getState(), null, 2)], { type: 'application/json' }), 'neuropil_workspace.json');
  toast('Workspace saved');
  $('.menu').removeAttribute('open');
};

window.addEventListener('error', (e) => showError(e.error || new Error(e.message), 'Uncaught error'));
window.addEventListener('unhandledrejection', (e) => showError(e.reason instanceof Error ? e.reason : new Error(String(e.reason)), 'Unhandled rejection'));

// boot
if (!sessions.length) newSession();
cur = sessions[0];
renderHistory();
renderChat();
resize();
applyWorkspace(cur.workspace || []);
if (cubes.length) frameAll();
if (isMobile()) $('#app').classList.add('no-side');
else if (!cubes.length) $('#app').classList.add('no-view');
pingEngine();
