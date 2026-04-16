let lastFilteredWorkers = [];
let lastFilteredBlocks = [];
let lastInternalCpuWorker = null;

const CACHE_KEYS = {
  status: 'ks_bridge_status_v1',
  stats: 'ks_bridge_stats_v1',
  updatedMs: 'ks_bridge_updated_ms_v1',
};

const WALLET_FILTER_KEY = 'ks_bridge_wallet_filter_v1';
const WORKER_ORDER_KEY = 'ks_bridge_worker_order_v1';
const BLOCKS_DAY_FILTER_KEY = 'ks_bridge_blocks_day_filter_v1';
const GEO_APPROX_HIDDEN_KEY = 'rkstratum_hide_geo_approx_v1';

/** Last non-empty geo string from the server; used when applying hide/show preference. */
let __hostGeoApproxString = '';

function normalizeWalletFilter(value) {
  return String(value ?? '').trim();
}

function getWorkerKey(worker) {
  // Create a unique key for a worker based on instance, worker name, and wallet
  const instance = String(worker?.instance ?? '').trim();
  const workerName = String(worker?.worker ?? '').trim();
  const wallet = String(worker?.wallet ?? '').trim();
  return `${instance}|${workerName}|${wallet}`;
}

function readWorkerOrder() {
  try {
    const stored = localStorage.getItem(WORKER_ORDER_KEY);
    if (!stored) return [];
    return JSON.parse(stored);
  } catch {
    return [];
  }
}

function writeWorkerOrder(order) {
  try {
    localStorage.setItem(WORKER_ORDER_KEY, JSON.stringify(order || []));
  } catch {
    // ignore
  }
}

function maintainWorkerOrder(existingWorkers, newWorkers) {
  // existingWorkers: array of worker keys in the desired order
  // newWorkers: array of worker objects from API
  const order = [...existingWorkers];
  const seen = new Set(existingWorkers);
  const workerMap = new Map();
  
  // Create a map of worker key -> worker object
  for (const w of newWorkers) {
    const key = getWorkerKey(w);
    workerMap.set(key, w);
  }
  
  // Remove workers that no longer exist
  const filteredOrder = order.filter(key => workerMap.has(key));
  
  // Add new workers at the end
  for (const w of newWorkers) {
    const key = getWorkerKey(w);
    if (!seen.has(key)) {
      filteredOrder.push(key);
      seen.add(key);
    }
  }
  
  // Return sorted workers array based on the maintained order
  const sorted = [];
  for (const key of filteredOrder) {
    const worker = workerMap.get(key);
    if (worker) {
      sorted.push(worker);
    }
  }
  
  // Update stored order
  writeWorkerOrder(filteredOrder);
  
  return sorted;
}

function formatHashrateHs(hs) {
  if (!hs || !Number.isFinite(hs)) return '-';
  const units = ['H/s','kH/s','MH/s','GH/s','TH/s','PH/s','EH/s'];
  let v = hs;
  let i = 0;
  while (v >= 1000 && i < units.length - 1) { v /= 1000; i++; }
  return `${v.toFixed(2)} ${units[i]}`;
}

function setText(id, value) {
  const el = document.getElementById(id);
  if (!el) return;
  el.textContent = value == null || value === '' ? '-' : String(value);
}

function applyNodeSyncPill(node) {
  const pill = document.getElementById('nodeSyncPill');
  if (!pill) return;
  pill.classList.remove('rk-pill--synced', 'rk-pill--syncing', 'rk-pill--unknown', 'rk-pill--disconnected');
  if (!node || typeof node !== 'object') {
    pill.classList.add('rk-pill--unknown');
    return;
  }
  if (!node.isConnected) pill.classList.add('rk-pill--disconnected');
  else if (node.isSynced === true) pill.classList.add('rk-pill--synced');
  else if (node.isSynced === false) pill.classList.add('rk-pill--syncing');
  else pill.classList.add('rk-pill--unknown');
}

function setNodeSyncHealth(node) {
  const bar = document.getElementById('nodeSyncHealthBar');
  const pctEl = document.getElementById('nodeSyncHealthPct');
  if (!bar || !pctEl) return;
  if (!node || typeof node !== 'object') {
    bar.style.width = '0%';
    pctEl.textContent = '-';
    return;
  }
  if (!node.isConnected) {
    bar.style.width = '0%';
    pctEl.textContent = 'Offline';
    return;
  }
  if (node.isSynced === true) {
    bar.style.width = '100%';
    pctEl.textContent = '100% · Synced';
    return;
  }
  const dag = node.blockCount != null ? Number(node.blockCount) : NaN;
  const hdr = node.headerCount != null ? Number(node.headerCount) : NaN;
  if (Number.isFinite(dag) && Number.isFinite(hdr) && hdr > 0) {
    const ratio = Math.min(1, Math.max(0, dag / hdr));
    const p = ratio * 100;
    bar.style.width = `${p.toFixed(1)}%`;
    pctEl.textContent = `${p.toFixed(1)}% · Catching up`;
    return;
  }
  bar.style.width = '50%';
  pctEl.textContent = 'Syncing…';
}

function setProcessCpuBarPct(barEl, pctEl, cpuPct) {
  if (!barEl || !pctEl) return;
  if (Number.isFinite(Number(cpuPct)) && Number(cpuPct) >= 0) {
    const c = Math.min(100, Math.max(0, Number(cpuPct)));
    barEl.style.width = `${c.toFixed(1)}%`;
    pctEl.textContent = `${c.toFixed(1)}%`;
  } else {
    barEl.style.width = '0%';
    pctEl.textContent = '-';
  }
}

/** @param {'Bridge' | 'Kaspad'} suffix */
function fillHostMiningProcessBlock(suffix, pid, rssBytes, virtBytes, cpuPct) {
  setText(`host${suffix}ProcPid`, pid != null && pid !== '' ? String(pid) : '-');
  setText(`host${suffix}ProcRss`, rssBytes != null && Number(rssBytes) > 0 ? formatBytes(Number(rssBytes)) : '-');
  setText(`host${suffix}ProcVirt`, virtBytes != null && Number(virtBytes) > 0 ? formatBytes(Number(virtBytes)) : '-');
  setProcessCpuBarPct(
    document.getElementById(`host${suffix}ProcCpuBar`),
    document.getElementById(`host${suffix}ProcCpuPct`),
    cpuPct,
  );
}

function clearHostMiningProcessUi() {
  fillHostMiningProcessBlock('Bridge', null, null, null, NaN);
  fillHostMiningProcessBlock('Kaspad', null, null, null, NaN);
  const bridgeLbl = document.getElementById('hostBridgeProcLabel');
  if (bridgeLbl) bridgeLbl.textContent = 'Bridge';
  const tag = document.getElementById('hostBridgeProcTag');
  if (tag) tag.textContent = '';
  document.getElementById('hostKaspadProcRow')?.classList.add('hidden');
}

function setHostUtilizationBars(host) {
  const ramBar = document.getElementById('hostRamBar');
  const ramPct = document.getElementById('hostRamPct');
  const cpuBar = document.getElementById('hostCpuBar');
  const cpuPct = document.getElementById('hostCpuPct');
  const clearRam = () => {
    if (ramBar) ramBar.style.width = '0%';
    if (ramPct) ramPct.textContent = '-';
  };
  const clearCpu = () => {
    if (cpuBar) cpuBar.style.width = '0%';
    if (cpuPct) cpuPct.textContent = '-';
  };
  if (!host || typeof host !== 'object') {
    clearRam();
    clearCpu();
    return;
  }
  const tot = Number(host.memoryTotalBytes);
  const av = Number(host.memoryAvailableBytes);
  if (Number.isFinite(tot) && tot > 0 && Number.isFinite(av) && av >= 0) {
    const used = Math.min(Math.max(tot - av, 0), tot);
    const pct = (used / tot) * 100;
    if (ramBar) ramBar.style.width = `${Math.min(100, Math.max(0, pct)).toFixed(1)}%`;
    if (ramPct) ramPct.textContent = `${pct.toFixed(1)}%`;
  } else {
    clearRam();
  }
  const cpu = Number(host.globalCpuUsagePercent);
  if (Number.isFinite(cpu) && cpu >= 0) {
    const c = Math.min(100, Math.max(0, cpu));
    if (cpuBar) cpuBar.style.width = `${c.toFixed(1)}%`;
    if (cpuPct) cpuPct.textContent = `${c.toFixed(1)}%`;
  } else {
    clearCpu();
  }
}

function setInternalCpuCardsVisible(visible) {
  const hashrateEl = document.getElementById('internalCpuHashrate');
  const blocksEl = document.getElementById('internalCpuBlocks');
  const cards = [hashrateEl?.parentElement, blocksEl?.parentElement].filter(Boolean);
  for (const card of cards) {
    card.classList.toggle('hidden', !visible);
  }
}

function formatDifficulty(d) {
  const n = Number(d);
  if (!Number.isFinite(n) || n <= 0) return '-';
  // show in scientific-ish compact form similar to terminal
  if (n >= 1e12) return `${(n/1e12).toFixed(2)}T`;
  if (n >= 1e9) return `${(n/1e9).toFixed(2)}G`;
  if (n >= 1e6) return `${(n/1e6).toFixed(2)}M`;
  if (n >= 1e3) return `${(n/1e3).toFixed(2)}K`;
  return n.toFixed(2);
}

/** @type {number|null} */
window.__nodeUpdatedMs = null;
/** @type {number|null} */
window.__hostUpdatedMs = null;

function formatBytes(n) {
  const x = Number(n);
  if (!Number.isFinite(x) || x < 0) return '-';
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  let v = x;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i += 1;
  }
  return `${v.toFixed(i === 0 ? 0 : 1)} ${units[i]}`;
}

/** Exact byte count for tooltips (aligned with OS drive properties). */
function formatExactByteLabel(n) {
  const x = Math.round(Number(n));
  if (!Number.isFinite(x) || x < 0) return '';
  try {
    return `${x.toLocaleString()} bytes`;
  } catch {
    return `${x} bytes`;
  }
}

function populateNodePanel(node) {
  if (!document.getElementById('nodeSyncPill')) {
    return;
  }

  const errEl = document.getElementById('nodePanelError');
  if (errEl) {
    errEl.classList.add('hidden');
    errEl.textContent = '';
  }

  if (!node || typeof node !== 'object') {
    window.__nodeUpdatedMs = null;
    applyNodeSyncPill(null);
    setNodeSyncHealth(null);
    setText('nodeConnection', '-');
    setText('nodeSync', '-');
    setText('nodeNetwork', '-');
    setText('nodeDagBlocks', '-');
    setText('nodeHeaders', '-');
    setText('nodePeers', '-');
    setText('nodeDaa', '-');
    setText('nodeSinkBlue', '-');
    setText('nodeMempool', '-');
    setText('nodeDifficultyRpc', '-');
    setText('nodeTip', '-');
    setText('nodePollAge', '-');
    const tipEl = document.getElementById('nodeTip');
    if (tipEl) {
      tipEl.removeAttribute('title');
      tipEl.removeAttribute('aria-label');
    }
    const bc = document.getElementById('nodeTipCopy');
    if (bc) {
      bc.classList.add('hidden');
      bc.removeAttribute('data-copy-text');
    }
    return;
  }

  window.__nodeUpdatedMs = node.lastUpdatedUnixMs != null ? Number(node.lastUpdatedUnixMs) : null;

  setText('nodeConnection', node.isConnected ? 'Connected' : 'Disconnected');
  let sync = 'Unknown';
  if (node.isSynced === true) sync = 'Synced';
  else if (node.isSynced === false) sync = 'Syncing';
  setText('nodeSync', sync);
  applyNodeSyncPill(node);
  setNodeSyncHealth(node);
  setText('nodeNetwork', node.networkDisplay || node.networkId || '-');
  setText('nodeDagBlocks', node.blockCount != null ? String(node.blockCount) : '-');
  setText('nodeHeaders', node.headerCount != null ? String(node.headerCount) : '-');
  setText('nodePeers', node.peers != null ? String(node.peers) : '-');
  setText('nodeDaa', node.virtualDaaScore != null ? String(node.virtualDaaScore) : '-');
  setText('nodeSinkBlue', node.sinkBlueScore != null ? String(node.sinkBlueScore) : '-');
  setText('nodeMempool', node.mempoolSize != null ? String(node.mempoolSize) : '-');
  const d = node.difficulty;
  setText('nodeDifficultyRpc', d != null && Number.isFinite(Number(d)) ? formatDifficulty(Number(d)) : '-');

  const tip = node.tipHash ? String(node.tipHash) : '';
  setText('nodeTip', tip || '-');
  const tipEl = document.getElementById('nodeTip');
  if (tipEl) {
    if (tip) tipEl.setAttribute('title', tip);
    else tipEl.removeAttribute('title');
    if (tip) tipEl.setAttribute('aria-label', `Tip hash: ${tip}`);
    else tipEl.removeAttribute('aria-label');
  }
  const bc = document.getElementById('nodeTipCopy');
  if (bc) {
    if (tip) {
      bc.classList.remove('hidden');
      bc.setAttribute('data-copy-text', tip);
    } else {
      bc.classList.add('hidden');
      bc.removeAttribute('data-copy-text');
    }
  }

  updatePollAgeLabels();
}

function updateHostMetricsBanner(status) {
  const el = document.getElementById('hostMetricsBanner');
  if (!el) return;
  if (!status || typeof status !== 'object') {
    el.classList.add('hidden');
    return;
  }
  if (!('host_metrics_enabled' in status)) {
    el.classList.add('hidden');
    return;
  }
  const enabled = status.host_metrics_enabled === true;
  const hasHost = status.host && typeof status.host === 'object';
  if (!enabled) {
    el.classList.remove('hidden');
    el.textContent =
      'Bridge host metrics are not in this binary. Use a normal build (default Cargo features), or: cargo build -p kaspa-stratum-bridge --features rkstratum_host_metrics — minimal builds use --no-default-features.';
    return;
  }
  if (!hasHost) {
    el.classList.remove('hidden');
    el.textContent = 'Collecting host metrics; first sample appears shortly (refresh runs every ~20s)…';
    return;
  }
  el.classList.add('hidden');
}

function updateNodeDifficultyHint(stats, node) {
  const hint = document.getElementById('nodeDifficultyHint');
  if (!hint) return;
  const prom = Number(stats?.networkDifficulty);
  const rpc = node && node.difficulty != null ? Number(node.difficulty) : NaN;
  if (!Number.isFinite(prom) || prom <= 0 || !Number.isFinite(rpc) || rpc <= 0) {
    hint.classList.add('hidden');
    hint.textContent = '';
    return;
  }
  hint.classList.add('hidden');
  hint.textContent = '';
}

function applyHostGeoApproxRow(geo) {
  const trimmed = geo ? String(geo).trim() : '';
  __hostGeoApproxString = trimmed;
  const geoRow = document.getElementById('hostGeoRow');
  const showWrap = document.getElementById('hostGeoShowWrap');
  if (!geoRow) return;
  const hasGeo = Boolean(trimmed);
  if (!hasGeo) {
    geoRow.classList.add('hidden');
    geoRow.setAttribute('tabindex', '-1');
    geoRow.removeAttribute('aria-label');
    if (showWrap) showWrap.classList.add('hidden');
    setText('hostGeo', '-');
    return;
  }
  setText('hostGeo', trimmed);
  let prefHidden = false;
  try {
    prefHidden = localStorage.getItem(GEO_APPROX_HIDDEN_KEY) === '1';
  } catch {
    prefHidden = false;
  }
  if (prefHidden) {
    geoRow.classList.add('hidden');
    geoRow.setAttribute('tabindex', '-1');
    geoRow.removeAttribute('aria-label');
    if (showWrap) showWrap.classList.remove('hidden');
  } else {
    geoRow.classList.remove('hidden');
    geoRow.setAttribute('tabindex', '0');
    geoRow.setAttribute(
      'aria-label',
      `Approximate geo: ${trimmed}. Click this row to hide it on this browser; use “Show Geo (approx.)” to bring it back.`,
    );
    if (showWrap) showWrap.classList.add('hidden');
  }
}

function populateHostPanel(host) {
  const sec = document.getElementById('hostSection');
  if (!host || typeof host !== 'object') {
    if (sec) sec.classList.add('hidden');
    window.__hostUpdatedMs = null;
    setHostUtilizationBars(null);
    applyHostGeoApproxRow('');
    clearHostMiningProcessUi();
    const ph = document.getElementById('hostProcessProcHint');
    if (ph) {
      ph.textContent = '';
      ph.classList.add('hidden');
    }
    document.getElementById('hostOsRow')?.classList.add('hidden');
    document.getElementById('hostNodeStorageBlock')?.classList.add('hidden');
    document.getElementById('hostNodeDataRow')?.classList.add('hidden');
    document.getElementById('hostVolumeRow')?.classList.add('hidden');
    document.getElementById('hostVolumeSpaceBlock')?.classList.add('hidden');
    document.getElementById('hostVolumeFsRow')?.classList.add('hidden');
    const vsBar = document.getElementById('hostVolumeSpaceBar');
    if (vsBar) vsBar.style.width = '0%';
    const vsTrack = document.getElementById('hostVolumeSpaceTrack');
    if (vsTrack) {
      vsTrack.setAttribute('aria-valuenow', '0');
    }
    setText('hostVolumeCapacity', '—');
    setText('hostVolumeUsed', '—');
    setText('hostVolumeFree', '—');
    setText('hostVolumeFs', '—');
    document.getElementById('hostVolumeCapacity')?.removeAttribute('title');
    document.getElementById('hostVolumeUsed')?.removeAttribute('title');
    document.getElementById('hostVolumeFree')?.removeAttribute('title');
    setText('hostSwap', '-');
    setText('hostOs', '-');
    setText('hostNodeData', '-');
    setText('hostVolume', '-');
    return;
  }
  if (sec) sec.classList.remove('hidden');
  window.__hostUpdatedMs = host.lastUpdatedUnixMs != null ? Number(host.lastUpdatedUnixMs) : null;

  setText('hostHostname', host.hostname || '-');
  setText('hostCpu', host.cpuBrand || '-');
  setText('hostThreads', host.cpuLogicalCount != null ? String(host.cpuLogicalCount) : '-');
  const tot = host.memoryTotalBytes;
  const av = host.memoryAvailableBytes;
  setText('hostRam', tot != null && av != null ? `${formatBytes(av)} / ${formatBytes(tot)}` : '-');

  const l1 = Number(host.loadOne);
  const l5 = Number(host.loadFive);
  const l15 = Number(host.loadFifteen);
  const cpu = Number(host.globalCpuUsagePercent);
  let loadStr = '-';
  if (Number.isFinite(l1) && l1 > 0.001) {
    loadStr = `${l1.toFixed(2)} / ${Number.isFinite(l5) ? l5.toFixed(2) : '?'} / ${Number.isFinite(l15) ? l15.toFixed(2) : '?'} (1/5/15m)`;
  } else if (Number.isFinite(cpu)) {
    loadStr = `${cpu.toFixed(1)}% CPU (all cores)`;
  }
  setText('hostLoad', loadStr);
  setHostUtilizationBars(host);

  const st = Number(host.swapTotalBytes);
  const su = Number(host.swapUsedBytes);
  if (Number.isFinite(st) && st >= 0 && Number.isFinite(su) && su >= 0) {
    setText('hostSwap', `${formatBytes(su)} / ${formatBytes(st)}`);
  } else {
    setText('hostSwap', '-');
  }

  const osRow = document.getElementById('hostOsRow');
  const osName = host.hostOsName != null ? String(host.hostOsName).trim() : '';
  const osVer = host.hostOsVersion != null ? String(host.hostOsVersion).trim() : '';
  const osLine = [osName, osVer].filter(Boolean).join(' · ');
  if (osRow) {
    if (osLine) {
      osRow.classList.remove('hidden');
      setText('hostOs', osLine);
    } else {
      osRow.classList.add('hidden');
      setText('hostOs', '-');
    }
  }

  const storBlock = document.getElementById('hostNodeStorageBlock');
  const dataRow = document.getElementById('hostNodeDataRow');
  const volRow = document.getElementById('hostVolumeRow');
  const spaceBlock = document.getElementById('hostVolumeSpaceBlock');
  const spaceBar = document.getElementById('hostVolumeSpaceBar');
  const spaceTrack = document.getElementById('hostVolumeSpaceTrack');
  const dd = host.nodeDataDir != null ? String(host.nodeDataDir).trim() : '';
  const volTotal = host.nodeVolumeTotalBytes != null ? Number(host.nodeVolumeTotalBytes) : NaN;
  const volAvail = host.nodeVolumeAvailableBytes != null ? Number(host.nodeVolumeAvailableBytes) : NaN;
  const hasVolumeBytes =
    Number.isFinite(volTotal) && volTotal > 0 && Number.isFinite(volAvail) && volAvail >= 0;
  const volParts = [];
  if (host.nodeVolumeDiskKind != null && String(host.nodeVolumeDiskKind).trim()) {
    volParts.push(String(host.nodeVolumeDiskKind).trim());
  }
  if (host.nodeVolumeMount != null && String(host.nodeVolumeMount).trim()) {
    volParts.push(String(host.nodeVolumeMount).trim());
  }
  const vup = host.nodeVolumeUsedPercent != null ? Number(host.nodeVolumeUsedPercent) : NaN;
  if (Number.isFinite(vup) && !hasVolumeBytes) {
    volParts.push(`${vup.toFixed(1)}% used`);
  }
  const volLine = volParts.join(' · ');
  const hasStor = Boolean(dd || volLine || hasVolumeBytes);
  if (storBlock) storBlock.classList.toggle('hidden', !hasStor);
  if (dataRow) {
    if (dd) {
      dataRow.classList.remove('hidden');
      const nd = document.getElementById('hostNodeData');
      setText('hostNodeData', dd);
      if (nd) nd.setAttribute('title', dd);
    } else {
      dataRow.classList.add('hidden');
      setText('hostNodeData', '-');
    }
  }
  if (volRow) {
    if (volLine) {
      volRow.classList.remove('hidden');
      setText('hostVolume', volLine);
    } else {
      volRow.classList.add('hidden');
      setText('hostVolume', '-');
    }
  }

  if (spaceBlock && spaceBar) {
    if (hasVolumeBytes) {
      const used = Math.min(volTotal, Math.max(0, volTotal - volAvail));
      const free = Math.max(0, volAvail);
      const pctUsed = (used / volTotal) * 100;
      const pctClamped = Math.min(100, Math.max(0, pctUsed));
      spaceBlock.classList.remove('hidden');
      const diskKind =
        host.nodeVolumeDiskKind != null && String(host.nodeVolumeDiskKind).trim()
          ? String(host.nodeVolumeDiskKind).trim()
          : '';

      const fsStr =
        host.nodeVolumeFs != null && String(host.nodeVolumeFs).trim() ? String(host.nodeVolumeFs).trim() : '';
      const fsRow = document.getElementById('hostVolumeFsRow');
      if (fsRow) {
        if (fsStr) {
          fsRow.classList.remove('hidden');
          setText('hostVolumeFs', fsStr);
        } else {
          fsRow.classList.add('hidden');
          setText('hostVolumeFs', '—');
        }
      }

      const capEl = document.getElementById('hostVolumeCapacity');
      if (capEl) {
        capEl.textContent = formatBytes(volTotal);
        const capExact = formatExactByteLabel(volTotal);
        if (capExact) capEl.setAttribute('title', capExact);
        else capEl.removeAttribute('title');
      }
      const usedEl = document.getElementById('hostVolumeUsed');
      if (usedEl) {
        usedEl.textContent = `${formatBytes(used)} (${pctClamped.toFixed(1)}% used)`;
        const uEx = formatExactByteLabel(used);
        if (uEx) usedEl.setAttribute('title', uEx);
        else usedEl.removeAttribute('title');
      }
      const freeEl = document.getElementById('hostVolumeFree');
      if (freeEl) {
        freeEl.textContent = formatBytes(free);
        const fEx = formatExactByteLabel(free);
        if (fEx) freeEl.setAttribute('title', fEx);
        else freeEl.removeAttribute('title');
      }

      spaceBar.style.width = `${pctClamped.toFixed(1)}%`;
      if (spaceTrack) {
        spaceTrack.setAttribute('aria-valuenow', String(Math.round(pctClamped)));
        const diskHint = diskKind ? `${diskKind} · ` : '';
        spaceTrack.setAttribute(
          'aria-label',
          `${diskHint}Used space on appdir volume: ${pctClamped.toFixed(0)} percent`,
        );
      }
    } else {
      spaceBlock.classList.add('hidden');
      spaceBar.style.width = '0%';
      document.getElementById('hostVolumeFsRow')?.classList.add('hidden');
      setText('hostVolumeCapacity', '—');
      setText('hostVolumeUsed', '—');
      setText('hostVolumeFree', '—');
      setText('hostVolumeFs', '—');
      document.getElementById('hostVolumeCapacity')?.removeAttribute('title');
      document.getElementById('hostVolumeUsed')?.removeAttribute('title');
      document.getElementById('hostVolumeFree')?.removeAttribute('title');
      if (spaceTrack) {
        spaceTrack.setAttribute('aria-valuenow', '0');
      }
    }
  }

  const embeddedKaspad = host.embeddedKaspad === true;
  const externalNodeKnown = host.embeddedKaspad === false;

  const bridgeLbl = document.getElementById('hostBridgeProcLabel');
  if (bridgeLbl) bridgeLbl.textContent = embeddedKaspad ? 'Bridge + kaspad' : 'Bridge';
  const bridgeTag = document.getElementById('hostBridgeProcTag');
  if (bridgeTag) {
    bridgeTag.textContent = embeddedKaspad ? 'in-process' : '';
  }
  fillHostMiningProcessBlock(
    'Bridge',
    host.bridgePid,
    host.bridgeMemoryBytes,
    host.bridgeVirtualMemoryBytes,
    host.bridgeCpuUsagePercent,
  );
  const kRow = document.getElementById('hostKaspadProcRow');
  if (kRow) {
    if (externalNodeKnown && host.kaspadPid != null) {
      kRow.classList.remove('hidden');
      fillHostMiningProcessBlock(
        'Kaspad',
        host.kaspadPid,
        host.kaspadMemoryBytes,
        host.kaspadVirtualMemoryBytes,
        host.kaspadCpuUsagePercent,
      );
    } else {
      kRow.classList.add('hidden');
      fillHostMiningProcessBlock('Kaspad', null, null, null, NaN);
    }
  }
  const ph = document.getElementById('hostProcessProcHint');
  if (ph) {
    if (embeddedKaspad) {
      ph.textContent =
        'In-process mode: node and bridge share one OS process. Figures match what you would see for this PID in Task Manager / perf-monitor-style totals for the whole process.';
      ph.classList.remove('hidden');
    } else if (externalNodeKnown && host.kaspadPid == null) {
      ph.textContent =
        'No separate kaspad process found (looks for process name kaspad or kaspad.exe). If your node uses another binary name or runs remotely, only Bridge line applies.';
      ph.classList.remove('hidden');
    } else {
      ph.textContent = '';
      ph.classList.add('hidden');
    }
  }

  const locRow = document.getElementById('hostLocationRow');
  const loc = host.operatorLocation ? String(host.operatorLocation).trim() : '';
  if (locRow) {
    if (loc) {
      locRow.classList.remove('hidden');
      setText('hostLocation', loc);
    } else {
      locRow.classList.add('hidden');
    }
  }

  const geo = host.geoLocation ? String(host.geoLocation).trim() : '';
  applyHostGeoApproxRow(geo);

  updatePollAgeLabels();
}

function updatePollAgeLabels() {
  const nEl = document.getElementById('nodePollAge');
  const ms = window.__nodeUpdatedMs;
  if (nEl) {
    if (ms != null && Number.isFinite(ms) && ms > 0) {
      const sec = Math.max(0, (Date.now() - ms) / 1000);
      nEl.textContent = `Updated ${sec < 60 ? Math.floor(sec) + 's' : Math.floor(sec / 60) + 'm'} ago`;
    } else {
      nEl.textContent = '-';
    }
  }
  const hEl = document.getElementById('hostPollAge');
  const hm = window.__hostUpdatedMs;
  if (hEl) {
    if (hm != null && Number.isFinite(hm) && hm > 0) {
      const sec = Math.max(0, (Date.now() - hm) / 1000);
      hEl.textContent = `Updated ${sec < 60 ? Math.floor(sec) + 's' : Math.floor(sec / 60) + 'm'} ago`;
    } else {
      hEl.textContent = '-';
    }
  }
}

function shortHash(h) {
  if (!h) return '-';
  return h.length > 18 ? `${h.slice(0, 10)}...${h.slice(-6)}` : h;
}

function formatUnixSeconds(ts) {
  const n = Number(ts);
  if (!Number.isFinite(n) || n <= 0) return '-';
  try {
    return new Date(n * 1000).toLocaleString();
  } catch {
    return String(ts);
  }
}

function formatUptime(seconds) {
  const s = Number(seconds);
  if (!Number.isFinite(s) || s < 0) return '-';
  
  const days = Math.floor(s / 86400);
  const hours = Math.floor((s % 86400) / 3600);
  const minutes = Math.floor((s % 3600) / 60);
  const secs = Math.floor(s % 60);
  
  if (days > 0) {
    return `${days}d ${hours}h ${minutes}m`;
  } else if (hours > 0) {
    return `${hours}h ${minutes}m ${secs}s`;
  } else if (minutes > 0) {
    return `${minutes}m ${secs}s`;
  } else {
    return `${secs}s`;
  }
}

function formatRelativeTime(timestamp) {
  const ts = Number(timestamp);
  if (!Number.isFinite(ts) || ts <= 0) return '-';
  
  const now = Math.floor(Date.now() / 1000);
  const diff = now - ts;
  
  if (diff < 60) {
    return `${diff}s ago`;
  } else if (diff < 3600) {
    const mins = Math.floor(diff / 60);
    return `${mins}m ago`;
  } else if (diff < 86400) {
    const hours = Math.floor(diff / 3600);
    return `${hours}h ago`;
  } else {
    const days = Math.floor(diff / 86400);
    return `${days}d ago`;
  }
}

function getStatusColor(status) {
  switch (status?.toLowerCase()) {
    case 'online':
      return 'text-green-400';
    case 'idle':
      return 'text-yellow-400';
    case 'offline':
      return 'text-red-400';
    default:
      return 'text-gray-400';
  }
}

function getStatusBgColor(status) {
  switch (status?.toLowerCase()) {
    case 'online':
      return 'bg-green-500';
    case 'idle':
      return 'bg-yellow-500';
    case 'offline':
      return 'bg-red-500';
    default:
      return 'bg-gray-500';
  }
}

function formatServerTime(date, isMobile = false) {
  if (!date || !(date instanceof Date)) return '-';
  try {
    if (isMobile) {
      // Compact format for mobile: "Jan 15, 3:45 PM"
      const options = { 
        month: 'short', 
        day: 'numeric',
        hour: 'numeric',
        minute: '2-digit',
        hour12: true
      };
      return date.toLocaleString('en-US', options);
    } else {
      // Full format for desktop: "Mon, Jan 15, 2024 3:45:30 PM"
      const options = { 
        weekday: 'short', 
        year: 'numeric', 
        month: 'short', 
        day: 'numeric',
        hour: 'numeric',
        minute: '2-digit',
        second: '2-digit',
        hour12: true
      };
      return date.toLocaleString('en-US', options);
    }
  } catch {
    return date.toLocaleString();
  }
}

function updateServerTime() {
  const el = document.getElementById('serverTime');
  if (!el) return;
  // Check if mobile based on window width (matches Tailwind's md breakpoint: 768px)
  const isMobile = window.innerWidth < 768;
  el.textContent = formatServerTime(new Date(), isMobile);
  updatePollAgeLabels();
}

function getBlocksDayFilter() {
  const el = document.getElementById('blocksDayFilter');
  if (!el) return 0;
  const value = Number(el.value);
  return Number.isFinite(value) && value >= 0 ? value : 0;
}

function setBlocksDayFilter(value) {
  const el = document.getElementById('blocksDayFilter');
  if (!el) return;
  const v = Number(value);
  if (Number.isFinite(v) && v >= 0) {
    el.value = String(v);
    try {
      localStorage.setItem(BLOCKS_DAY_FILTER_KEY, String(v));
    } catch {
      // ignore
    }
  }
}

function getBlocksDayFilterFromStorage() {
  try {
    const stored = localStorage.getItem(BLOCKS_DAY_FILTER_KEY);
    if (!stored) return 0;
    const value = Number(stored);
    return Number.isFinite(value) && value >= 0 ? value : 0;
  } catch {
    return 0;
  }
}

function filterBlocksByDays(blocks, days) {
  if (!Array.isArray(blocks) || days <= 0) return blocks;
  const now = Math.floor(Date.now() / 1000);
  const cutoffSeconds = days * 24 * 60 * 60;
  const cutoffTime = now - cutoffSeconds;
  return blocks.filter(b => {
    const ts = Number(b?.timestamp);
    if (!Number.isFinite(ts) || ts <= 0) return false;
    return ts >= cutoffTime;
  });
}

function displayWorkerName(worker) {
  const w = String(worker ?? '').trim();
  if (w === 'InternalCPU') return 'RKStratum CPU Miner';
  return w || '-';
}

function escapeHtmlAttr(value) {
  return String(value ?? '')
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;');
}

function parseNonceToBigInt(nonce) {
  const s = String(nonce ?? '').trim();
  if (!s) return null;

  try {
    if (s.startsWith('0x') || s.startsWith('0X')) return BigInt(s);
  } catch {
    // fall through
  }

  if (/^[0-9]+$/.test(s)) {
    try {
      return BigInt(s);
    } catch {
      return null;
    }
  }

  if (/^[0-9a-fA-F]+$/.test(s)) {
    try {
      return BigInt('0x' + s);
    } catch {
      return null;
    }
  }

  return null;
}

function formatNonceInfo(nonce) {
  const bi = parseNonceToBigInt(nonce);
  if (!bi) {
    const raw = String(nonce ?? '');
    return { display: raw || '-', title: raw || '-' };
  }
  const dec = bi.toString(10);
  const hex = bi.toString(16);
  return {
    display: `0x${hex}`,
    title: `dec: ${dec}\nhex: 0x${hex}`,
  };
}

function escapeCsvCell(value) {
  const s = value == null ? '' : String(value);
  if (/[",\n\r]/.test(s)) return `"${s.replace(/"/g, '""')}"`;
  return s;
}

function downloadCsv(filename, rows) {
  const csv = rows.map(r => r.map(escapeCsvCell).join(',')).join('\n');
  const blob = new Blob([csv], { type: 'text/csv;charset=utf-8' });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = filename;
  document.body.appendChild(a);
  a.click();
  a.remove();
  URL.revokeObjectURL(url);
}

async function copyToClipboard(text) {
  const value = String(text ?? '');
  if (!value) return false;
  try {
    if (navigator.clipboard && window.isSecureContext) {
      await navigator.clipboard.writeText(value);
      return true;
    }
  } catch {
    // fall back below
  }

  try {
    const ta = document.createElement('textarea');
    ta.value = value;
    ta.setAttribute('readonly', '');
    ta.style.position = 'fixed';
    ta.style.left = '-9999px';
    document.body.appendChild(ta);
    ta.select();
    const ok = document.execCommand('copy');
    ta.remove();
    return ok;
  } catch {
    return false;
  }
}

function showToast(message) {
  const el = document.getElementById('toast');
  if (!el) return;
  el.textContent = message;
  el.classList.remove('hidden');
  clearTimeout(showToast._t);
  showToast._t = setTimeout(() => el.classList.add('hidden'), 1600);
}

function isCoarsePointerDevice() {
  try {
    return window.matchMedia && window.matchMedia('(pointer: coarse)').matches;
  } catch {
    return false;
  }
}

function openRowDetailModal(title, rows) {
  const modal = document.getElementById('rowDetailModal');
  const body = document.getElementById('rowDetailBody');
  const titleEl = document.getElementById('rowDetailTitle');
  if (!modal || !body || !titleEl) return;

  titleEl.textContent = title || 'Details';
  body.innerHTML = (rows || []).map(({ label, value, copyValue }) => {
    const v = value == null || value === '' ? '-' : String(value);
    const copy = copyValue != null && String(copyValue) !== ''
      ? `<button type="button" class="bg-surface-2 border border-card px-3 py-2 rounded-lg text-sm font-medium text-white hover:border-kaspa-primary shrink-0" data-copy-text="${escapeHtmlAttr(copyValue)}">Copy</button>`
      : '';
    return `
      <div class="bg-surface-2 border border-card rounded-xl px-4 py-3">
        <div class="flex items-start justify-between gap-3">
          <div class="min-w-0">
            <div class="text-xs text-gray-400">${escapeHtmlAttr(label)}</div>
            <div class="text-sm text-white break-all">${escapeHtmlAttr(v)}</div>
          </div>
          ${copy}
        </div>
      </div>
    `;
  }).join('');

  modal.classList.remove('hidden');
  try { document.body.style.overflow = 'hidden'; } catch {}
}

function closeRowDetailModal() {
  const modal = document.getElementById('rowDetailModal');
  if (!modal) return;
  modal.classList.add('hidden');
  try { document.body.style.overflow = ''; } catch {}
}

function cacheReadJson(key) {
  try {
    const raw = localStorage.getItem(key);
    if (!raw) return null;
    return JSON.parse(raw);
  } catch {
    return null;
  }
}

function cacheWriteJson(key, value) {
  try {
    localStorage.setItem(key, JSON.stringify(value));
  } catch {
    // ignore quota / disabled storage
  }
}

function readCachedSnapshot() {
  const status = cacheReadJson(CACHE_KEYS.status);
  const stats = cacheReadJson(CACHE_KEYS.stats);
  const updatedMs = Number(localStorage.getItem(CACHE_KEYS.updatedMs) || 0);
  if (!status || !stats) return null;
  return { status, stats, updatedMs };
}

function mergeBlockHistory(incomingBlocks, existingBlocks) {
  const byHash = new Map();
  for (const b of (existingBlocks || [])) {
    const h = b && b.hash;
    if (h) byHash.set(h, b);
  }
  for (const b of (incomingBlocks || [])) {
    const h = b && b.hash;
    if (h) byHash.set(h, b);
  }
  const merged = Array.from(byHash.values());
  merged.sort((a, b) => (Number(b.bluescore) || 0) - (Number(a.bluescore) || 0));
  return merged;
}

function cacheUpdate(status, stats) {
  const existing = cacheReadJson(CACHE_KEYS.stats);
  const mergedBlocks = mergeBlockHistory(stats?.blocks, existing?.blocks);
  const prevTotalBlocks = Number(existing?.totalBlocks);
  const incomingTotalBlocks = stats?.totalBlocks ?? stats?.total_blocks ?? stats?.totalblocks;
  const nextTotalBlocks = Number(incomingTotalBlocks);
  const mergedCount = Array.isArray(mergedBlocks) ? mergedBlocks.length : 0;
  const totalBlocksCandidates = [];
  if (Number.isFinite(prevTotalBlocks)) totalBlocksCandidates.push(prevTotalBlocks);
  if (Number.isFinite(nextTotalBlocks)) totalBlocksCandidates.push(nextTotalBlocks);
  if (Number.isFinite(mergedCount) && mergedCount > 0) totalBlocksCandidates.push(mergedCount);
  const totalBlocks = totalBlocksCandidates.length
    ? Math.max(...totalBlocksCandidates)
    : (incomingTotalBlocks ?? existing?.totalBlocks ?? mergedCount);

  // Render with full block history, but keep localStorage bounded to avoid quota issues.
  const CACHE_BLOCKS_MAX = 500;
  const statsToRender = { ...(stats || {}), totalBlocks, blocks: mergedBlocks };
  const statsToStore = { ...(stats || {}), totalBlocks, blocks: mergedBlocks.slice(0, CACHE_BLOCKS_MAX) };
  cacheWriteJson(CACHE_KEYS.status, status);
  cacheWriteJson(CACHE_KEYS.stats, statsToStore);
  try { localStorage.setItem(CACHE_KEYS.updatedMs, String(Date.now())); } catch {}
  return statsToRender;
}

function cacheClear() {
  try {
    localStorage.removeItem(CACHE_KEYS.status);
    localStorage.removeItem(CACHE_KEYS.stats);
    localStorage.removeItem(CACHE_KEYS.updatedMs);
  } catch {
    // ignore
  }
}

function setLastUpdated(updatedMs, isCached) {
  const el = document.getElementById('lastUpdated');
  if (!el) return;
  if (!updatedMs || !Number.isFinite(updatedMs) || updatedMs <= 0) {
    el.textContent = '-';
    el.removeAttribute('title');
    return;
  }
  const s = new Date(updatedMs).toLocaleString();
  const display = isCached ? `${s} (cached)` : s;
  el.textContent = display;
  el.title = display;
}

function displayTotalBlocksFromStats(stats) {
  const n = Number(stats?.totalBlocks ?? stats?.total_blocks ?? stats?.totalblocks);
  const blocksCount = Array.isArray(stats?.blocks) ? stats.blocks.length : 0;
  const candidates = [];
  if (Number.isFinite(n)) candidates.push(n);
  if (Number.isFinite(blocksCount) && blocksCount > 0) candidates.push(blocksCount);
  if (!candidates.length) return stats?.totalBlocks ?? stats?.total_blocks ?? stats?.totalblocks ?? blocksCount;
  return Math.max(...candidates);
}

function pickColors(n) {
  const base = ['#22c55e','#0ea5e9','#a855f7','#f59e0b','#ef4444','#14b8a6','#e11d48','#84cc16'];
  const out = [];
  for (let i = 0; i < n; i++) out.push(base[i % base.length]);
  return out;
}

function getWalletFilter() {
  return normalizeWalletFilter(document.getElementById('walletFilter')?.value);
}

function setWalletFilter(value) {
  const v = normalizeWalletFilter(value);
  const el = document.getElementById('walletFilter');
  if (el) el.value = v;
  try {
    if (v) localStorage.setItem(WALLET_FILTER_KEY, v);
    else localStorage.removeItem(WALLET_FILTER_KEY);
  } catch {
    // ignore
  }
}

function getWalletFilterFromStorage() {
  try {
    return normalizeWalletFilter(localStorage.getItem(WALLET_FILTER_KEY));
  } catch {
    return '';
  }
}

function renderWalletSummary(stats, filter) {
  const el = document.getElementById('walletSummary');
  if (!el) return;
  const f = normalizeWalletFilter(filter);
  if (!f) {
    el.classList.add('hidden');
    el.innerHTML = '';
    return;
  }

  const workersAll = Array.isArray(stats?.workers) ? stats.workers : [];
  const blocksAll = Array.isArray(stats?.blocks) ? stats.blocks : [];
  const workers = workersAll.filter(w => (w.wallet || '').includes(f));
  const blocks = blocksAll.filter(b => (b.wallet || '').includes(f));

  const activeWorkers = workers.length;
  const totalShares = workers.reduce((a, w) => a + (Number(w.shares) || 0), 0);
  const totalInvalid = workers.reduce((a, w) => a + (Number(w.invalid) || 0), 0);
  const totalStale = workers.reduce((a, w) => a + (Number(w.stale) || 0), 0);
  const totalHashrateHs = workers.reduce((a, w) => a + ((Number(w.hashrate) || 0) * 1e9), 0);

  el.classList.remove('hidden');
  el.innerHTML = `
    <div class="flex items-start justify-between gap-4">
      <div class="min-w-0">
        <div class="text-xs text-gray-400">Wallet</div>
        <div class="text-sm text-white break-all">${escapeHtmlAttr(f)}</div>
      </div>
      <div class="shrink-0">
        <button type="button" class="bg-surface-1 border border-card px-3 py-1.5 rounded-lg text-xs font-medium text-white hover:border-kaspa-primary" data-copy-text="${escapeHtmlAttr(f)}">Copy</button>
      </div>
    </div>
    <div class="mt-3 grid grid-cols-2 lg:grid-cols-4 gap-3 text-sm">
      <div class="bg-surface-1 border border-card rounded-lg px-3 py-2">
        <div class="text-xs text-gray-400">Workers</div>
        <div class="text-white font-semibold tabular-nums">${activeWorkers}</div>
      </div>
      <div class="bg-surface-1 border border-card rounded-lg px-3 py-2">
        <div class="text-xs text-gray-400">Blocks</div>
        <div class="text-white font-semibold tabular-nums">${blocks.length}</div>
      </div>
      <div class="bg-surface-1 border border-card rounded-lg px-3 py-2">
        <div class="text-xs text-gray-400">Hashrate</div>
        <div class="text-white font-semibold tabular-nums">${formatHashrateHs(totalHashrateHs)}</div>
      </div>
      <div class="bg-surface-1 border border-card rounded-lg px-3 py-2">
        <div class="text-xs text-gray-400">Shares (S/I)</div>
        <div class="text-white font-semibold tabular-nums">${totalShares} <span class="text-gray-400">(${totalStale}/${totalInvalid})</span></div>
      </div>
    </div>
  `;
}

const COLLAPSE_KEY = 'ks_bridge_collapsed_sections_v1';

function readCollapsedSections() {
  try {
    return JSON.parse(localStorage.getItem(COLLAPSE_KEY) || '{}') || {};
  } catch {
    return {};
  }
}

function writeCollapsedSections(map) {
  try {
    localStorage.setItem(COLLAPSE_KEY, JSON.stringify(map || {}));
  } catch {
    // ignore
  }
}

function setSectionCollapsed(id, collapsed) {
  const body = document.querySelector(`[data-collapsible-body="${id}"]`);
  const icon = document.querySelector(`[data-collapsible-icon="${id}"]`);
  const label = document.querySelector(`[data-collapsible-label="${id}"]`);
  const toggle = document.querySelector(`[data-collapsible-toggle="${id}"]`);
  if (!body) return;

  body.classList.toggle('hidden', !!collapsed);
  if (toggle) toggle.setAttribute('aria-expanded', collapsed ? 'false' : 'true');

  if (icon) {
    icon.classList.toggle('rotate-180', !!collapsed);
  }
  if (label) {
    label.textContent = collapsed ? 'Expand' : 'Collapse';
  }
}

function initCollapsibles() {
  const saved = readCollapsedSections();
  const ids = new Set();
  for (const el of document.querySelectorAll('[data-collapsible-body]')) {
    ids.add(el.getAttribute('data-collapsible-body'));
  }
  for (const id of ids) {
    const defaultCollapsed = false;
    const collapsed = id === 'raw' ? true : (saved[id] != null ? !!saved[id] : defaultCollapsed);
    setSectionCollapsed(id, collapsed);
  }
}

// --- Session trends (Chart.js): Grafana-style multi-series, data from /api/status host + node ---
const TRENDS_STORAGE_KEY = 'ks_bridge_trends_v1';
const MAX_TREND_POINTS = 900;
const TRENDS_VIEW_MODE_KEY = 'ks_bridge_trends_view_v1';
const TRENDS_EMBED_URL_KEY = 'ks_bridge_trends_embed_url_v1';

const rkTrends = { charts: {} };

function sanitizeTrendsEmbedUrl(raw) {
  const s = String(raw ?? '').trim();
  if (!s) return '';
  try {
    const u = new URL(s, window.location.href);
    if (u.protocol !== 'http:' && u.protocol !== 'https:') return '';
    u.username = '';
    u.password = '';
    return u.toString();
  } catch {
    return '';
  }
}

function readTrendsViewMode() {
  try {
    const v = localStorage.getItem(TRENDS_VIEW_MODE_KEY);
    return v === 'longrange' ? 'longrange' : 'session';
  } catch {
    return 'session';
  }
}

function writeTrendsViewMode(mode) {
  try {
    localStorage.setItem(TRENDS_VIEW_MODE_KEY, mode === 'longrange' ? 'longrange' : 'session');
  } catch {
    /* ignore */
  }
}

function readTrendsEmbedUrlStored() {
  try {
    return sanitizeTrendsEmbedUrl(localStorage.getItem(TRENDS_EMBED_URL_KEY) || '');
  } catch {
    return '';
  }
}

function writeTrendsEmbedUrlStored(url) {
  try {
    if (url) localStorage.setItem(TRENDS_EMBED_URL_KEY, url);
    else localStorage.removeItem(TRENDS_EMBED_URL_KEY);
  } catch {
    /* ignore */
  }
}

function setTrendsLongRangeIframeSrc(url) {
  const iframe = document.getElementById('trendsLongRangeIframe');
  const hint = document.getElementById('trendsLongRangeIframeHint');
  if (!iframe) return;
  const safe = sanitizeTrendsEmbedUrl(url);
  if (safe) {
    iframe.src = safe;
    if (hint) hint.classList.remove('hidden');
  } else {
    iframe.src = 'about:blank';
    if (hint) hint.classList.add('hidden');
  }
}

function getBridgeMetricsPageUrl() {
  if (typeof window.__RKSTRATUM_API_ORIGIN__ === 'string' && window.__RKSTRATUM_API_ORIGIN__) {
    return window.__RKSTRATUM_API_ORIGIN__.replace(/\/$/, '') + '/metrics';
  }
  try {
    return new URL('metrics', window.location.href).href;
  } catch {
    return `${window.location.origin}/metrics`;
  }
}

function buildPrometheusScrapeYaml() {
  let hostPort;
  let schemeLine = '';
  try {
    const u = new URL(getBridgeMetricsPageUrl());
    hostPort = u.port ? `${u.hostname}:${u.port}` : u.hostname;
    if (u.protocol === 'https:') {
      schemeLine = '\n  scheme: https';
    }
  } catch {
    const h = window.location.hostname || '127.0.0.1';
    const p = window.location.port;
    hostPort = p ? `${h}:${p}` : h;
    if (window.location.protocol === 'https:') {
      schemeLine = '\n  scheme: https';
    }
  }
  return `# Add under scrape_configs: in prometheus.yml
- job_name: kaspa-stratum-bridge
  scrape_interval: 15s
  metrics_path: /metrics${schemeLine}
  static_configs:
    - targets: ['${hostPort}']
`;
}

function updateTrendsLongRangeQuickUi() {
  const metricsUrl = getBridgeMetricsPageUrl();
  const display = document.getElementById('trendsMetricsScrapeUrlDisplay');
  if (display) display.textContent = metricsUrl;
  const link = document.getElementById('trendsLongRangeOpenMetricsLink');
  if (link) link.setAttribute('href', metricsUrl);
}

function resizeTrendCharts() {
  Object.values(rkTrends.charts).forEach((c) => {
    try {
      c.resize();
    } catch {
      /* ignore */
    }
  });
}

function applyTrendsViewMode(mode) {
  let isLong = mode === 'longrange';
  const sessionBtn = document.getElementById('trendsViewSessionBtn');
  const longBtn = document.getElementById('trendsViewLongRangeBtn');
  const sessionControls = document.getElementById('trendsSessionControls');
  const intro = document.getElementById('trendsIntroText');
  const longPanel = document.getElementById('trendsLongRangePanel');
  const sessionWrap = document.getElementById('trendsSessionChartsWrap');
  const longSub = document.getElementById('trendsLongRangeSub');
  const sessionSub = document.getElementById('trendsSessionChartsSub');
  const hasSessionUi = Boolean(sessionWrap || sessionSub || sessionBtn);
  const hasLongRangeUi = Boolean(longPanel || longSub || longBtn);

  if (isLong && !hasLongRangeUi) isLong = false;
  if (!isLong && !hasSessionUi && hasLongRangeUi) isLong = true;
  if (sessionBtn) {
    sessionBtn.classList.toggle('rk-trends-view-btn--active', !isLong);
    sessionBtn.setAttribute('aria-pressed', !isLong ? 'true' : 'false');
  }
  if (longBtn) {
    longBtn.classList.toggle('rk-trends-view-btn--active', isLong);
    longBtn.setAttribute('aria-pressed', isLong ? 'true' : 'false');
  }
  if (sessionControls) sessionControls.classList.toggle('hidden', isLong);
  if (intro) {
    intro.innerHTML = isLong
      ? ''
      : 'Session: live charts (~2s refresh; cleared when this tab closes). Long range: Prometheus helpers and optional embed. Bridge status is in the <strong class="text-gray-400 font-medium">nav above</strong>; host details in <strong class="text-gray-400 font-medium">Bridge host</strong> below.';
  }
  if (longPanel) longPanel.classList.toggle('hidden', !isLong);
  if (sessionWrap) sessionWrap.classList.toggle('hidden', isLong);
  if (longSub) longSub.classList.toggle('hidden', !isLong);
  if (sessionSub) {
    sessionSub.classList.toggle('hidden', isLong);
    sessionSub.classList.toggle('rk-subcollapsible--no-top-rule', !isLong);
  }
  writeTrendsViewMode(isLong ? 'longrange' : 'session');
  if (isLong) {
    updateTrendsLongRangeQuickUi();
    const input = document.getElementById('trendsLongRangeUrl');
    const stored = readTrendsEmbedUrlStored();
    if (input && stored && !String(input.value || '').trim()) input.value = stored;
    const fromInput = sanitizeTrendsEmbedUrl(input?.value || '');
    const toLoad = fromInput || stored;
    if (toLoad) setTrendsLongRangeIframeSrc(toLoad);
  } else {
    requestAnimationFrame(() => resizeTrendCharts());
  }
}

function computeNodeSyncPctForTrends(node) {
  if (!node || typeof node !== 'object' || !node.isConnected) return null;
  if (node.isSynced === true) return 100;
  const dag = node.blockCount != null ? Number(node.blockCount) : NaN;
  const hdr = node.headerCount != null ? Number(node.headerCount) : NaN;
  if (Number.isFinite(dag) && Number.isFinite(hdr) && hdr > 0) {
    return Math.min(100, Math.max(0, (dag / hdr) * 100));
  }
  return null;
}

function hostMemUsedGbTrends(host) {
  if (!host || typeof host !== 'object') return null;
  if (host.memoryUsedBytes != null) {
    const n = Number(host.memoryUsedBytes);
    if (Number.isFinite(n) && n >= 0) return n / 1024 ** 3;
  }
  const t = Number(host.memoryTotalBytes);
  const a = Number(host.memoryAvailableBytes);
  if (Number.isFinite(t) && t > 0 && Number.isFinite(a) && a >= 0) return (t - a) / 1024 ** 3;
  return null;
}

/** Same RAM "in use" % as the Bridge host card (`setHostUtilizationBars`). */
function hostMemUsedPercentTrends(host) {
  if (!host || typeof host !== 'object') return null;
  const tot = Number(host.memoryTotalBytes);
  const av = Number(host.memoryAvailableBytes);
  if (Number.isFinite(tot) && tot > 0 && Number.isFinite(av) && av >= 0) {
    const used = Math.min(Math.max(tot - av, 0), tot);
    return (used / tot) * 100;
  }
  if (host.memoryUsedBytes != null && Number.isFinite(Number(host.memoryUsedBytes)) && Number.isFinite(tot) && tot > 0) {
    return Math.min(100, Math.max(0, (Number(host.memoryUsedBytes) / tot) * 100));
  }
  return null;
}

function readTrendPoints() {
  try {
    const raw = sessionStorage.getItem(TRENDS_STORAGE_KEY);
    if (!raw) return [];
    const arr = JSON.parse(raw);
    return Array.isArray(arr) ? arr : [];
  } catch {
    return [];
  }
}

function writeTrendPoints(points) {
  try {
    sessionStorage.setItem(TRENDS_STORAGE_KEY, JSON.stringify(points.slice(-MAX_TREND_POINTS)));
  } catch {
    /* quota */
  }
}

function getTrendsWindowMs() {
  const v = Number(document.getElementById('trendsWindowSelect')?.value);
  return Number.isFinite(v) && v > 0 ? v : 900000;
}

function filterTrendsByWindow(points, winMs) {
  const cutoff = Date.now() - winMs;
  return points.filter((p) => p.t >= cutoff);
}

function fmtTrendCpu(n) {
  if (!Number.isFinite(n)) return '–';
  return `${n.toFixed(1)}%`;
}

function fmtTrendMemGb(n) {
  if (!Number.isFinite(n)) return '–';
  return `${n.toFixed(2)} GB`;
}

function fmtTrendMbps(n) {
  if (!Number.isFinite(n)) return '–';
  return `${n.toFixed(2)} MB/s`;
}

function fmtTrendInt(n) {
  if (!Number.isFinite(n)) return '–';
  return String(Math.round(n));
}

function seriesTriplet(arr, fmt) {
  const v = arr.filter((x) => Number.isFinite(x));
  if (!v.length) return '';
  const min = Math.min(...v);
  const max = Math.max(...v);
  const mean = v.reduce((a, b) => a + b, 0) / v.length;
  return `min ${fmt(min)} · mean ${fmt(mean)} · max ${fmt(max)}`;
}

function trendPlugins() {
  return {
    legend: {
      display: true,
      position: 'bottom',
      labels: { boxWidth: 10, boxHeight: 10, color: '#94a3b8', font: { size: 10 } },
    },
    tooltip: {
      backgroundColor: 'rgba(15, 23, 42, 0.94)',
      titleColor: '#e2e8f0',
      bodyColor: '#cbd5e1',
      borderColor: 'rgba(112, 199, 186, 0.25)',
      borderWidth: 1,
    },
  };
}

function trendScaleX() {
  return {
    display: true,
    grid: { color: 'rgba(148, 163, 184, 0.08)' },
    ticks: { color: '#64748b', maxRotation: 0, autoSkip: true, maxTicksLimit: 8, font: { size: 9 } },
  };
}

function trendScaleY(extra = {}) {
  return {
    display: true,
    grid: { color: 'rgba(148, 163, 184, 0.1)' },
    ticks: { color: '#64748b', font: { size: 9 } },
    ...extra,
  };
}

function updateOrCreateTrendChart(canvasId, type, data, options) {
  if (typeof Chart === 'undefined') return;
  const canvas = document.getElementById(canvasId);
  if (!canvas) return;
  const existing = rkTrends.charts[canvasId];
  if (existing) {
    const nOld = existing.data.datasets.length;
    const nNew = data.datasets.length;
    if (nOld !== nNew) {
      try {
        existing.destroy();
      } catch {
        /* ignore */
      }
      delete rkTrends.charts[canvasId];
    } else {
      existing.data.labels = data.labels;
      existing.data.datasets = data.datasets;
      existing.update('none');
      return;
    }
  }
  const ctx = canvas.getContext('2d');
  if (!ctx) return;
  rkTrends.charts[canvasId] = new Chart(ctx, { type, data, options });
}

function destroyRecentBlocksCharts() {
  for (const id of ['recentBlocksTimelineChart', 'recentBlocksWorkerChart']) {
    const c = rkTrends.charts[id];
    if (c) {
      try {
        c.destroy();
      } catch {
        /* ignore */
      }
      delete rkTrends.charts[id];
    }
  }
}

function bucketRecentBlocksTimeline(blocks) {
  const tsList = (blocks || [])
    .map((b) => Number(b?.timestamp))
    .filter((t) => Number.isFinite(t) && t > 0);
  if (!tsList.length) {
    return { labels: [], data: [], summary: '' };
  }
  const minT = Math.min(...tsList);
  const maxT = Math.max(...tsList);
  const spanSec = Math.max(0, maxT - minT);
  let bucketSec;
  let bucketWord;
  if (spanSec <= 1) {
    bucketSec = 3600;
    bucketWord = 'hour';
  } else if (spanSec < 3 * 3600) {
    bucketSec = 15 * 60;
    bucketWord = '15 min';
  } else if (spanSec < 48 * 3600) {
    bucketSec = 3600;
    bucketWord = 'hour';
  } else {
    bucketSec = 86400;
    bucketWord = 'day';
  }
  const buckets = new Map();
  for (const t of tsList) {
    const k = Math.floor(t / bucketSec) * bucketSec;
    buckets.set(k, (buckets.get(k) || 0) + 1);
  }
  const keys = [...buckets.keys()].sort((a, b) => a - b);
  const labelOpts =
    bucketSec >= 86400
      ? { month: 'short', day: 'numeric' }
      : bucketSec >= 3600
        ? { month: 'short', day: 'numeric', hour: '2-digit' }
        : { hour: '2-digit', minute: '2-digit' };
  const labels = keys.map((k) => new Date(k * 1000).toLocaleString(undefined, labelOpts));
  const data = keys.map((k) => buckets.get(k));
  const total = data.reduce((a, b) => a + b, 0);
  const peak = Math.max(...data);
  const mean = total / data.length;
  const summary = `Σ ${total} · peak ${peak} · μ ${mean.toFixed(1)} / ${bucketWord}`;
  return { labels, data, summary };
}

function aggregateRecentBlocksByWorker(blocks) {
  const m = new Map();
  for (const b of blocks || []) {
    const w = `${b.instance || '-'} / ${displayWorkerName(b.worker)}`;
    m.set(w, (m.get(w) || 0) + 1);
  }
  const items = [...m.entries()]
    .map(([label, value]) => ({ label, value }))
    .sort((a, b) => (b.value || 0) - (a.value || 0));
  const top = items.slice(0, 8);
  const rest = items.slice(8);
  const restTotal = rest.reduce((a, b) => a + (Number(b.value) || 0), 0);
  if (restTotal > 0) top.push({ label: 'Other', value: restTotal });
  return top;
}

function updateRecentBlocksViz(blocks) {
  const list = Array.isArray(blocks) ? blocks : [];
  const elCount = document.getElementById('recentBlocksStatCount');
  const elWorkers = document.getElementById('recentBlocksStatWorkers');
  const elSpan = document.getElementById('recentBlocksStatSpan');
  const elCharts = document.getElementById('recentBlocksVizCharts');
  const elEmpty = document.getElementById('recentBlocksVizEmpty');
  const elTlStats = document.getElementById('recentBlocksTimelineStats');
  const elWkStats = document.getElementById('recentBlocksWorkerStats');

  const workerNames = new Set();
  for (const b of list) {
    workerNames.add(displayWorkerName(b.worker) || '-');
  }
  const tsValid = list.map((b) => Number(b?.timestamp)).filter((t) => Number.isFinite(t) && t > 0);

  if (elCount) elCount.textContent = String(list.length);
  if (elWorkers) elWorkers.textContent = String(workerNames.size);

  if (elSpan) {
    if (tsValid.length >= 2) {
      const lo = Math.min(...tsValid);
      const hi = Math.max(...tsValid);
      const a = formatUnixSeconds(lo);
      const b = formatUnixSeconds(hi);
      elSpan.textContent = `${a} → ${b}`;
      elSpan.title = `${a} → ${b}`;
    } else if (tsValid.length === 1) {
      const s = formatUnixSeconds(tsValid[0]);
      elSpan.textContent = s;
      elSpan.title = s;
    } else {
      elSpan.textContent = '—';
      elSpan.title = '';
    }
  }

  if (!list.length) {
    destroyRecentBlocksCharts();
    if (elCharts) elCharts.classList.add('hidden');
    if (elEmpty) elEmpty.classList.remove('hidden');
    if (elTlStats) elTlStats.textContent = '';
    if (elWkStats) elWkStats.textContent = '';
    requestAnimationFrame(() => resizeTrendCharts());
    return;
  }

  if (elCharts) elCharts.classList.remove('hidden');
  if (elEmpty) elEmpty.classList.add('hidden');

  const tl = bucketRecentBlocksTimeline(list);
  let tlLabels = tl.labels;
  let tlData = tl.data;
  if (elTlStats) elTlStats.textContent = tl.summary;
  if (!tlLabels.length) {
    tlLabels = ['—'];
    tlData = [list.length];
    if (elTlStats) elTlStats.textContent = `${list.length} blocks (no valid timestamps for timeline)`;
  }

  const workersTop = aggregateRecentBlocksByWorker(list);
  const workerTotal = workersTop.reduce((a, i) => a + (Number(i.value) || 0), 0);
  if (elWkStats) {
    elWkStats.textContent =
      workersTop.length > 0
        ? `${workersTop.length} segment${workersTop.length === 1 ? '' : 's'} · ${workerTotal} blocks`
        : '';
  }

  if (typeof Chart === 'undefined') return;

  const yMax = Math.max(4, ...tlData);
  updateOrCreateTrendChart(
    'recentBlocksTimelineChart',
    'bar',
    {
      labels: tlLabels,
      datasets: [
        {
          label: 'Blocks',
          data: tlData,
          backgroundColor: 'rgba(45, 212, 191, 0.42)',
          borderColor: 'rgba(112, 199, 186, 0.85)',
          borderWidth: 1,
          borderRadius: 4,
          barPercentage: 0.88,
        },
      ],
    },
    {
      responsive: true,
      maintainAspectRatio: false,
      plugins: trendPlugins(),
      scales: {
        x: {
          ...trendScaleX(),
          ticks: { ...trendScaleX().ticks, maxRotation: 50, minRotation: 0 },
        },
        y: trendScaleY({
          beginAtZero: true,
          suggestedMax: yMax,
          ticks: {
            color: '#64748b',
            font: { size: 9 },
            precision: 0,
          },
        }),
      },
    },
  );

  updateOrCreateTrendChart(
    'recentBlocksWorkerChart',
    'doughnut',
    {
      labels: workersTop.map((i) => i.label),
      datasets: [
        {
          data: workersTop.map((i) => i.value),
          backgroundColor: pickColors(workersTop.length),
          borderWidth: 0,
          hoverOffset: 4,
        },
      ],
    },
    {
      responsive: true,
      maintainAspectRatio: false,
      cutout: '58%',
      plugins: {
        ...trendPlugins(),
        legend: {
          display: true,
          position: 'bottom',
          labels: { boxWidth: 10, boxHeight: 10, color: '#94a3b8', font: { size: 9 } },
        },
      },
    },
  );

  requestAnimationFrame(() => resizeTrendCharts());
}

const WORKER_VIZ_CHART_IDS = [
  'workerChartHashrate',
  'workerChartDifficulty',
  'workerChartSubmission',
  'workerChartSession',
];

function destroyWorkerChartById(canvasId) {
  const c = rkTrends.charts[canvasId];
  if (c) {
    try {
      c.destroy();
    } catch {
      /* ignore */
    }
    delete rkTrends.charts[canvasId];
  }
}

function destroyWorkersCharts() {
  for (const id of WORKER_VIZ_CHART_IDS) {
    destroyWorkerChartById(id);
  }
}

function shortWorkerVizLabel(w, idx) {
  const instRaw = String(w.instance ?? '').trim();
  const name = displayWorkerName(w.worker);
  let left = instRaw;
  if (!left) left = `[${idx + 1}]`;
  const s = `${left} ${name}`.trim();
  return s.length > 32 ? `${s.slice(0, 30)}…` : s;
}

function workerStatNum(w, camel, snake) {
  const v = w?.[camel] ?? w?.[snake];
  if (v == null || v === '') return 0;
  const n = Number(v);
  return Number.isFinite(n) ? n : 0;
}

function workerBalanceKasText(w) {
  const v = w?.balanceKas ?? w?.balance_kas;
  if (v == null || v === '') return '-';
  const n = Number(v);
  if (!Number.isFinite(n)) return '-';
  return n.toFixed(4);
}

/** Shorten long Kaspa addresses for table cells; full value stays in title and Copy. */
function truncateWalletForDisplay(wallet) {
  const s = String(wallet ?? '').trim();
  if (!s) return '-';
  if (s.length <= 22) return s;
  const head = 14;
  const tail = 10;
  if (head + tail + 3 >= s.length) return `${s.slice(0, 20)}…`;
  return `${s.slice(0, head)}…${s.slice(-tail)}`;
}

function workerWalletCellHtml(w) {
  const wallet = String(w?.wallet ?? '').trim();
  const title = escapeHtmlAttr(wallet);
  const display = escapeHtmlAttr(truncateWalletForDisplay(wallet));
  const btn = wallet
    ? `<button type="button" class="bg-surface-1 border border-card px-1.5 py-0.5 rounded text-[0.65rem] sm:text-xs hover:border-kaspa-primary shrink-0" data-copy-text="${escapeHtmlAttr(wallet)}">Copy</button>`
    : '';
  return `<div class="flex items-center gap-1 min-w-0">
            <span class="min-w-0 flex-1 truncate" title="${title}">${display}</span>
            ${btn}
          </div>`;
}

function workerShareThroughErrorTds(w) {
  const dup = workerStatNum(w, 'duplicateShares', 'duplicate_shares');
  const weak = workerStatNum(w, 'weakShares', 'weak_shares');
  const acc = workerStatNum(w, 'blocksAcceptedByNode', 'blocks_accepted_by_node');
  const nblue = workerStatNum(w, 'blocksNotConfirmedBlue', 'blocks_not_confirmed_blue');
  const disc = workerStatNum(w, 'disconnects', 'disconnects');
  const jobs = workerStatNum(w, 'jobs', 'jobs');
  const err = workerStatNum(w, 'errors', 'errors');
  const bal = workerBalanceKasText(w);
  return `
        <td class="py-1.5 tabular-nums">${w.shares ?? '-'}</td>
        <td class="py-1.5 tabular-nums rk-wt-aux">${w.stale ?? '-'}</td>
        <td class="py-1.5 tabular-nums rk-wt-aux">${w.invalid ?? '-'}</td>
        <td class="py-1.5 tabular-nums rk-wt-aux" title="Duplicate shares">${dup}</td>
        <td class="py-1.5 tabular-nums rk-wt-aux" title="Weak shares">${weak}</td>
        <td class="py-1.5 tabular-nums">${w.blocks ?? '-'}</td>
        <td class="py-1.5 tabular-nums rk-wt-aux" title="Blocks accepted by node">${acc}</td>
        <td class="py-1.5 tabular-nums rk-wt-aux" title="Blocks not confirmed blue">${nblue}</td>
        <td class="py-1.5 tabular-nums rk-wt-aux" title="Disconnects">${disc}</td>
        <td class="py-1.5 tabular-nums rk-wt-phone-hide" title="Jobs sent">${jobs}</td>
        <td class="py-1.5 tabular-nums rk-wt-phone-hide" title="Balance (KAS)">${bal}</td>
        <td class="py-1.5 tabular-nums rk-wt-aux" title="Errors">${err}</td>`;
}

function internalCpuShareThroughErrorTds({ shares, stale, invalid, blocksAccepted }) {
  const b = Number(blocksAccepted) || 0;
  return `
        <td class="py-1.5 tabular-nums">${shares}</td>
        <td class="py-1.5 tabular-nums rk-wt-aux">${stale}</td>
        <td class="py-1.5 tabular-nums rk-wt-aux">${invalid}</td>
        <td class="py-1.5 tabular-nums rk-wt-aux" title="Duplicate shares">0</td>
        <td class="py-1.5 tabular-nums rk-wt-aux" title="Weak shares">0</td>
        <td class="py-1.5 tabular-nums">${b}</td>
        <td class="py-1.5 tabular-nums rk-wt-aux" title="Blocks accepted by node">${b}</td>
        <td class="py-1.5 tabular-nums rk-wt-aux" title="Blocks not confirmed blue">-</td>
        <td class="py-1.5 tabular-nums rk-wt-aux" title="Disconnects">-</td>
        <td class="py-1.5 tabular-nums" title="Jobs sent">-</td>
        <td class="py-1.5 tabular-nums" title="Balance (KAS)">-</td>
        <td class="py-1.5 tabular-nums rk-wt-aux" title="Errors">-</td>`;
}

function workerStatusSessionTds(w) {
  const statusHtml = w.status
    ? `<span class="inline-flex items-center gap-1.5"><span class="w-2 h-2 rounded-full ${getStatusBgColor(w.status)}"></span><span class="${getStatusColor(w.status)} capitalize">${escapeHtmlAttr(w.status)}</span></span>`
    : '-';
  return `
        <td class="py-1.5 rk-wt-status">${statusHtml}</td>
        <td class="py-1.5 rk-wt-phone-hide" title="${w.lastSeen ? formatUnixSeconds(w.lastSeen) : ''}">${w.lastSeen ? formatRelativeTime(w.lastSeen) : '-'}</td>
        <td class="py-1.5 rk-wt-phone-hide" title="${w.sessionUptime != null ? formatUptime(w.sessionUptime) : ''}">${w.sessionUptime != null ? formatUptime(w.sessionUptime) : '-'}</td>`;
}

function buildWorkerDataRowHtml(w) {
  const workerDisplay = displayWorkerName(w.worker);
  return (
    `<td class="py-1.5" title="${escapeHtmlAttr(w.instance || '')}">${escapeHtmlAttr(w.instance || '-')}</td>` +
    `<td class="py-1.5" title="${escapeHtmlAttr(workerDisplay)}">${escapeHtmlAttr(workerDisplay)}</td>` +
    `<td class="py-1.5 rk-wt-wallet align-top">${workerWalletCellHtml(w)}</td>` +
    `<td class="py-1.5" title="${escapeHtmlAttr(formatHashrateHs((w.hashrate || 0) * 1e9))}">${formatHashrateHs((w.hashrate || 0) * 1e9)}</td>` +
    `<td class="py-1.5">${w.currentDifficulty != null ? formatDifficulty(w.currentDifficulty) : '-'}</td>` +
    workerShareThroughErrorTds(w) +
    workerStatusSessionTds(w)
  );
}

function internalCpuWorkerRowHtml(icpu) {
  const hashrateHs = (Number(icpu.hashrateGhs) || 0) * 1e9;
  const wallet = String(icpu.wallet ?? '').trim();
  const shares = Number(icpu.shares ?? icpu.blocksAccepted) || 0;
  const stale = Number(icpu.stale ?? ((Number(icpu.blocksSubmitted) || 0) - (Number(icpu.blocksAccepted) || 0))) || 0;
  const invalid = Number(icpu.invalid ?? 0) || 0;
  const b = Number(icpu.blocksAccepted) || 0;
  return (
    `<td class="py-1.5">-</td>` +
    `<td class="py-1.5">${escapeHtmlAttr(displayWorkerName('InternalCPU'))}</td>` +
    `<td class="py-1.5 rk-wt-wallet align-top">${workerWalletCellHtml({ wallet })}</td>` +
    `<td class="py-1.5" title="${escapeHtmlAttr(formatHashrateHs(hashrateHs))}">${formatHashrateHs(hashrateHs)}</td>` +
    `<td class="py-1.5">-</td>` +
    internalCpuShareThroughErrorTds({ shares, stale, invalid, blocksAccepted: b }) +
    `<td class="py-1.5 rk-wt-status">-</td><td class="py-1.5 rk-wt-phone-hide">-</td><td class="py-1.5 rk-wt-phone-hide">-</td>`
  );
}

function setMiningBlockSubtotals(stats) {
  const el = document.getElementById('blockTotalsSub');
  if (!el) return;
  const rawA = stats?.totalBlocksAcceptedByNode ?? stats?.total_blocks_accepted_by_node;
  const rawN = stats?.totalBlocksNotConfirmedBlue ?? stats?.total_blocks_not_confirmed_blue;
  const a = rawA == null || rawA === '' ? 0 : Number(rawA);
  const n = rawN == null || rawN === '' ? 0 : Number(rawN);
  el.textContent = `Node accepted ${Number.isFinite(a) ? a : 0} · Not blue ${Number.isFinite(n) ? n : 0}`;
}

function buildWorkerVizRows(workers, icpu) {
  const rows = [];
  if (icpu && typeof icpu === 'object') {
    const hashrateGhs = Number(icpu.hashrateGhs) || 0;
    const shares = Number(icpu.shares ?? icpu.blocksAccepted) || 0;
    const stale =
      Number(icpu.stale ?? ((Number(icpu.blocksSubmitted) || 0) - (Number(icpu.blocksAccepted) || 0))) || 0;
    const invalid = Number(icpu.invalid ?? 0) || 0;
    const blocks = Number(icpu.blocksAccepted) || 0;
    rows.push({
      label: displayWorkerName('InternalCPU'),
      hashrateGhs,
      difficulty: null,
      shares,
      stale,
      invalid,
      duplicateShares: 0,
      weakShares: 0,
      blocks,
      blocksAcceptedByNode: blocks,
      blocksNotConfirmedBlue: 0,
      sessionUptimeSec: null,
    });
  }
  let idx = 0;
  for (const w of workers || []) {
    const cd = w.currentDifficulty;
    const diff = cd != null && Number.isFinite(Number(cd)) ? Number(cd) : null;
    rows.push({
      label: shortWorkerVizLabel(w, idx),
      hashrateGhs: Number(w.hashrate) || 0,
      difficulty: diff,
      shares: Number(w.shares) || 0,
      stale: Number(w.stale) || 0,
      invalid: Number(w.invalid) || 0,
      duplicateShares: workerStatNum(w, 'duplicateShares', 'duplicate_shares'),
      weakShares: workerStatNum(w, 'weakShares', 'weak_shares'),
      blocks: Number(w.blocks) || 0,
      blocksAcceptedByNode: workerStatNum(w, 'blocksAcceptedByNode', 'blocks_accepted_by_node'),
      blocksNotConfirmedBlue: workerStatNum(w, 'blocksNotConfirmedBlue', 'blocks_not_confirmed_blue'),
      sessionUptimeSec:
        w.sessionUptime != null && Number.isFinite(Number(w.sessionUptime))
          ? Number(w.sessionUptime)
          : null,
    });
    idx += 1;
  }
  return rows;
}

function workersHorizBarOptions(extraScales = {}) {
  return {
    indexAxis: 'y',
    responsive: true,
    maintainAspectRatio: false,
    plugins: trendPlugins(),
    scales: {
      x: {
        type: 'linear',
        beginAtZero: true,
        grid: { color: 'rgba(148, 163, 184, 0.1)' },
        ticks: { color: '#64748b', font: { size: 9 } },
        ...extraScales.x,
      },
      y: {
        type: 'category',
        grid: { display: false },
        ticks: { color: '#94a3b8', font: { size: 9 }, autoSkip: false },
        ...extraScales.y,
      },
    },
  };
}

function applyWorkersChartHeights(rowCount) {
  const n = Math.max(1, Number(rowCount) || 1);
  const narrow = Math.min(520, Math.max(168, 64 + n * 28));
  const wide = Math.min(680, Math.max(260, 112 + n * 38));
  for (const el of document.querySelectorAll('.rk-workers-viz .rk-workers-chart-canvas-wrap')) {
    el.style.height = `${el.classList.contains('rk-workers-chart-canvas-wrap--wide') ? wide : narrow}px`;
  }
}

function updateWorkersViz(rows) {
  const list = Array.isArray(rows) ? rows : [];
  const elCharts = document.getElementById('workersVizCharts');
  const elEmpty = document.getElementById('workersVizEmpty');
  const diffCard = document.getElementById('workersCardDifficulty');
  const sessCard = document.getElementById('workersCardSession');

  const sumHrGhs = list.reduce((a, r) => a + (Number(r.hashrateGhs) || 0), 0);
  const sumShares = list.reduce((a, r) => a + (Number(r.shares) || 0), 0);
  const sumBlocks = list.reduce((a, r) => a + (Number(r.blocks) || 0), 0);

  const elHr = document.getElementById('workersStatHashrateSum');
  if (elHr) elHr.textContent = sumHrGhs > 0 ? formatHashrateHs(sumHrGhs * 1e9) : '—';
  const elCnt = document.getElementById('workersStatCount');
  if (elCnt) elCnt.textContent = String(list.length);
  const elSh = document.getElementById('workersStatSharesSum');
  if (elSh) elSh.textContent = String(sumShares);
  const elBl = document.getElementById('workersStatBlocksSum');
  if (elBl) elBl.textContent = String(sumBlocks);

  const setStat = (id, text) => {
    const el = document.getElementById(id);
    if (el) el.textContent = text;
  };

  if (!list.length) {
    destroyWorkersCharts();
    if (elCharts) elCharts.classList.add('hidden');
    if (elEmpty) elEmpty.classList.remove('hidden');
    diffCard?.classList.add('hidden');
    sessCard?.classList.add('hidden');
    setStat('workersChartHashrateStats', '');
    setStat('workersChartDifficultyStats', '');
    setStat('workersChartSubmissionStats', '');
    setStat('workersChartSessionStats', '');
    requestAnimationFrame(() => resizeTrendCharts());
    return;
  }

  if (elCharts) elCharts.classList.remove('hidden');
  if (elEmpty) elEmpty.classList.add('hidden');

  const labels = list.map((r) => r.label);
  const hashrates = list.map((r) => r.hashrateGhs);
  const difficulties = list.map((r) => (r.difficulty != null ? r.difficulty : null));
  const hasDiff = difficulties.some((d) => d != null && Number.isFinite(d));
  const shares = list.map((r) => r.shares);
  const stales = list.map((r) => r.stale);
  const invalids = list.map((r) => r.invalid);
  const dups = list.map((r) => Number(r.duplicateShares) || 0);
  const weaks = list.map((r) => Number(r.weakShares) || 0);
  const blocks = list.map((r) => r.blocks);
  const accNodes = list.map((r) => Number(r.blocksAcceptedByNode) || 0);
  const nblues = list.map((r) => Number(r.blocksNotConfirmedBlue) || 0);
  const uptimes = list.map((r) => (r.sessionUptimeSec != null ? r.sessionUptimeSec : 0));
  const hasUptime = list.some((r) => r.sessionUptimeSec != null && r.sessionUptimeSec > 0);

  const hrPos = hashrates.filter((x) => Number.isFinite(x) && x > 0);
  if (hrPos.length) {
    const mn = Math.min(...hrPos);
    const mx = Math.max(...hrPos);
    const mean = hrPos.reduce((a, b) => a + b, 0) / hrPos.length;
    setStat('workersChartHashrateStats', `min ${mn.toFixed(2)} · μ ${mean.toFixed(2)} · max ${mx.toFixed(2)} GH/s`);
  } else {
    setStat('workersChartHashrateStats', '');
  }

  if (hasDiff) {
    const dvals = difficulties.filter((d) => d != null && Number.isFinite(d));
    const dmin = Math.min(...dvals);
    const dmax = Math.max(...dvals);
    setStat('workersChartDifficultyStats', `range ${formatDifficulty(dmin)} … ${formatDifficulty(dmax)}`);
  } else {
    setStat('workersChartDifficultyStats', '');
  }

  const sumDup = dups.reduce((a, b) => a + b, 0);
  const sumWeak = weaks.reduce((a, b) => a + b, 0);
  const sumAcc = accNodes.reduce((a, b) => a + b, 0);
  const sumNblue = nblues.reduce((a, b) => a + b, 0);
  setStat(
    'workersChartSubmissionStats',
    `Σ shares ${sumShares} · stale ${stales.reduce((a, b) => a + b, 0)} · invalid ${invalids.reduce((a, b) => a + b, 0)} · dup ${sumDup} · weak ${sumWeak} · blocks ${sumBlocks} · acc(node) ${sumAcc} · not-blue ${sumNblue} · hover for per-worker values`,
  );

  if (hasUptime) {
    const umax = Math.max(...list.map((r) => r.sessionUptimeSec || 0));
    setStat('workersChartSessionStats', `longest ${formatUptime(umax)}`);
  } else {
    setStat('workersChartSessionStats', 'no session uptime reported');
  }

  applyWorkersChartHeights(list.length);

  if (typeof Chart === 'undefined') return;

  updateOrCreateTrendChart(
    'workerChartHashrate',
    'bar',
    {
      labels,
      datasets: [
        {
          label: 'Hashrate',
          data: hashrates,
          backgroundColor: 'rgba(34, 197, 94, 0.55)',
          borderColor: 'rgba(34, 197, 94, 0.95)',
          borderWidth: 1,
          borderRadius: 4,
          barPercentage: 0.88,
        },
      ],
    },
    workersHorizBarOptions(),
  );

  if (hasDiff) {
    diffCard?.classList.remove('hidden');
    updateOrCreateTrendChart(
      'workerChartDifficulty',
      'bar',
      {
        labels,
        datasets: [
          {
            label: 'Diff',
            data: difficulties.map((d) => (d != null && Number.isFinite(d) ? d : 0)),
            backgroundColor: difficulties.map((d) =>
              d != null && Number.isFinite(d) ? 'rgba(168, 85, 247, 0.55)' : 'rgba(55, 65, 81, 0.25)',
            ),
            borderColor: difficulties.map((d) =>
              d != null && Number.isFinite(d) ? 'rgba(168, 85, 247, 0.9)' : 'rgba(55, 65, 81, 0.4)',
            ),
            borderWidth: 1,
            borderRadius: 4,
            barPercentage: 0.88,
          },
        ],
      },
      workersHorizBarOptions({
        x: {
          ticks: {
            color: '#64748b',
            font: { size: 9 },
            callback(v) {
              return formatDifficulty(v);
            },
          },
        },
      }),
    );
  } else {
    destroyWorkerChartById('workerChartDifficulty');
    diffCard?.classList.add('hidden');
  }

  const submissionOpts = workersHorizBarOptions();
  updateOrCreateTrendChart(
    'workerChartSubmission',
    'bar',
    {
      labels,
      datasets: [
        {
          label: 'Shares',
          data: shares,
          backgroundColor: 'rgba(34, 197, 94, 0.6)',
          borderColor: 'rgba(34, 197, 94, 0.95)',
          borderWidth: 1,
          borderRadius: 3,
        },
        {
          label: 'Stale',
          data: stales,
          backgroundColor: 'rgba(245, 158, 11, 0.55)',
          borderColor: 'rgba(245, 158, 11, 0.95)',
          borderWidth: 1,
          borderRadius: 3,
        },
        {
          label: 'Invalid',
          data: invalids,
          backgroundColor: 'rgba(239, 68, 68, 0.55)',
          borderColor: 'rgba(239, 68, 68, 0.95)',
          borderWidth: 1,
          borderRadius: 3,
        },
        {
          label: 'Dup',
          data: dups,
          backgroundColor: 'rgba(139, 92, 246, 0.52)',
          borderColor: 'rgba(139, 92, 246, 0.92)',
          borderWidth: 1,
          borderRadius: 3,
        },
        {
          label: 'Weak',
          data: weaks,
          backgroundColor: 'rgba(236, 72, 153, 0.48)',
          borderColor: 'rgba(236, 72, 153, 0.9)',
          borderWidth: 1,
          borderRadius: 3,
        },
        {
          label: 'Blocks',
          data: blocks,
          backgroundColor: 'rgba(14, 165, 233, 0.55)',
          borderColor: 'rgba(14, 165, 233, 0.95)',
          borderWidth: 1,
          borderRadius: 3,
        },
        {
          label: 'Acc',
          data: accNodes,
          backgroundColor: 'rgba(6, 182, 212, 0.52)',
          borderColor: 'rgba(6, 182, 212, 0.92)',
          borderWidth: 1,
          borderRadius: 3,
        },
        {
          label: 'N-blue',
          data: nblues,
          backgroundColor: 'rgba(249, 115, 22, 0.48)',
          borderColor: 'rgba(249, 115, 22, 0.9)',
          borderWidth: 1,
          borderRadius: 3,
        },
      ],
    },
    {
      ...submissionOpts,
      datasets: {
        bar: {
          categoryPercentage: 0.62,
          barPercentage: 0.72,
        },
      },
    },
  );

  if (hasUptime) {
    sessCard?.classList.remove('hidden');
    updateOrCreateTrendChart(
      'workerChartSession',
      'bar',
      {
        labels,
        datasets: [
          {
            label: 'Uptime',
            data: uptimes,
            backgroundColor: 'rgba(45, 212, 191, 0.5)',
            borderColor: 'rgba(45, 212, 191, 0.9)',
            borderWidth: 1,
            borderRadius: 4,
            barPercentage: 0.88,
          },
        ],
      },
      workersHorizBarOptions({
        x: {
          ticks: {
            color: '#64748b',
            font: { size: 9 },
            callback(v) {
              return formatUptime(v);
            },
          },
        },
      }),
    );
  } else {
    destroyWorkerChartById('workerChartSession');
    sessCard?.classList.add('hidden');
  }

  requestAnimationFrame(() => resizeTrendCharts());
}

function renderTrendChartsFromPoints(pts) {
  if (typeof Chart === 'undefined') return;
  const emptyEl = document.getElementById('trendsChartsEmpty');
  const gridEl = document.getElementById('trendsChartsGrid');
  if (!emptyEl || !gridEl) return;

  if (pts.length === 0) {
    emptyEl.classList.remove('hidden');
    gridEl.classList.add('opacity-40');
    return;
  }
  emptyEl.classList.add('hidden');
  gridEl.classList.remove('opacity-40');

  const labels = pts.map((p) =>
    new Date(p.t).toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit', second: '2-digit' }),
  );

  const hostCpu = pts.map((p) => (Number.isFinite(p.hostCpu) ? p.hostCpu : null));
  const bridgeCpu = pts.map((p) => (Number.isFinite(p.bridgeCpu) ? p.bridgeCpu : null));
  const kaspadCpu = pts.map((p) => (Number.isFinite(p.kaspadCpu) ? p.kaspadCpu : null));
  const showKaspadCpu = kaspadCpu.some((x) => Number.isFinite(x));
  const elCpu = document.getElementById('trendsStatsCpu');
  if (elCpu) {
    let s = `All cores ${seriesTriplet(hostCpu, fmtTrendCpu)} · Bridge ${seriesTriplet(bridgeCpu, fmtTrendCpu)}`;
    if (showKaspadCpu) s += ` · Kaspad ${seriesTriplet(kaspadCpu, fmtTrendCpu)}`;
    elCpu.textContent = s;
  }

  const cpuDatasets = [
    {
      label: 'Host CPU % (all cores)',
      data: hostCpu,
      borderColor: '#22c55e',
      backgroundColor: 'rgba(34, 197, 94, 0.06)',
      fill: false,
      tension: 0.22,
      spanGaps: true,
      pointRadius: 0,
      borderWidth: 2,
    },
    {
      label: 'Bridge process %',
      data: bridgeCpu,
      borderColor: '#2dd4bf',
      backgroundColor: 'rgba(45, 212, 191, 0.04)',
      fill: false,
      tension: 0.22,
      spanGaps: true,
      pointRadius: 0,
      borderWidth: 1.5,
    },
  ];
  if (showKaspadCpu) {
    cpuDatasets.push({
      label: 'Kaspad process %',
      data: kaspadCpu,
      borderColor: '#c084fc',
      backgroundColor: 'rgba(192, 132, 252, 0.04)',
      fill: false,
      tension: 0.22,
      spanGaps: true,
      pointRadius: 0,
      borderWidth: 1.5,
    });
  }

  updateOrCreateTrendChart('trendChartCpu', 'line', {
    labels,
    datasets: cpuDatasets,
  }, {
    responsive: true,
    maintainAspectRatio: false,
    interaction: { mode: 'index', intersect: false },
    plugins: trendPlugins(),
    scales: { x: trendScaleX(), y: trendScaleY({ suggestedMin: 0, suggestedMax: 100 }) },
  });

  const memGb = pts.map((p) => (Number.isFinite(p.memUsedGb) ? p.memUsedGb : null));
  const swapGb = pts.map((p) => (Number.isFinite(p.swapUsedGb) ? p.swapUsedGb : null));
  const memUsedPct = pts.map((p) => (Number.isFinite(p.memUsedPct) ? p.memUsedPct : null));
  const bridgeRssGb = pts.map((p) => (Number.isFinite(p.bridgeRssGb) ? p.bridgeRssGb : null));
  const kaspadRssGb = pts.map((p) => (Number.isFinite(p.kaspadRssGb) ? p.kaspadRssGb : null));
  const showKaspadRss = kaspadRssGb.some((x) => Number.isFinite(x));
  const embeddedLast = pts.length ? pts[pts.length - 1]?.hostEmbedded === true : false;
  const bridgeRssLabel = embeddedLast ? 'Bridge+kaspad RSS (GB)' : 'Bridge RSS (GB)';
  const elMem = document.getElementById('trendsStatsMem');
  if (elMem) {
    let s = `In use % ${seriesTriplet(memUsedPct, fmtTrendCpu)} · RAM ${seriesTriplet(memGb, fmtTrendMemGb)} · swap ${seriesTriplet(swapGb, fmtTrendMemGb)}`;
    s += ` · ${embeddedLast ? 'Proc RSS' : 'Bridge RSS'} ${seriesTriplet(bridgeRssGb, fmtTrendMemGb)}`;
    if (showKaspadRss) s += ` · kaspad RSS ${seriesTriplet(kaspadRssGb, fmtTrendMemGb)}`;
    elMem.textContent = s;
  }

  const memDatasets = [
    {
      label: 'RAM used (GB)',
      data: memGb,
      borderColor: '#70c7ba',
      backgroundColor: 'rgba(112, 199, 186, 0.12)',
      fill: false,
      tension: 0.22,
      spanGaps: true,
      pointRadius: 0,
      borderWidth: 2,
      yAxisID: 'y',
    },
    {
      label: 'Swap used (GB)',
      data: swapGb,
      borderColor: '#fbbf24',
      backgroundColor: 'rgba(251, 191, 36, 0.06)',
      fill: false,
      tension: 0.22,
      spanGaps: true,
      pointRadius: 0,
      borderWidth: 1.5,
      yAxisID: 'y',
    },
    {
      label: bridgeRssLabel,
      data: bridgeRssGb,
      borderColor: '#38bdf8',
      backgroundColor: 'rgba(56, 189, 248, 0.06)',
      fill: false,
      tension: 0.22,
      spanGaps: true,
      pointRadius: 0,
      borderWidth: 1.5,
      yAxisID: 'y',
    },
    {
      label: 'RAM in use % (same as host card)',
      data: memUsedPct,
      borderColor: '#f472b6',
      backgroundColor: 'rgba(244, 114, 182, 0.05)',
      fill: false,
      tension: 0.22,
      spanGaps: true,
      pointRadius: 0,
      borderWidth: 1.75,
      yAxisID: 'y1',
    },
  ];
  if (showKaspadRss) {
    memDatasets.splice(3, 0, {
      label: 'Kaspad RSS (GB)',
      data: kaspadRssGb,
      borderColor: '#c084fc',
      backgroundColor: 'rgba(192, 132, 252, 0.05)',
      fill: false,
      tension: 0.22,
      spanGaps: true,
      pointRadius: 0,
      borderWidth: 1.5,
      yAxisID: 'y',
    });
  }

  updateOrCreateTrendChart('trendChartMem', 'line', {
    labels,
    datasets: memDatasets,
  }, {
    responsive: true,
    maintainAspectRatio: false,
    interaction: { mode: 'index', intersect: false },
    plugins: trendPlugins(),
    scales: {
      x: trendScaleX(),
      y: {
        type: 'linear',
        display: true,
        position: 'left',
        grid: { color: 'rgba(148, 163, 184, 0.1)' },
        ticks: { color: '#64748b', font: { size: 9 } },
        title: { display: true, text: 'GB', color: '#64748b', font: { size: 10 } },
      },
      y1: {
        type: 'linear',
        display: true,
        position: 'right',
        grid: { drawOnChartArea: false },
        suggestedMin: 0,
        suggestedMax: 100,
        ticks: { color: '#f472b6', font: { size: 9 } },
        title: { display: true, text: 'RAM %', color: '#f472b6', font: { size: 10 } },
      },
    },
  });

  const netRx = pts.map((p) => (Number.isFinite(p.netRxMbps) ? p.netRxMbps : null));
  const netTx = pts.map((p) => (Number.isFinite(p.netTxMbps) ? p.netTxMbps : null));
  const elNet = document.getElementById('trendsStatsNet');
  if (elNet) {
    elNet.textContent = `RX ${seriesTriplet(netRx, fmtTrendMbps)} · TX ${seriesTriplet(netTx, fmtTrendMbps)}`;
  }

  updateOrCreateTrendChart('trendChartNet', 'line', {
    labels,
    datasets: [
      {
        label: 'RX MB/s',
        data: netRx,
        borderColor: '#22c55e',
        backgroundColor: 'rgba(34, 197, 94, 0.12)',
        fill: true,
        tension: 0.22,
        spanGaps: true,
        pointRadius: 0,
        borderWidth: 2,
      },
      {
        label: 'TX MB/s',
        data: netTx,
        borderColor: '#38bdf8',
        backgroundColor: 'rgba(56, 189, 248, 0.06)',
        fill: false,
        tension: 0.22,
        spanGaps: true,
        pointRadius: 0,
        borderWidth: 1.5,
      },
    ],
  }, {
    responsive: true,
    maintainAspectRatio: false,
    interaction: { mode: 'index', intersect: false },
    plugins: trendPlugins(),
    scales: { x: trendScaleX(), y: trendScaleY() },
  });

  const dR = pts.map((p) => (Number.isFinite(p.diskRMBps) ? p.diskRMBps : null));
  const dW = pts.map((p) => (Number.isFinite(p.diskWMBps) ? p.diskWMBps : null));
  const elDisk = document.getElementById('trendsStatsDisk');
  if (elDisk) {
    elDisk.textContent = `Read ${seriesTriplet(dR, fmtTrendMbps)} · write ${seriesTriplet(dW, fmtTrendMbps)}`;
  }

  updateOrCreateTrendChart('trendChartDisk', 'line', {
    labels,
    datasets: [
      {
        label: 'Read MB/s',
        data: dR,
        borderColor: '#a78bfa',
        backgroundColor: 'rgba(167, 139, 250, 0.12)',
        fill: true,
        tension: 0.22,
        spanGaps: true,
        pointRadius: 0,
        borderWidth: 2,
      },
      {
        label: 'Write MB/s',
        data: dW,
        borderColor: '#f472b6',
        backgroundColor: 'rgba(244, 114, 182, 0.06)',
        fill: false,
        tension: 0.22,
        spanGaps: true,
        pointRadius: 0,
        borderWidth: 1.5,
      },
    ],
  }, {
    responsive: true,
    maintainAspectRatio: false,
    interaction: { mode: 'index', intersect: false },
    plugins: trendPlugins(),
    scales: { x: trendScaleX(), y: trendScaleY() },
  });

  const peers = pts.map((p) => (Number.isFinite(p.peers) ? p.peers : null));
  const mempool = pts.map((p) => (Number.isFinite(p.mempool) ? p.mempool : null));
  const elRpc = document.getElementById('trendsStatsRpc');
  if (elRpc) {
    elRpc.textContent = `Peers ${seriesTriplet(peers, fmtTrendInt)} · mempool ${seriesTriplet(mempool, fmtTrendInt)}`;
  }

  updateOrCreateTrendChart('trendChartRpc', 'line', {
    labels,
    datasets: [
      {
        label: 'Peers',
        data: peers,
        borderColor: '#2dd4bf',
        backgroundColor: 'rgba(45, 212, 191, 0.06)',
        fill: false,
        tension: 0.22,
        spanGaps: true,
        pointRadius: 0,
        borderWidth: 2,
        yAxisID: 'y',
      },
      {
        label: 'Mempool txs',
        data: mempool,
        borderColor: '#fb923c',
        backgroundColor: 'rgba(251, 146, 60, 0.05)',
        fill: false,
        tension: 0.22,
        spanGaps: true,
        pointRadius: 0,
        borderWidth: 1.5,
        yAxisID: 'y1',
      },
    ],
  }, {
    responsive: true,
    maintainAspectRatio: false,
    interaction: { mode: 'index', intersect: false },
    plugins: trendPlugins(),
    scales: {
      x: trendScaleX(),
      y: {
        type: 'linear',
        display: true,
        position: 'left',
        grid: { color: 'rgba(148, 163, 184, 0.1)' },
        ticks: { color: '#64748b', font: { size: 9 } },
        title: { display: true, text: 'Peers', color: '#64748b', font: { size: 10 } },
      },
      y1: {
        type: 'linear',
        display: true,
        position: 'right',
        grid: { drawOnChartArea: false },
        ticks: { color: '#94a3b8', font: { size: 9 } },
        title: { display: true, text: 'Mempool', color: '#94a3b8', font: { size: 10 } },
      },
    },
  });

  const syncPct = pts.map((p) => (Number.isFinite(p.syncPct) ? p.syncPct : null));
  const volPct = pts.map((p) => (Number.isFinite(p.volUsedPct) ? p.volUsedPct : null));
  const elSync = document.getElementById('trendsStatsSync');
  if (elSync) {
    elSync.textContent = `DAG/header % ${seriesTriplet(syncPct, fmtTrendCpu)} · vol used % ${seriesTriplet(volPct, fmtTrendCpu)}`;
  }

  updateOrCreateTrendChart('trendChartSync', 'line', {
    labels,
    datasets: [
      {
        label: 'Header/DAG %',
        data: syncPct,
        borderColor: '#22c55e',
        backgroundColor: 'rgba(34, 197, 94, 0.06)',
        fill: false,
        tension: 0.22,
        spanGaps: true,
        pointRadius: 0,
        borderWidth: 2,
      },
      {
        label: 'Data volume used %',
        data: volPct,
        borderColor: '#eab308',
        backgroundColor: 'rgba(234, 179, 8, 0.05)',
        fill: false,
        tension: 0.22,
        spanGaps: true,
        pointRadius: 0,
        borderWidth: 1.5,
      },
    ],
  }, {
    responsive: true,
    maintainAspectRatio: false,
    interaction: { mode: 'index', intersect: false },
    plugins: trendPlugins(),
    scales: { x: trendScaleX(), y: trendScaleY({ suggestedMin: 0, suggestedMax: 100 }) },
  });
}

function refreshTrendChartsUi() {
  const pts = filterTrendsByWindow(readTrendPoints(), getTrendsWindowMs());
  renderTrendChartsFromPoints(pts);
}

function appendTrendSample(status) {
  if (typeof Chart === 'undefined') return;
  const host = status?.host;
  if (!host || typeof host !== 'object') return;

  const node = status?.node;
  const memUsedGb = hostMemUsedGbTrends(host);
  const memUsedPct = hostMemUsedPercentTrends(host);
  const swapUsedGb =
    host.swapUsedBytes != null && Number.isFinite(Number(host.swapUsedBytes))
      ? Number(host.swapUsedBytes) / 1024 ** 3
      : null;
  const bridgeRssGb =
    host.bridgeMemoryBytes != null && Number.isFinite(Number(host.bridgeMemoryBytes))
      ? Number(host.bridgeMemoryBytes) / 1024 ** 3
      : null;
  const embedded = host.embeddedKaspad === true;
  const kaspadCpu =
    !embedded &&
    host.kaspadCpuUsagePercent != null &&
    Number.isFinite(Number(host.kaspadCpuUsagePercent))
      ? Number(host.kaspadCpuUsagePercent)
      : null;
  const kaspadRssGb =
    !embedded &&
    host.kaspadMemoryBytes != null &&
    Number.isFinite(Number(host.kaspadMemoryBytes))
      ? Number(host.kaspadMemoryBytes) / 1024 ** 3
      : null;

  const netRxMbps =
    host.networkRxBytesPerSec != null && Number.isFinite(Number(host.networkRxBytesPerSec))
      ? Number(host.networkRxBytesPerSec) / (1024 * 1024)
      : null;
  const netTxMbps =
    host.networkTxBytesPerSec != null && Number.isFinite(Number(host.networkTxBytesPerSec))
      ? Number(host.networkTxBytesPerSec) / (1024 * 1024)
      : null;

  const diskRMBps =
    host.bridgeDiskReadBytesPerSec != null && Number.isFinite(Number(host.bridgeDiskReadBytesPerSec))
      ? Number(host.bridgeDiskReadBytesPerSec) / (1024 * 1024)
      : null;
  const diskWMBps =
    host.bridgeDiskWriteBytesPerSec != null && Number.isFinite(Number(host.bridgeDiskWriteBytesPerSec))
      ? Number(host.bridgeDiskWriteBytesPerSec) / (1024 * 1024)
      : null;

  const peers = node?.peers != null && Number.isFinite(Number(node.peers)) ? Number(node.peers) : null;
  const mempool = node?.mempoolSize != null && Number.isFinite(Number(node.mempoolSize)) ? Number(node.mempoolSize) : null;
  const syncPct = computeNodeSyncPctForTrends(node);
  const volUsedPct =
    host.nodeVolumeUsedPercent != null && Number.isFinite(Number(host.nodeVolumeUsedPercent))
      ? Number(host.nodeVolumeUsedPercent)
      : null;

  const hostCpu = Number.isFinite(Number(host.globalCpuUsagePercent)) ? Number(host.globalCpuUsagePercent) : null;
  const bridgeCpu =
    host.bridgeCpuUsagePercent != null && Number.isFinite(Number(host.bridgeCpuUsagePercent))
      ? Number(host.bridgeCpuUsagePercent)
      : null;

  const list = readTrendPoints();
  list.push({
    t: Date.now(),
    hostEmbedded: embedded,
    hostCpu,
    bridgeCpu,
    kaspadCpu,
    memUsedGb,
    memUsedPct,
    swapUsedGb,
    bridgeRssGb,
    kaspadRssGb,
    netRxMbps,
    netTxMbps,
    diskRMBps,
    diskWMBps,
    peers,
    mempool,
    syncPct,
    volUsedPct,
  });
  writeTrendPoints(list);
  refreshTrendChartsUi();
}

function initTrendsPanel() {
  if (!document.getElementById('trendChartCpu') && !document.getElementById('trendsLongRangePanel')) {
    return;
  }

  updateTrendsLongRangeQuickUi();

  const urlInput = document.getElementById('trendsLongRangeUrl');
  const storedUrl = readTrendsEmbedUrlStored();
  if (urlInput && storedUrl) urlInput.value = storedUrl;

  document.getElementById('trendsViewSessionBtn')?.addEventListener('click', () => applyTrendsViewMode('session'));
  document.getElementById('trendsViewLongRangeBtn')?.addEventListener('click', () => applyTrendsViewMode('longrange'));

  document.getElementById('trendsLongRangeOpenPromBtn')?.addEventListener('click', () => {
    const host = window.location.hostname || '127.0.0.1';
    window.open(`http://${host}:9090`, '_blank', 'noopener,noreferrer');
  });

  document.getElementById('trendsLongRangeCopyScrapeBtn')?.addEventListener('click', async () => {
    const ok = await copyToClipboard(buildPrometheusScrapeYaml());
    showToast(ok ? 'Scrape job copied (paste into prometheus.yml)' : 'Copy failed');
  });

  document.getElementById('trendsLongRangeCopyTargetBtn')?.addEventListener('click', async () => {
    const ok = await copyToClipboard(getBridgeMetricsPageUrl());
    showToast(ok ? 'Scrape URL copied' : 'Copy failed');
  });

  document.getElementById('trendsLongRangeSaveBtn')?.addEventListener('click', () => {
    const raw = document.getElementById('trendsLongRangeUrl')?.value || '';
    const safe = sanitizeTrendsEmbedUrl(raw);
    if (!safe) {
      showToast('Enter a valid http(s) URL');
      return;
    }
    writeTrendsEmbedUrlStored(safe);
    if (urlInput) urlInput.value = safe;
    setTrendsLongRangeIframeSrc(safe);
    showToast('Long-range URL saved');
  });

  document.getElementById('trendsLongRangeOpenBtn')?.addEventListener('click', () => {
    const raw = document.getElementById('trendsLongRangeUrl')?.value || '';
    const safe = sanitizeTrendsEmbedUrl(raw) || readTrendsEmbedUrlStored();
    if (!safe) {
      showToast('Enter a URL first');
      return;
    }
    window.open(safe, '_blank', 'noopener,noreferrer');
  });

  document.getElementById('trendsLongRangeClearUrlBtn')?.addEventListener('click', () => {
    writeTrendsEmbedUrlStored('');
    if (urlInput) urlInput.value = '';
    setTrendsLongRangeIframeSrc('');
    showToast('URL cleared');
  });

  const sel = document.getElementById('trendsWindowSelect');
  if (sel) sel.addEventListener('change', refreshTrendChartsUi);
  const clr = document.getElementById('trendsClearBtn');
  if (clr) {
    clr.addEventListener('click', () => {
      try {
        sessionStorage.removeItem(TRENDS_STORAGE_KEY);
      } catch {
        /* ignore */
      }
      Object.keys(rkTrends.charts).forEach((k) => {
        try {
          rkTrends.charts[k]?.destroy();
        } catch {
          /* ignore */
        }
        delete rkTrends.charts[k];
      });
      refreshTrendChartsUi();
      showToast('Trend series cleared');
    });
  }
  let resizeT;
  window.addEventListener('resize', () => {
    clearTimeout(resizeT);
    resizeT = setTimeout(() => {
      resizeTrendCharts();
    }, 200);
  });
  applyTrendsViewMode(readTrendsViewMode());
  refreshTrendChartsUi();
}

/**
 * Pull status + stats from the bridge API and repaint the dashboard.
 * @param {{ silent?: boolean }} [options] — If `silent: true`, do not show the nav spinner or "Loading…" (for background polling). Use explicit `silent: false` (default) for first load and Refresh button.
 */
async function refresh(options = {}) {
  const silent = options.silent === true;
  const loader = document.getElementById('status-loader');
  const statusText = document.getElementById('status-text');
  const setDot = (state, title) => {
    if (!statusText) return;
    statusText.className = `status-dot status-dot--${state}`;
    statusText.title = title;
    statusText.textContent = '';
    const lab = document.getElementById('statusLabel');
    if (lab) {
      lab.textContent = title === 'Loading' ? 'Loading…' : title;
      lab.className =
        state === 'online'
          ? 'text-sm font-medium text-emerald-400/95 tabular-nums shrink-0'
          : state === 'offline'
            ? 'text-sm font-medium text-red-400/90 tabular-nums shrink-0'
            : 'text-sm font-medium text-gray-300 tabular-nums shrink-0';
    }
  };

  if (!silent) {
    if (loader) loader.style.display = 'inline-block';
    setDot('loading', 'Loading');
  }

  try {
    const [statusRes, statsRes] = await Promise.all([
      fetch(rkstratumApiUrl('api/status'), { cache: 'no-store' }),
      fetch(rkstratumApiUrl('api/stats'), { cache: 'no-store' }),
    ]);

    if (!statusRes.ok) throw new Error(`status HTTP ${statusRes.status}`);
    if (!statsRes.ok) throw new Error(`stats HTTP ${statsRes.status}`);

    const status = await statusRes.json();
    const stats = await statsRes.json();

    const mergedStats = cacheUpdate(status, stats);

    if (loader) loader.style.display = 'none';
    setDot('online', 'Online');

    setText('kaspadVersion', status.kaspad_version ?? '-');
    setText('instances', status.instances);
    setLastUpdated(Date.now(), false);
    populateNodePanel(status.node);
    populateHostPanel(status.host);
    updateHostMetricsBanner(status);
    updateNodeDifficultyHint(mergedStats, status.node);

    setText('totalBlocks', mergedStats.totalBlocks);
    setMiningBlockSubtotals(mergedStats);
    setText('totalShares', mergedStats.totalShares);
    setText('activeWorkers', mergedStats.activeWorkers);
    
    // Calculate and display total worker hashrate
    const totalWorkerHashrateHs = (mergedStats.workers || []).reduce((sum, w) => sum + ((w.hashrate || 0) * 1e9), 0);
    const totalWorkerHashrateEl = document.getElementById('totalWorkerHashrate');
    if (totalWorkerHashrateEl && totalWorkerHashrateHs > 0) {
      totalWorkerHashrateEl.textContent = `(${formatHashrateHs(totalWorkerHashrateHs)})`;
    } else if (totalWorkerHashrateEl) {
      totalWorkerHashrateEl.textContent = '';
    }
    
    setText('networkHashrate', formatHashrateHs(mergedStats.networkHashrate));
    
    // Display bridge uptime
    if (mergedStats.bridgeUptime != null) {
      setText('bridgeUptime', formatUptime(mergedStats.bridgeUptime));
    } else {
      setText('bridgeUptime', '-');
    }
    setText('networkDifficulty', formatDifficulty(mergedStats.networkDifficulty));
    setText('networkBlockCount', mergedStats.networkBlockCount ?? '-');

    const icpu = mergedStats.internalCpu;
    if (icpu && typeof icpu === 'object') {
      setInternalCpuCardsVisible(true);
      setText('internalCpuHashrate', formatHashrateHs((Number(icpu.hashrateGhs) || 0) * 1e9));
      const accepted = Number(icpu.blocksAccepted) || 0;
      const submitted = Number(icpu.blocksSubmitted) || 0;
      setText('internalCpuBlocks', `${accepted} (${submitted} submitted)`);
    } else {
      setInternalCpuCardsVisible(false);
      setText('internalCpuHashrate', '-');
      setText('internalCpuBlocks', '-');
    }

    const filter = getWalletFilter();
    const dayFilter = getBlocksDayFilter();

    renderWalletSummary(mergedStats, filter);

    let blocks = (mergedStats.blocks || []).filter(b => !filter || (b.wallet || '').includes(filter));
    blocks = filterBlocksByDays(blocks, dayFilter);
    lastFilteredBlocks = blocks;
    const blocksBody = document.getElementById('blocksBody');
    if (blocksBody) {
      blocksBody.innerHTML = '';
      blocks.forEach((b, idx) => {
        const nonceInfo = formatNonceInfo(b.nonce);
        const hashFull = b.hash || '';
        const hashShort = shortHash(hashFull);
        const workerDisplay = displayWorkerName(b.worker);
        const tr = document.createElement('tr');
        tr.className = 'border-b border-card/50 cursor-pointer';
        tr.setAttribute('data-row-kind', 'block');
        tr.setAttribute('data-row-index', String(idx));
        tr.innerHTML = `
        <td class="py-1.5 pr-3" title="${b.timestamp || ''}">${formatUnixSeconds(b.timestamp)}</td>
        <td class="py-1.5 pr-3" title="${escapeHtmlAttr(b.instance || '')}">${b.instance || '-'}</td>
        <td class="py-1.5 pr-3" title="${escapeHtmlAttr(b.bluescore || '')}">${b.bluescore || '-'}</td>
        <td class="py-1.5 pr-3" title="${escapeHtmlAttr(workerDisplay)}">${escapeHtmlAttr(workerDisplay)}</td>
        <td class="py-1.5 pr-3 w-[13rem] max-w-[13rem] align-top">
          <div class="flex items-center gap-2 min-w-0">
            <span class="min-w-0 flex-1 truncate" title="${escapeHtmlAttr(b.wallet || '')}">${escapeHtmlAttr(truncateWalletForDisplay(b.wallet || ''))}</span>
            ${b.wallet ? `<button type="button" class="bg-surface-1 border border-card px-2 py-0.5 rounded text-xs hover:border-kaspa-primary shrink-0" data-copy-text="${escapeHtmlAttr(b.wallet)}">Copy</button>` : ''}
          </div>
        </td>
        <td class="py-1.5 pr-3 font-mono" title="${escapeHtmlAttr(nonceInfo.title)}">${nonceInfo.display || '-'}</td>
        <td class="py-1.5 pr-3">
          <div class="flex items-center gap-2 min-w-0">
            <span class="font-mono min-w-0 truncate" title="${hashFull}">${hashShort}</span>
            ${hashFull ? `<button type="button" class="bg-surface-1 border border-card px-2 py-0.5 rounded text-xs hover:border-kaspa-primary shrink-0" data-copy-text="${escapeHtmlAttr(hashFull)}">Copy</button>` : ''}
          </div>
        </td>
      `;
        blocksBody.appendChild(tr);
      });
    }
    updateRecentBlocksViz(blocks);

    const allWorkers = mergedStats.workers || [];
    const existingOrder = readWorkerOrder();
    const orderedWorkers = maintainWorkerOrder(existingOrder, allWorkers);
    const workers = orderedWorkers.filter(w => !filter || (w.wallet || '').includes(filter));
    lastFilteredWorkers = workers;
    const workersBody = document.getElementById('workersBody');
    if (workersBody) {
      workersBody.innerHTML = '';
    }
    lastInternalCpuWorker = null;

    // Show filter indicator if wallet filter is active
    const workersTable = document.querySelector('[data-workers-table]');
    if (workersTable) {
      const filterIndicator = workersTable.querySelector('.wallet-filter-indicator');
      if (filter) {
        if (!filterIndicator) {
          const indicator = document.createElement('div');
          indicator.className = 'wallet-filter-indicator bg-yellow-900/20 border border-yellow-700/50 rounded-lg px-3 py-2 mb-3 text-sm text-yellow-300';
          indicator.innerHTML = `
            <div class="flex items-center justify-between gap-3">
              <span>⚠️ Showing ${workers.length} of ${allWorkers.length} workers (filtered by wallet)</span>
              <button type="button" class="text-yellow-300 hover:text-yellow-200 underline text-xs" onclick="document.getElementById('walletClearBtn')?.click()">Clear filter</button>
            </div>
          `;
          workersTable.insertBefore(indicator, workersTable.firstChild);
        } else {
          filterIndicator.querySelector('span').textContent = `⚠️ Showing ${workers.length} of ${allWorkers.length} workers (filtered by wallet)`;
        }
      } else if (filterIndicator) {
        filterIndicator.remove();
      }
    }

    // Render internal CPU miner row as a pseudo-worker (not affected by wallet filter).
    if (workersBody && !filter && icpu && typeof icpu === 'object') {
      const tr = document.createElement('tr');
      tr.className = 'border-b border-card/50 cursor-pointer';
      const hashrateHs = (Number(icpu.hashrateGhs) || 0) * 1e9;
      const wallet = String(icpu.wallet ?? '').trim();
      const shares = Number(icpu.shares ?? icpu.blocksAccepted) || 0;
      const stale = Number(icpu.stale ?? ((Number(icpu.blocksSubmitted) || 0) - (Number(icpu.blocksAccepted) || 0))) || 0;
      const invalid = Number(icpu.invalid ?? 0) || 0;
      lastInternalCpuWorker = { wallet, hashrateHs, shares, stale, invalid, blocks: Number(icpu.blocksAccepted) || 0 };
      tr.setAttribute('data-row-kind', 'icpu');
      tr.setAttribute('data-row-index', '-1');
      tr.innerHTML = internalCpuWorkerRowHtml(icpu);
      workersBody.appendChild(tr);
    }

    if (workersBody) {
      workers.forEach((w, idx) => {
        const tr = document.createElement('tr');
        tr.className = 'border-b border-card/50 cursor-pointer';
        tr.setAttribute('data-row-kind', 'worker');
        tr.setAttribute('data-row-index', String(idx));
        tr.innerHTML = buildWorkerDataRowHtml(w);
        workersBody.appendChild(tr);
      });
    }

    updateWorkersViz(buildWorkerVizRows(workers, !filter && icpu && typeof icpu === 'object' ? icpu : null));

    appendTrendSample(status);

    // raw view is on /raw.html
  } catch (e) {
    if (loader) loader.style.display = 'none';
    setDot('offline', 'Offline');
    const ne = document.getElementById('nodePanelError');
    const cached = readCachedSnapshot();
    if (ne) {
      if (cached) {
        ne.classList.add('hidden');
        ne.textContent = '';
      } else {
        ne.classList.remove('hidden');
        ne.textContent = `API refresh failed: ${e && e.message ? e.message : String(e)}`;
      }
    }
    if (!cached) {
      populateNodePanel(null);
      populateHostPanel(null);
      updateHostMetricsBanner(null);
      updateNodeDifficultyHint(null, null);
    }
    if (cached) {
      setText('kaspadVersion', cached.status.kaspad_version ?? '-');
      setText('instances', cached.status.instances ?? '-');
      setLastUpdated(cached.updatedMs, true);
      populateNodePanel(cached.status.node);
      populateHostPanel(cached.status.host);
      updateHostMetricsBanner(cached.status);
      updateNodeDifficultyHint(cached.stats, cached.status.node);
      
      // Display bridge uptime from cached stats
      if (cached.stats.bridgeUptime != null) {
        setText('bridgeUptime', formatUptime(cached.stats.bridgeUptime));
      } else {
        setText('bridgeUptime', '-');
      }

      const tbEl = document.getElementById('totalBlocks');
      if (tbEl) tbEl.textContent = displayTotalBlocksFromStats(cached.stats);
      setMiningBlockSubtotals(cached.stats);
      setText('totalShares', cached.stats.totalShares);
      setText('activeWorkers', cached.stats.activeWorkers);
      setText('networkHashrate', formatHashrateHs(cached.stats.networkHashrate));
      setText('networkDifficulty', formatDifficulty(cached.stats.networkDifficulty));
      setText('networkBlockCount', cached.stats.networkBlockCount ?? '-');

      const icpu = cached.stats.internalCpu;
      if (icpu && typeof icpu === 'object') {
        setInternalCpuCardsVisible(true);
        setText('internalCpuHashrate', formatHashrateHs((Number(icpu.hashrateGhs) || 0) * 1e9));
        const accepted = Number(icpu.blocksAccepted) || 0;
        const submitted = Number(icpu.blocksSubmitted) || 0;
        setText('internalCpuBlocks', `${accepted} (${submitted} submitted)`);
      } else {
        setInternalCpuCardsVisible(false);
        setText('internalCpuHashrate', '-');
        setText('internalCpuBlocks', '-');
      }

      const filter = getWalletFilter();
      const dayFilter = getBlocksDayFilter();

      renderWalletSummary(cached.stats, filter);

      let blocks = (cached.stats.blocks || []).filter(b => !filter || (b.wallet || '').includes(filter));
      blocks = filterBlocksByDays(blocks, dayFilter);
      lastFilteredBlocks = blocks;
      const blocksBody = document.getElementById('blocksBody');
      if (blocksBody) {
        blocksBody.innerHTML = '';
        blocks.forEach((b, idx) => {
          const nonceInfo = formatNonceInfo(b.nonce);
          const hashFull = b.hash || '';
          const hashShort = shortHash(hashFull);
          const workerDisplay = displayWorkerName(b.worker);
          const tr = document.createElement('tr');
          tr.className = 'border-b border-card/50 cursor-pointer';
          tr.setAttribute('data-row-kind', 'block');
          tr.setAttribute('data-row-index', String(idx));
          tr.innerHTML = `
          <td class="py-1.5 pr-3" title="${b.timestamp || ''}">${formatUnixSeconds(b.timestamp)}</td>
          <td class="py-1.5 pr-3" title="${escapeHtmlAttr(b.instance || '')}">${b.instance || '-'}</td>
          <td class="py-1.5 pr-3" title="${escapeHtmlAttr(b.bluescore || '')}">${b.bluescore || '-'}</td>
        <td class="py-1.5 pr-3" title="${escapeHtmlAttr(workerDisplay)}">${escapeHtmlAttr(workerDisplay)}</td>
        <td class="py-1.5 pr-3">
          <div class="flex items-center gap-2 min-w-0">
              <span class="min-w-0 truncate" title="${escapeHtmlAttr(b.wallet || '')}">${escapeHtmlAttr(b.wallet || '-')}</span>
              ${b.wallet ? `<button type="button" class="bg-surface-1 border border-card px-2 py-0.5 rounded text-xs hover:border-kaspa-primary shrink-0" data-copy-text="${escapeHtmlAttr(b.wallet)}">Copy</button>` : ''}
          </div>
        </td>
          <td class="py-1.5 pr-3 font-mono" title="${escapeHtmlAttr(nonceInfo.title)}">${nonceInfo.display || '-'}</td>
          <td class="py-1.5 pr-3">
            <div class="flex items-center gap-2 min-w-0">
              <span class="font-mono min-w-0 truncate" title="${hashFull}">${hashShort}</span>
              ${hashFull ? `<button type="button" class="bg-surface-1 border border-card px-2 py-0.5 rounded text-xs hover:border-kaspa-primary shrink-0" data-copy-text="${escapeHtmlAttr(hashFull)}">Copy</button>` : ''}
            </div>
          </td>
        `;
          blocksBody.appendChild(tr);
        });
      }
      updateRecentBlocksViz(blocks);

      const allWorkers = cached.stats.workers || [];
      const existingOrder = readWorkerOrder();
      const orderedWorkers = maintainWorkerOrder(existingOrder, allWorkers);
      const workers = orderedWorkers.filter(w => !filter || (w.wallet || '').includes(filter));
      lastFilteredWorkers = workers;
      const workersBody = document.getElementById('workersBody');
      if (workersBody) {
        workersBody.innerHTML = '';
      }
      lastInternalCpuWorker = null;

      // Show filter indicator if wallet filter is active
      const workersTable = document.querySelector('[data-workers-table]');
      if (workersTable) {
        const filterIndicator = workersTable.querySelector('.wallet-filter-indicator');
        if (filter) {
          if (!filterIndicator) {
            const indicator = document.createElement('div');
            indicator.className = 'wallet-filter-indicator bg-yellow-900/20 border border-yellow-700/50 rounded-lg px-3 py-2 mb-3 text-sm text-yellow-300';
            indicator.innerHTML = `
              <div class="flex items-center justify-between gap-3">
                <span>⚠️ Showing ${workers.length} of ${allWorkers.length} workers (filtered by wallet)</span>
                <button type="button" class="text-yellow-300 hover:text-yellow-200 underline text-xs" onclick="document.getElementById('walletClearBtn')?.click()">Clear filter</button>
              </div>
            `;
            workersTable.insertBefore(indicator, workersTable.firstChild);
          } else {
            filterIndicator.querySelector('span').textContent = `⚠️ Showing ${workers.length} of ${allWorkers.length} workers (filtered by wallet)`;
          }
        } else if (filterIndicator) {
          filterIndicator.remove();
        }
      }

      // Render internal CPU miner row as a pseudo-worker (not affected by wallet filter).
      if (workersBody && !filter && icpu && typeof icpu === 'object') {
        const tr = document.createElement('tr');
        tr.className = 'border-b border-card/50 cursor-pointer';
        const hashrateHs = (Number(icpu.hashrateGhs) || 0) * 1e9;
        const wallet = String(icpu.wallet ?? '').trim();
        const shares = Number(icpu.shares ?? icpu.blocksAccepted) || 0;
        const stale = Number(icpu.stale ?? ((Number(icpu.blocksSubmitted) || 0) - (Number(icpu.blocksAccepted) || 0))) || 0;
        const invalid = Number(icpu.invalid ?? 0) || 0;
        lastInternalCpuWorker = { wallet, hashrateHs, shares, stale, invalid, blocks: Number(icpu.blocksAccepted) || 0 };
        tr.setAttribute('data-row-kind', 'icpu');
        tr.setAttribute('data-row-index', '-1');
        tr.innerHTML = internalCpuWorkerRowHtml(icpu);
        workersBody.appendChild(tr);
      }

      if (workersBody) {
        workers.forEach((w, idx) => {
          const tr = document.createElement('tr');
          tr.className = 'border-b border-card/50 cursor-pointer';
          tr.setAttribute('data-row-kind', 'worker');
          tr.setAttribute('data-row-index', String(idx));
          tr.innerHTML = buildWorkerDataRowHtml(w);
          workersBody.appendChild(tr);
        });
      }

      updateWorkersViz(buildWorkerVizRows(workers, !filter && icpu && typeof icpu === 'object' ? icpu : null));

      return;
    }

    setLastUpdated(0, false);
  } finally {
    if (loader) loader.style.display = 'none';
  }
}

document.addEventListener('click', async (e) => {
  const collapseBtn = e.target.closest('[data-collapsible-toggle]');
  if (collapseBtn) {
    const id = collapseBtn.getAttribute('data-collapsible-toggle');
    if (!id) return;
    const isRaw = id === 'raw';
    const collapsedNow = collapseBtn.getAttribute('aria-expanded') === 'false';
    const collapsedNext = !collapsedNow;
    setSectionCollapsed(id, collapsedNext);
    if (
      (id === 'trendsCharts' ||
        id === 'recentBlocks' ||
        id === 'workers' ||
        id === 'recentBlocksViz' ||
        id === 'workersViz' ||
        id === 'trendsSessionChartsBody' ||
        id === 'trendsLongRangeBody') &&
      !collapsedNext
    ) {
      requestAnimationFrame(() => resizeTrendCharts());
    }
    if (!isRaw) {
      const saved = readCollapsedSections();
      saved[id] = collapsedNext;
      writeCollapsedSections(saved);
    }
    return;
  }

  const btn = e.target.closest('[data-copy-id],[data-copy-text]');
  if (btn) {
    let value = '';
    if (btn.dataset.copyText != null) {
      value = btn.dataset.copyText;
    } else if (btn.dataset.copyId) {
      const el = document.getElementById(btn.dataset.copyId);
      value = el ? (el.textContent || '') : '';
    }

    const ok = await copyToClipboard(value);
    showToast(ok ? 'Copied' : 'Copy failed');
    return;
  }

  // Tap-to-expand rows on mobile / coarse pointer devices.
  if (!isCoarsePointerDevice()) return;
  const row = e.target.closest('tr[data-row-kind]');
  if (!row) return;

  const kind = row.getAttribute('data-row-kind');
  const idx = Number(row.getAttribute('data-row-index') || -1);

  if (kind === 'block') {
    const b = lastFilteredBlocks[idx];
    if (!b) return;
    const nonceInfo = formatNonceInfo(b.nonce);
    const workerDisplay = displayWorkerName(b.worker);
    openRowDetailModal('Recent Block', [
      { label: 'Timestamp', value: formatUnixSeconds(b.timestamp), copyValue: b.timestamp },
      { label: 'Instance', value: b.instance || '-', copyValue: b.instance || '' },
      { label: 'Bluescore', value: b.bluescore || '-', copyValue: b.bluescore || '' },
      { label: 'Worker', value: workerDisplay, copyValue: workerDisplay },
      { label: 'Wallet', value: b.wallet || '-', copyValue: b.wallet || '' },
      { label: 'Nonce', value: nonceInfo.title || nonceInfo.display || '-', copyValue: b.nonce || '' },
      { label: 'Hash', value: b.hash || '-', copyValue: b.hash || '' },
    ]);
    return;
  }

  if (kind === 'worker') {
    const w = lastFilteredWorkers[idx];
    if (!w) return;
    const workerDisplay = displayWorkerName(w.worker);
    const bal = workerBalanceKasText(w);
    openRowDetailModal('Worker', [
      { label: 'Instance', value: w.instance || '-', copyValue: w.instance || '' },
      { label: 'Worker', value: workerDisplay, copyValue: workerDisplay },
      { label: 'Wallet', value: w.wallet || '-', copyValue: w.wallet || '' },
      { label: 'Hashrate', value: formatHashrateHs((w.hashrate || 0) * 1e9), copyValue: String((w.hashrate || 0) * 1e9) },
      { label: 'Current Difficulty', value: w.currentDifficulty != null ? formatDifficulty(w.currentDifficulty) : '-', copyValue: w.currentDifficulty != null ? String(w.currentDifficulty) : '' },
      { label: 'Session Uptime', value: w.sessionUptime != null ? formatUptime(w.sessionUptime) : '-', copyValue: w.sessionUptime != null ? String(w.sessionUptime) : '' },
      { label: 'Shares', value: w.shares ?? '-', copyValue: w.shares ?? '' },
      { label: 'Stale', value: w.stale ?? '-', copyValue: w.stale ?? '' },
      { label: 'Invalid', value: w.invalid ?? '-', copyValue: w.invalid ?? '' },
      { label: 'Duplicate shares', value: String(workerStatNum(w, 'duplicateShares', 'duplicate_shares')), copyValue: String(workerStatNum(w, 'duplicateShares', 'duplicate_shares')) },
      { label: 'Weak shares', value: String(workerStatNum(w, 'weakShares', 'weak_shares')), copyValue: String(workerStatNum(w, 'weakShares', 'weak_shares')) },
      { label: 'Blocks', value: w.blocks ?? '-', copyValue: w.blocks ?? '' },
      {
        label: 'Blocks accepted (node)',
        value: String(workerStatNum(w, 'blocksAcceptedByNode', 'blocks_accepted_by_node')),
        copyValue: String(workerStatNum(w, 'blocksAcceptedByNode', 'blocks_accepted_by_node')),
      },
      {
        label: 'Blocks not confirmed blue',
        value: String(workerStatNum(w, 'blocksNotConfirmedBlue', 'blocks_not_confirmed_blue')),
        copyValue: String(workerStatNum(w, 'blocksNotConfirmedBlue', 'blocks_not_confirmed_blue')),
      },
      { label: 'Disconnects', value: String(workerStatNum(w, 'disconnects', 'disconnects')), copyValue: String(workerStatNum(w, 'disconnects', 'disconnects')) },
      { label: 'Jobs', value: String(workerStatNum(w, 'jobs', 'jobs')), copyValue: String(workerStatNum(w, 'jobs', 'jobs')) },
      { label: 'Balance (KAS)', value: bal, copyValue: bal !== '-' ? bal : '' },
      { label: 'Errors', value: String(workerStatNum(w, 'errors', 'errors')), copyValue: String(workerStatNum(w, 'errors', 'errors')) },
    ]);
    return;
  }

  if (kind === 'icpu') {
    const icpu = lastInternalCpuWorker;
    if (!icpu) return;
    openRowDetailModal('RKStratum CPU Miner', [
      { label: 'Worker', value: displayWorkerName('InternalCPU'), copyValue: displayWorkerName('InternalCPU') },
      { label: 'Wallet', value: icpu.wallet || '-', copyValue: icpu.wallet || '' },
      { label: 'Hashrate', value: formatHashrateHs(icpu.hashrateHs || 0), copyValue: String(icpu.hashrateHs || 0) },
      { label: 'Shares', value: icpu.shares ?? '-', copyValue: icpu.shares ?? '' },
      { label: 'Stale', value: icpu.stale ?? '-', copyValue: icpu.stale ?? '' },
      { label: 'Invalid', value: icpu.invalid ?? '-', copyValue: icpu.invalid ?? '' },
      { label: 'Blocks', value: icpu.blocks ?? '-', copyValue: icpu.blocks ?? '' },
    ]);
  }
});

document.getElementById('downloadWorkersCsv')?.addEventListener('click', () => {
  const rows = [
    [
      'instance',
      'worker',
      'wallet',
      'hashrate_ghs',
      'current_difficulty',
      'session_uptime_secs',
      'shares',
      'stale',
      'invalid',
      'duplicate_shares',
      'weak_shares',
      'blocks',
      'blocks_accepted_by_node',
      'blocks_not_confirmed_blue',
      'disconnects',
      'jobs',
      'balance_kas',
      'errors',
    ],
    ...lastFilteredWorkers.map((w) => [
      w.instance ?? '',
      w.worker ?? '',
      w.wallet ?? '',
      (Number(w.hashrate) || 0).toFixed(6),
      w.currentDifficulty != null ? String(w.currentDifficulty) : '',
      w.sessionUptime != null ? String(w.sessionUptime) : '',
      w.shares ?? '',
      w.stale ?? '',
      w.invalid ?? '',
      workerStatNum(w, 'duplicateShares', 'duplicate_shares'),
      workerStatNum(w, 'weakShares', 'weak_shares'),
      w.blocks ?? '',
      workerStatNum(w, 'blocksAcceptedByNode', 'blocks_accepted_by_node'),
      workerStatNum(w, 'blocksNotConfirmedBlue', 'blocks_not_confirmed_blue'),
      workerStatNum(w, 'disconnects', 'disconnects'),
      workerStatNum(w, 'jobs', 'jobs'),
      (() => {
        const t = workerBalanceKasText(w);
        return t === '-' ? '' : t;
      })(),
      workerStatNum(w, 'errors', 'errors'),
    ]),
  ];
  const ts = new Date().toISOString().replace(/[:.]/g, '-');
  downloadCsv(`workers-${ts}.csv`, rows);
});

document.getElementById('downloadBlocksCsv')?.addEventListener('click', () => {
  const rows = [
    ['timestamp_unix','timestamp_local','instance','bluescore','worker','wallet','nonce','hash'],
    ...lastFilteredBlocks.map(b => [
      b.timestamp ?? '',
      formatUnixSeconds(b.timestamp),
      b.instance ?? '',
      b.bluescore ?? '',
      b.worker ?? '',
      b.wallet ?? '',
      b.nonce ?? '',
      b.hash ?? '',
    ]),
  ];
  const ts = new Date().toISOString().replace(/[:.]/g, '-');
  downloadCsv(`blocks-${ts}.csv`, rows);
});

document.getElementById('refreshBtn')?.addEventListener('click', refresh);
(function initWalletSearch() {
  const input = document.getElementById('walletSearchInput');
  const searchBtn = document.getElementById('walletSearchBtn');
  const clearBtn = document.getElementById('walletClearBtn');
  const persisted = getWalletFilterFromStorage();

  if (input) input.value = persisted;
  setWalletFilter(persisted);

  const doSearch = () => {
    const v = normalizeWalletFilter(input?.value);
    setWalletFilter(v);
    void refresh({ silent: true });
  };

  const doClear = () => {
    if (input) input.value = '';
    setWalletFilter('');
    void refresh({ silent: true });
  };

  if (searchBtn) searchBtn.addEventListener('click', doSearch);
  if (clearBtn) clearBtn.addEventListener('click', doClear);
  if (input) {
    input.addEventListener('keydown', (e) => {
      if (e.key === 'Enter') doSearch();
    });
  }
})();

(function initBlocksDayFilter() {
  const select = document.getElementById('blocksDayFilter');
  if (!select) return;
  
  const persisted = getBlocksDayFilterFromStorage();
  setBlocksDayFilter(persisted);
  
  select.addEventListener('change', () => {
    const value = getBlocksDayFilter();
    setBlocksDayFilter(value);
    void refresh({ silent: true });
  });
})();

(function initHostGeoApproxToggle() {
  const geoRow = document.getElementById('hostGeoRow');
  const showBtn = document.getElementById('hostGeoShowBtn');
  if (!geoRow) return;

  const persistHidden = () => {
    try {
      localStorage.setItem(GEO_APPROX_HIDDEN_KEY, '1');
    } catch {
      /* ignore quota / private mode */
    }
  };

  const persistShown = () => {
    try {
      localStorage.removeItem(GEO_APPROX_HIDDEN_KEY);
    } catch {
      /* ignore */
    }
  };

  geoRow.addEventListener('click', () => {
    if (geoRow.classList.contains('hidden') || !__hostGeoApproxString) return;
    persistHidden();
    applyHostGeoApproxRow(__hostGeoApproxString);
  });

  geoRow.addEventListener('keydown', (e) => {
    if (geoRow.classList.contains('hidden') || !__hostGeoApproxString) return;
    if (e.key !== 'Enter' && e.key !== ' ') return;
    e.preventDefault();
    persistHidden();
    applyHostGeoApproxRow(__hostGeoApproxString);
  });

  showBtn?.addEventListener('click', () => {
    if (!__hostGeoApproxString) return;
    persistShown();
    applyHostGeoApproxRow(__hostGeoApproxString);
  });
})();

document.getElementById('clearCacheBtn')?.addEventListener('click', () => {
  cacheClear();
  setLastUpdated(0, false);
  showToast('Cache cleared');
  void refresh();
});

(function initRowDetailModalControls() {
  const closeBtn = document.getElementById('rowDetailClose');
  const backdrop = document.getElementById('rowDetailBackdrop');
  if (closeBtn) closeBtn.addEventListener('click', closeRowDetailModal);
  if (backdrop) backdrop.addEventListener('click', closeRowDetailModal);
  document.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') closeRowDetailModal();
  });
})();
/** When opened in a normal browser, show local time in the nav; Tauri embeds pass embeddedChrome=1 and use the shell clock instead. */
(function initNavClockForStandalone() {
  try {
    const sp = new URLSearchParams(window.location.search);
    if (sp.get('embeddedChrome') === '1') return;
    const slot = document.getElementById('rkNavClockSlot');
    if (!slot) return;
    slot.innerHTML = `
      <div class="rk-nav-clock px-3 py-2 sm:px-4 sm:py-2.5 rounded-xl border border-[rgba(112,199,186,0.2)] bg-gradient-to-br from-slate-900/80 to-slate-950/90 shadow-inner">
        <div class="text-[10px] sm:text-[11px] font-medium uppercase tracking-wider text-[rgba(112,199,186,0.75)]">Local time</div>
        <div id="serverTime" class="text-white font-semibold tabular-nums text-[11px] sm:text-sm leading-snug mt-0.5">-</div>
      </div>`;
  } catch {
    /* ignore */
  }
})();

// Update server time every second
setInterval(() => {
  updateServerTime();
}, 1000);

// Initial server time update
updateServerTime();

// Update server time format on window resize (for responsive display)
let resizeTimeout;
window.addEventListener('resize', () => {
  clearTimeout(resizeTimeout);
  resizeTimeout = setTimeout(() => {
    updateServerTime();
  }, 100);
});

setInterval(() => {
  // avoid overlapping refresh calls if the network is slow
  if (document.hidden) return;
  void refresh({ silent: true });
}, 2000);
// Restore cached data immediately, then refresh live
(function bootstrapFromCache() {
  const cached = readCachedSnapshot();
  if (!cached) return;
  setText('kaspadVersion', cached.status.kaspad_version ?? '-');
  setText('instances', cached.status.instances ?? '-');
  setLastUpdated(cached.updatedMs, true);
  populateNodePanel(cached.status.node);
  populateHostPanel(cached.status.host);
  updateHostMetricsBanner(cached.status);
  updateNodeDifficultyHint(cached.stats, cached.status.node);

  // Display bridge uptime from cached stats
  if (cached.stats.bridgeUptime != null) {
    setText('bridgeUptime', formatUptime(cached.stats.bridgeUptime));
  } else {
    setText('bridgeUptime', '-');
  }

  {
    const tb = document.getElementById('totalBlocks');
    if (tb) tb.textContent = displayTotalBlocksFromStats(cached.stats);
  }
  setMiningBlockSubtotals(cached.stats);
  setText('totalShares', cached.stats.totalShares);
  setText('activeWorkers', cached.stats.activeWorkers);
  setText('networkHashrate', formatHashrateHs(cached.stats.networkHashrate));
  setText('networkDifficulty', formatDifficulty(cached.stats.networkDifficulty));
  setText('networkBlockCount', cached.stats.networkBlockCount ?? '-');

  const filter = getWalletFilter();
  const dayFilter = getBlocksDayFilter();

  renderWalletSummary(cached.stats, filter);

  let blocks = (cached.stats.blocks || []).filter(b => !filter || (b.wallet || '').includes(filter));
  blocks = filterBlocksByDays(blocks, dayFilter);
  lastFilteredBlocks = blocks;
  const blocksBody = document.getElementById('blocksBody');
  if (blocksBody) {
    blocksBody.innerHTML = '';
    blocks.forEach((b, idx) => {
      const nonceInfo = formatNonceInfo(b.nonce);
      const hashFull = b.hash || '';
      const hashShort = shortHash(hashFull);
      const workerDisplay = displayWorkerName(b.worker);
      const tr = document.createElement('tr');
      tr.className = 'border-b border-card/50 cursor-pointer';
      tr.setAttribute('data-row-kind', 'block');
      tr.setAttribute('data-row-index', String(idx));
      tr.innerHTML = `
      <td class="py-1.5 pr-3" title="${b.timestamp || ''}">${formatUnixSeconds(b.timestamp)}</td>
      <td class="py-1.5 pr-3" title="${escapeHtmlAttr(b.instance || '')}">${b.instance || '-'}</td>
      <td class="py-1.5 pr-3" title="${escapeHtmlAttr(b.bluescore || '')}">${b.bluescore || '-'}</td>
      <td class="py-1.5 pr-3" title="${escapeHtmlAttr(workerDisplay)}">${escapeHtmlAttr(workerDisplay)}</td>
      <td class="py-1.5 pr-3">
        <div class="flex items-center gap-2 min-w-0">
          <span class="min-w-0 truncate" title="${escapeHtmlAttr(b.wallet || '')}">${escapeHtmlAttr(b.wallet || '-')}</span>
          ${b.wallet ? `<button type="button" class="bg-surface-1 border border-card px-2 py-0.5 rounded text-xs hover:border-kaspa-primary shrink-0" data-copy-text="${escapeHtmlAttr(b.wallet)}">Copy</button>` : ''}
        </div>
      </td>
      <td class="py-1.5 pr-3 font-mono" title="${escapeHtmlAttr(nonceInfo.title)}">${nonceInfo.display || '-'}</td>
      <td class="py-1.5 pr-3">
        <div class="flex items-center gap-2 min-w-0">
          <span class="font-mono min-w-0 truncate" title="${hashFull}">${hashShort}</span>
          ${hashFull ? `<button type="button" class="bg-surface-1 border border-card px-2 py-0.5 rounded text-xs hover:border-kaspa-primary shrink-0" data-copy-text="${escapeHtmlAttr(hashFull)}">Copy</button>` : ''}
        </div>
      </td>
    `;
      blocksBody.appendChild(tr);
    });
  }
  updateRecentBlocksViz(blocks);

  const allWorkers = cached.stats.workers || [];
  const existingOrder = readWorkerOrder();
  const orderedWorkers = maintainWorkerOrder(existingOrder, allWorkers);
  const workers = orderedWorkers.filter(w => !filter || (w.wallet || '').includes(filter));
  lastFilteredWorkers = workers;
  const workersBody = document.getElementById('workersBody');
  if (workersBody) {
    workersBody.innerHTML = '';
  }
  lastInternalCpuWorker = null;

  // Show filter indicator if wallet filter is active
  const workersTable = document.querySelector('[data-workers-table]');
  if (workersTable) {
    const filterIndicator = workersTable.querySelector('.wallet-filter-indicator');
    if (filter) {
      if (!filterIndicator) {
        const indicator = document.createElement('div');
        indicator.className = 'wallet-filter-indicator bg-yellow-900/20 border border-yellow-700/50 rounded-lg px-3 py-2 mb-3 text-sm text-yellow-300';
        indicator.innerHTML = `
          <div class="flex items-center justify-between gap-3">
            <span>⚠️ Showing ${workers.length} of ${allWorkers.length} workers (filtered by wallet)</span>
            <button type="button" class="text-yellow-300 hover:text-yellow-200 underline text-xs" onclick="document.getElementById('walletClearBtn')?.click()">Clear filter</button>
          </div>
        `;
        workersTable.insertBefore(indicator, workersTable.firstChild);
      } else {
        filterIndicator.querySelector('span').textContent = `⚠️ Showing ${workers.length} of ${allWorkers.length} workers (filtered by wallet)`;
      }
    } else if (filterIndicator) {
      filterIndicator.remove();
    }
  }

  const icpuBoot = cached.stats.internalCpu;
  if (workersBody && !filter && icpuBoot && typeof icpuBoot === 'object') {
    const tr = document.createElement('tr');
    tr.className = 'border-b border-card/50 cursor-pointer';
    const hashrateHs = (Number(icpuBoot.hashrateGhs) || 0) * 1e9;
    const wallet = String(icpuBoot.wallet ?? '').trim();
    const shares = Number(icpuBoot.shares ?? icpuBoot.blocksAccepted) || 0;
    const stale =
      Number(icpuBoot.stale ?? ((Number(icpuBoot.blocksSubmitted) || 0) - (Number(icpuBoot.blocksAccepted) || 0))) || 0;
    const invalid = Number(icpuBoot.invalid ?? 0) || 0;
    lastInternalCpuWorker = {
      wallet,
      hashrateHs,
      shares,
      stale,
      invalid,
      blocks: Number(icpuBoot.blocksAccepted) || 0,
    };
    tr.setAttribute('data-row-kind', 'icpu');
    tr.setAttribute('data-row-index', '-1');
    tr.innerHTML = internalCpuWorkerRowHtml(icpuBoot);
    workersBody.appendChild(tr);
  }

  if (workersBody) {
    workers.forEach((w, idx) => {
      const tr = document.createElement('tr');
      tr.className = 'border-b border-card/50 cursor-pointer';
      tr.setAttribute('data-row-kind', 'worker');
      tr.setAttribute('data-row-index', String(idx));
      tr.innerHTML = buildWorkerDataRowHtml(w);
      workersBody.appendChild(tr);
    });
  }

  updateWorkersViz(
    buildWorkerVizRows(workers, !filter && icpuBoot && typeof icpuBoot === 'object' ? icpuBoot : null),
  );
})();
initCollapsibles();
initTrendsPanel();
refresh();
