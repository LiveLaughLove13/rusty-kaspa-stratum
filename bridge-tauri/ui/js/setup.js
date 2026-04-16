    const LS_KEY = 'rkstratumBridgeDesktopSetupV2';
    const LS_KEY_LEGACY = 'rkstratumBridgeDesktopSetupV1';
    const LS_FIRST_RUN_DISMISSED = 'rkstratumBridgeDesktopFirstRunDismissed';
    const invoke = (cmd, args) => window.__TAURI__?.invoke?.(cmd, args);

    let chromeWired = false;
    let lastDashboardUrl;
    let logPollTimer = null;
    let logCursor = 0;
    let logPath = '';

    function tickShellLocalTime() {
      const el = document.getElementById('shellLocalTime');
      if (!el) return;
      try {
        el.textContent = new Date().toLocaleString('en-US', {
          weekday: 'short',
          year: 'numeric',
          month: 'short',
          day: 'numeric',
          hour: 'numeric',
          minute: '2-digit',
          second: '2-digit',
          hour12: true,
        });
      } catch {
        el.textContent = new Date().toLocaleString();
      }
    }

    /** Header actions shared by setup and dashboard (incl. Stop bridge — requires staying on the Tauri page). */
    function wireChrome() {
      if (chromeWired) return;
      chromeWired = true;
      document.getElementById('btnLogs').addEventListener('click', () => {
        const drawer = document.getElementById('logDrawer');
        const isOpen = drawer.classList.toggle('open');
        if (isOpen) {
          startLogPolling(true);
        } else {
          stopLogPolling();
        }
      });
      document.getElementById('btnRevealExe').addEventListener('click', () => {
        invoke('reveal_exe_directory').catch((e) => alert(String(e)));
      });
      document.getElementById('btnStop').addEventListener('click', async () => {
        const b = document.getElementById('btnStop');
        b.disabled = true;
        try {
          await invoke('stop_bridge');
        } catch (e) {
          alert(String(e));
          b.disabled = false;
          return;
        }
        document.getElementById('dashFrame').src = 'about:blank';
        document.getElementById('view-dashboard').classList.remove('is-visible');
        document.getElementById('view-setup').classList.remove('setup-hidden');
        document.getElementById('runningChrome').classList.remove('is-visible');
        b.disabled = false;
        lastDashboardUrl = null;
        resetNodePill();
        startStatusPoll();
      });
    }

    async function pollLogs(initial = false) {
      const stream = document.getElementById('logStream');
      const pathEl = document.getElementById('logPath');
      if (!stream || !pathEl) return;
      try {
        const res = await invoke('bridge_log_tail', {
          req: { cursor: logCursor, maxBytes: 65536 },
        });
        if (!res) return;
        if (!res.path) {
          if (initial) stream.textContent = 'No bridge log file found yet. Start bridge and wait for first log line.';
          pathEl.textContent = 'Log file: —';
          return;
        }
        if (res.path !== logPath) {
          logPath = res.path;
          logCursor = 0;
          stream.textContent = '';
        }
        pathEl.textContent = `Log file: ${res.path}`;
        const chunk = String(res.text || '');
        if (chunk) {
          stream.textContent += chunk;
          if (stream.textContent.length > 200000) {
            stream.textContent = stream.textContent.slice(-180000);
          }
          stream.scrollTop = stream.scrollHeight;
        } else if (!stream.textContent.trim()) {
          stream.textContent = 'Waiting for log output...';
        }
        logCursor = Number(res.cursor || logCursor);
      } catch (e) {
        if (initial) stream.textContent = `Log stream unavailable: ${String(e && e.message ? e.message : e)}`;
      }
    }

    function startLogPolling(initial = false) {
      if (logPollTimer) clearInterval(logPollTimer);
      pollLogs(initial);
      logPollTimer = setInterval(() => pollLogs(false), 1000);
    }

    function stopLogPolling() {
      if (logPollTimer) clearInterval(logPollTimer);
      logPollTimer = null;
    }

    function parseInstanceLines(text) {
      return String(text || '')
        .split(/\r?\n/)
        .map((l) => l.trim())
        .filter((l) => l.length > 0 && !l.startsWith('#'));
    }

    function parseKaspadExtra(text) {
      const t = text.trim();
      if (!t) return [];
      const lines = t.split(/\r?\n/).map((l) => l.trim()).filter(Boolean);
      if (lines.length > 1) return lines.flatMap((line) => line.split(/\s+/).filter(Boolean));
      return t.split(/\s+/).filter(Boolean);
    }

    function parseOptionalInt(raw, fieldLabel) {
      const s = String(raw || '').trim();
      if (!s) return null;
      const n = Number(s);
      if (!Number.isFinite(n) || n < 0 || !Number.isInteger(n)) {
        throw new Error(`${fieldLabel} must be a non-negative integer`);
      }
      return n;
    }

    /** Checkbox in UI: checked → CLI `true`; unchecked → omit flag (config default). */
    function readCbTrueElseNull(id) {
      const el = document.getElementById(id);
      if (!el) return null;
      return el.checked ? true : null;
    }

    function applyCbTrueElseNull(id, v) {
      const el = document.getElementById(id);
      if (!el) return;
      el.checked = v === true || v === 'true';
    }

    function saveCbChecked(id) {
      const el = document.getElementById(id);
      return !!(el && el.checked);
    }

    function parseInstanceSpecLine(line) {
      const raw = String(line || '').trim();
      const o = { port: '', diff: '', prom: '', raw };
      if (!raw) return o;
      for (const part of raw.split(',')) {
        const idx = part.indexOf('=');
        const key = (idx >= 0 ? part.slice(0, idx) : 'port').trim().toLowerCase();
        const val = (idx >= 0 ? part.slice(idx + 1) : part).trim();
        if (key === 'port' || key === 'stratum' || key === 'stratum_port') o.port = val;
        else if (key === 'diff' || key === 'min_share_diff') o.diff = val;
        else if (key === 'prom' || key === 'prom_port') o.prom = val;
      }
      return o;
    }

    function formatInstanceSpec(o) {
      if (!o || !String(o.port || '').trim()) return '';
      const parts = [`port=${String(o.port).trim()}`];
      if (o.diff && String(o.diff).trim()) parts.push(`diff=${String(o.diff).trim()}`);
      if (o.prom && String(o.prom).trim()) parts.push(`prom=${String(o.prom).trim()}`);
      return parts.join(',');
    }

    let instanceSpecsState = [];
    let instancesEditorWired = false;

    function syncInstancesTextareaFromList() {
      const ta = document.getElementById('instances');
      if (!ta) return;
      ta.value = instanceSpecsState.map((s) => formatInstanceSpec(s)).filter(Boolean).join('\n');
    }

    function loadInstanceSpecsFromTextarea() {
      const ta = document.getElementById('instances');
      const lines = parseInstanceLines(ta ? ta.value : '');
      instanceSpecsState = lines.map((line) => parseInstanceSpecLine(line));
    }

    function renderInstanceRows() {
      const list = document.getElementById('instanceList');
      if (!list) return;
      list.textContent = '';
      syncInstancesTextareaFromList();
      if (instanceSpecsState.length === 0) {
        const empty = document.createElement('div');
        empty.className = 'instance-empty';
        empty.textContent =
          'No extra instances. Use “+ Add instance” for more Stratum ports, or configure a single port in Global Settings / config.yaml.';
        list.appendChild(empty);
        return;
      }
      instanceSpecsState.forEach((spec, idx) => {
        const row = document.createElement('div');
        row.className = 'instance-row';
        row.dataset.idx = String(idx);

        const view = document.createElement('div');
        view.className = 'instance-row-view';
        const title = document.createElement('div');
        title.className = 'instance-row-title';
        title.textContent = `Instance ${idx + 1}`;
        const meta = document.createElement('div');
        meta.className = 'instance-meta';
        [
          ['Port', spec.port || '—'],
          ['Difficulty', spec.diff || '—'],
          ['Prometheus', spec.prom || '—'],
        ].forEach(([k, v]) => {
          const span = document.createElement('span');
          const em = document.createElement('em');
          em.textContent = k;
          span.appendChild(em);
          span.appendChild(document.createTextNode(` ${v}`));
          meta.appendChild(span);
        });
        view.appendChild(title);
        view.appendChild(meta);

        const actions = document.createElement('div');
        actions.className = 'instance-row-actions';
        const btnEdit = document.createElement('button');
        btnEdit.type = 'button';
        btnEdit.className = 'btn-instance btn-instance--edit';
        btnEdit.dataset.action = 'edit';
        btnEdit.textContent = 'Edit';
        const btnRm = document.createElement('button');
        btnRm.type = 'button';
        btnRm.className = 'btn-instance btn-instance--remove';
        btnRm.dataset.action = 'remove';
        btnRm.textContent = 'Remove';
        actions.appendChild(btnEdit);
        actions.appendChild(btnRm);

        const edit = document.createElement('div');
        edit.className = 'instance-row-edit';
        function mkField(labelText, cls, ph, val) {
          const wrap = document.createElement('div');
          wrap.className = 'field';
          const lb = document.createElement('label');
          lb.textContent = labelText;
          const inp = document.createElement('input');
          inp.type = 'text';
          inp.className = cls;
          inp.placeholder = ph;
          inp.value = val;
          wrap.appendChild(lb);
          wrap.appendChild(inp);
          return wrap;
        }
        edit.appendChild(mkField('Port', 'inst-inp-port', ':5555', spec.port || ''));
        edit.appendChild(mkField('Difficulty', 'inst-inp-diff', '8192', spec.diff || ''));
        edit.appendChild(mkField('Prometheus', 'inst-inp-prom', ':2114', spec.prom || ''));
        const editActs = document.createElement('div');
        editActs.className = 'instance-edit-actions';
        const btnSave = document.createElement('button');
        btnSave.type = 'button';
        btnSave.className = 'btn-teal btn-inst-save';
        btnSave.style.padding = '0.4rem 0.75rem';
        btnSave.style.fontSize = '12px';
        btnSave.textContent = 'Save';
        const btnCancel = document.createElement('button');
        btnCancel.type = 'button';
        btnCancel.className = 'btn-muted btn-inst-cancel';
        btnCancel.style.padding = '0.4rem 0.75rem';
        btnCancel.style.fontSize = '12px';
        btnCancel.textContent = 'Cancel';
        editActs.appendChild(btnSave);
        editActs.appendChild(btnCancel);
        edit.appendChild(editActs);

        row.appendChild(view);
        row.appendChild(actions);
        row.appendChild(edit);
        list.appendChild(row);
      });
    }

    function initInstancesEditor() {
      loadInstanceSpecsFromTextarea();
      renderInstanceRows();
      if (instancesEditorWired) return;
      instancesEditorWired = true;
      const addBtn = document.getElementById('btnAddInstance');
      const list = document.getElementById('instanceList');
      if (addBtn) {
        addBtn.addEventListener('click', (e) => {
          e.preventDefault();
          e.stopPropagation();
          instanceSpecsState.push({ port: ':5555', diff: '8192', prom: ':2114', raw: '' });
          renderInstanceRows();
          const rows = document.querySelectorAll('#instanceList .instance-row');
          const last = rows[rows.length - 1];
          if (last) {
            last.classList.add('instance-row--editing');
            last.querySelector('.inst-inp-port')?.focus();
          }
        });
      }
      if (list) {
        list.addEventListener('click', (e) => {
          const btn = e.target.closest('button');
          if (!btn) return;
          const row = btn.closest('.instance-row');
          if (!row || row.dataset.idx === undefined) return;
          const idx = Number(row.dataset.idx);
          if (btn.dataset.action === 'remove') {
            instanceSpecsState.splice(idx, 1);
            renderInstanceRows();
          } else if (btn.dataset.action === 'edit') {
            document.querySelectorAll('#instanceList .instance-row').forEach((r) => r.classList.remove('instance-row--editing'));
            row.classList.add('instance-row--editing');
            row.querySelector('.inst-inp-port')?.focus();
          } else if (btn.classList.contains('btn-inst-save')) {
            const port = row.querySelector('.inst-inp-port')?.value.trim() || '';
            const diff = row.querySelector('.inst-inp-diff')?.value.trim() || '';
            const prom = row.querySelector('.inst-inp-prom')?.value.trim() || '';
            instanceSpecsState[idx] = { port, diff, prom, raw: '' };
            row.classList.remove('instance-row--editing');
            renderInstanceRows();
          } else if (btn.classList.contains('btn-inst-cancel')) {
            row.classList.remove('instance-row--editing');
            renderInstanceRows();
          }
        });
      }
    }

    function collectDto() {
      syncInstancesTextareaFromList();
      return {
        config: document.getElementById('config').value.trim() || null,
        testnet: document.getElementById('testnet').checked,
        nodeMode: document.getElementById('nodeMode').value || null,
        appdir: document.getElementById('appdir').value.trim() || null,
        coinbaseTagSuffix: document.getElementById('coinbase').value.trim() || null,
        kaspadAddress: document.getElementById('kaspadAddress').value.trim() || null,
        blockWaitTimeMs: parseOptionalInt(document.getElementById('blockWaitTimeMs').value, 'Block wait time'),
        printStats: readCbTrueElseNull('printStatsCb'),
        logToFile: readCbTrueElseNull('logToFileCb'),
        healthCheckPort: document.getElementById('healthCheckPort').value.trim() || null,
        webDashboardPort: document.getElementById('webDashboardPort').value.trim() || null,
        varDiff: readCbTrueElseNull('varDiffCb'),
        sharesPerMin: parseOptionalInt(document.getElementById('sharesPerMin').value, 'Shares per minute'),
        varDiffStats: readCbTrueElseNull('varDiffStatsCb'),
        extranonceSize: parseOptionalInt(document.getElementById('extranonceSize').value, 'Extranonce size'),
        pow2Clamp: readCbTrueElseNull('pow2ClampCb'),
        approximateGeoLookup: readCbTrueElseNull('approximateGeoLookupCb'),
        stratumPort: document.getElementById('stratumPort').value.trim() || null,
        promPort: document.getElementById('promPort').value.trim() || null,
        minShareDiff: parseOptionalInt(document.getElementById('minShareDiff').value, 'Min share diff'),
        instances: parseInstanceLines(document.getElementById('instances').value),
        instanceLogToFile: readCbTrueElseNull('instanceLogToFileCb'),
        instanceVarDiff: readCbTrueElseNull('instanceVarDiffCb'),
        instanceSharesPerMin: parseOptionalInt(
          document.getElementById('instanceSharesPerMin').value,
          'Instance shares per minute',
        ),
        instanceVarDiffStats: readCbTrueElseNull('instanceVarDiffStatsCb'),
        instancePow2Clamp: readCbTrueElseNull('instancePow2ClampCb'),
        internalCpuMiner: document.getElementById('internalCpuMiner').checked,
        internalCpuMinerAddress: document.getElementById('internalCpuMinerAddress').value.trim() || null,
        internalCpuMinerThreads: parseOptionalInt(
          document.getElementById('internalCpuMinerThreads').value,
          'CPU miner threads',
        ),
        internalCpuMinerThrottleMs: parseOptionalInt(
          document.getElementById('internalCpuMinerThrottleMs').value,
          'CPU miner throttle ms',
        ),
        internalCpuMinerTemplatePollMs: parseOptionalInt(
          document.getElementById('internalCpuMinerTemplatePollMs').value,
          'CPU miner template poll ms',
        ),
        kaspadExtraArgs: parseKaspadExtra(document.getElementById('kaspadArgs').value),
      };
    }

    function applySaved() {
      try {
        let raw = localStorage.getItem(LS_KEY);
        if (!raw) raw = localStorage.getItem(LS_KEY_LEGACY);
        if (!raw) return;
        const o = JSON.parse(raw);
        if (o.config != null) document.getElementById('config').value = o.config;
        if (typeof o.testnet === 'boolean') document.getElementById('testnet').checked = o.testnet;
        if (o.nodeMode != null) document.getElementById('nodeMode').value = o.nodeMode;
        if (o.appdir != null) document.getElementById('appdir').value = o.appdir;
        if (o.coinbase != null) document.getElementById('coinbase').value = o.coinbase;
        if (o.kaspadAddress != null) document.getElementById('kaspadAddress').value = o.kaspadAddress;
        if (o.blockWaitTimeMs != null) document.getElementById('blockWaitTimeMs').value = o.blockWaitTimeMs;
        if (o.healthCheckPort != null) document.getElementById('healthCheckPort').value = o.healthCheckPort;
        if (o.webDashboardPort != null) document.getElementById('webDashboardPort').value = o.webDashboardPort;
        if (o.sharesPerMin != null) document.getElementById('sharesPerMin').value = o.sharesPerMin;
        if (o.extranonceSize != null) document.getElementById('extranonceSize').value = o.extranonceSize;
        if (o.stratumPort != null) document.getElementById('stratumPort').value = o.stratumPort;
        if (o.promPort != null) document.getElementById('promPort').value = o.promPort;
        if (o.minShareDiff != null) document.getElementById('minShareDiff').value = o.minShareDiff;
        if (o.instances != null) {
          const inst = document.getElementById('instances');
          inst.value = Array.isArray(o.instances) ? o.instances.join('\n') : String(o.instances);
        }
        if (o.instanceSharesPerMin != null) document.getElementById('instanceSharesPerMin').value = o.instanceSharesPerMin;
        applyCbTrueElseNull('printStatsCb', o.printStats);
        applyCbTrueElseNull('logToFileCb', o.logToFile);
        applyCbTrueElseNull('varDiffCb', o.varDiff);
        applyCbTrueElseNull('varDiffStatsCb', o.varDiffStats);
        applyCbTrueElseNull('pow2ClampCb', o.pow2Clamp);
        applyCbTrueElseNull('approximateGeoLookupCb', o.approximateGeoLookup);
        applyCbTrueElseNull('instanceLogToFileCb', o.instanceLogToFile);
        applyCbTrueElseNull('instanceVarDiffCb', o.instanceVarDiff);
        applyCbTrueElseNull('instanceVarDiffStatsCb', o.instanceVarDiffStats);
        applyCbTrueElseNull('instancePow2ClampCb', o.instancePow2Clamp);
        if (typeof o.internalCpuMiner === 'boolean') document.getElementById('internalCpuMiner').checked = o.internalCpuMiner;
        if (o.internalCpuMinerAddress != null) document.getElementById('internalCpuMinerAddress').value = o.internalCpuMinerAddress;
        if (o.internalCpuMinerThreads != null) document.getElementById('internalCpuMinerThreads').value = o.internalCpuMinerThreads;
        if (o.internalCpuMinerThrottleMs != null) document.getElementById('internalCpuMinerThrottleMs').value = o.internalCpuMinerThrottleMs;
        if (o.internalCpuMinerTemplatePollMs != null) document.getElementById('internalCpuMinerTemplatePollMs').value = o.internalCpuMinerTemplatePollMs;
        if (o.kaspadArgs != null) document.getElementById('kaspadArgs').value = o.kaspadArgs;
      } catch (_) { /* ignore */ }
    }

    function saveConfiguration() {
      syncInstancesTextareaFromList();
      const o = {
        config: document.getElementById('config').value,
        testnet: document.getElementById('testnet').checked,
        nodeMode: document.getElementById('nodeMode').value,
        appdir: document.getElementById('appdir').value,
        coinbase: document.getElementById('coinbase').value,
        kaspadAddress: document.getElementById('kaspadAddress').value,
        blockWaitTimeMs: document.getElementById('blockWaitTimeMs').value,
        healthCheckPort: document.getElementById('healthCheckPort').value,
        webDashboardPort: document.getElementById('webDashboardPort').value,
        sharesPerMin: document.getElementById('sharesPerMin').value,
        extranonceSize: document.getElementById('extranonceSize').value,
        stratumPort: document.getElementById('stratumPort').value,
        promPort: document.getElementById('promPort').value,
        minShareDiff: document.getElementById('minShareDiff').value,
        instances: document.getElementById('instances').value,
        instanceSharesPerMin: document.getElementById('instanceSharesPerMin').value,
        printStats: saveCbChecked('printStatsCb'),
        logToFile: saveCbChecked('logToFileCb'),
        varDiff: saveCbChecked('varDiffCb'),
        varDiffStats: saveCbChecked('varDiffStatsCb'),
        pow2Clamp: saveCbChecked('pow2ClampCb'),
        approximateGeoLookup: saveCbChecked('approximateGeoLookupCb'),
        instanceLogToFile: saveCbChecked('instanceLogToFileCb'),
        instanceVarDiff: saveCbChecked('instanceVarDiffCb'),
        instanceVarDiffStats: saveCbChecked('instanceVarDiffStatsCb'),
        instancePow2Clamp: saveCbChecked('instancePow2ClampCb'),
        internalCpuMiner: document.getElementById('internalCpuMiner').checked,
        internalCpuMinerAddress: document.getElementById('internalCpuMinerAddress').value,
        internalCpuMinerThreads: document.getElementById('internalCpuMinerThreads').value,
        internalCpuMinerThrottleMs: document.getElementById('internalCpuMinerThrottleMs').value,
        internalCpuMinerTemplatePollMs: document.getElementById('internalCpuMinerTemplatePollMs').value,
        kaspadArgs: document.getElementById('kaspadArgs').value,
      };
      localStorage.setItem(LS_KEY, JSON.stringify(o));
      const t = document.getElementById('toast');
      t.textContent = 'Configuration saved';
      t.classList.add('show');
      setTimeout(() => t.classList.remove('show'), 2200);
    }

    function updateBridgePill(bridgeRunning) {
      const pillB = document.getElementById('pillBridge');
      const txtB = document.getElementById('txtBridge');
      if (bridgeRunning) {
        txtB.textContent = 'Running';
        pillB.classList.add('on');
      } else {
        txtB.textContent = 'Stopped';
        pillB.classList.remove('on');
      }
    }

    function resetNodePill() {
      const pillN = document.getElementById('pillNode');
      const txtN = document.getElementById('txtNode');
      txtN.textContent = 'Not connected';
      pillN.classList.remove('on', 'warn');
    }

    async function refreshNodeFromStatusApi() {
      const url = lastDashboardUrl;
      const pillN = document.getElementById('pillNode');
      const txtN = document.getElementById('txtNode');
      if (!url) {
        resetNodePill();
        return;
      }
      let origin;
      try {
        origin = new URL(String(url).trim()).origin;
      } catch {
        txtN.textContent = 'Status unknown';
        pillN.classList.remove('on');
        pillN.classList.add('warn');
        return;
      }
      try {
        const res = await fetch(`${origin}/api/status`);
        if (!res.ok) throw new Error(String(res.status));
        const j = await res.json();
        const connected = !!(j.node && j.node.isConnected);
        const synced = j.node && j.node.isSynced;
        if (connected) {
          let extra = '';
          if (synced === true) extra = ' · synced';
          else if (synced === false) extra = ' · syncing';
          txtN.textContent = `Connected${extra}`;
          pillN.classList.add('on');
          pillN.classList.remove('warn');
        } else {
          txtN.textContent = 'Not connected';
          pillN.classList.remove('on');
          pillN.classList.add('warn');
        }
      } catch {
        txtN.textContent = 'Status unknown';
        pillN.classList.remove('on');
        pillN.classList.add('warn');
      }
    }

    let statusTimer;
    function startStatusPoll() {
      if (statusTimer) clearInterval(statusTimer);
      const tick = async () => {
        try {
          const on = await invoke('bridge_is_running');
          updateBridgePill(!!on);
          if (on && lastDashboardUrl) {
            await refreshNodeFromStatusApi();
          } else {
            resetNodePill();
          }
        } catch (_) { /* ignore */ }
      };
      tick();
      statusTimer = setInterval(tick, 2000);
    }
    function stopStatusPoll() {
      if (statusTimer) clearInterval(statusTimer);
    }

    /** Vendored operator UI (`bridge-tauri/ui/dashboard/*`) + `?api=` bridge HTTP origin for JSON APIs. */
    function dashboardEmbedSrc(dashboardHttpUrl) {
      const origin = new URL(dashboardHttpUrl).origin;
      return (
        'dashboard/index.html?api=' +
        encodeURIComponent(origin) +
        '&embeddedChrome=1'
      );
    }

    function showRunningDashboard(dashboardHttpUrl) {
      lastDashboardUrl = dashboardHttpUrl;
      document.getElementById('loader').style.display = 'none';
      document.getElementById('shell').style.display = 'flex';
      document.getElementById('view-setup').classList.add('setup-hidden');
      document.getElementById('view-dashboard').classList.add('is-visible');
      document.getElementById('runningChrome').classList.add('is-visible');
      document.getElementById('dashFrame').src = dashboardEmbedSrc(dashboardHttpUrl);
      wireChrome();
      startStatusPoll();
    }

    async function connectCliMode() {
      const msg = document.getElementById('msg');
      const errEl = document.getElementById('loaderErr');
      startStatusPoll();
      let url = 'http://127.0.0.1:3030/';
      try {
        url = await invoke('dashboard_default_url');
      } catch (e) {
        errEl.style.display = 'block';
        errEl.textContent = String(e && e.message ? e.message : e);
        msg.textContent = 'Could not read dashboard URL.';
        stopStatusPoll();
        return;
      }
      showRunningDashboard(url);
    }

    function showErr(text) {
      const errBox = document.getElementById('err');
      errBox.textContent = text;
      errBox.classList.add('show');
    }
    function hideErr() {
      const errBox = document.getElementById('err');
      errBox.classList.remove('show');
    }

    async function maybeShowCpuSection() {
      try {
        const on = await invoke('cpu_miner_feature_enabled');
        const el = document.getElementById('detCpu');
        const nav = document.getElementById('setupNavCpu');
        if (el) el.style.display = on ? '' : 'none';
        if (nav) nav.classList.toggle('is-cpu-nav-visible', !!on);
      } catch {
        const el = document.getElementById('detCpu');
        const nav = document.getElementById('setupNavCpu');
        if (el) el.style.display = 'none';
        if (nav) nav.classList.remove('is-cpu-nav-visible');
      }
    }

    /** Show exactly one setup “page”; inactive `<details>` are hidden (not stacked). */
    function showSetupPage(id) {
      const root = document.getElementById('setupScroll');
      if (!root) return;
      const target = id ? document.getElementById(id) : null;
      if (!target || target.tagName !== 'DETAILS' || !target.classList.contains('setup-page')) return;
      if (target.style.display === 'none') return;
      [...root.querySelectorAll('details.setup-page')].forEach((d) => {
        const on = d === target;
        d.classList.toggle('is-setup-active', on);
        d.open = on;
      });
      syncSetupNavFromActivePage();
      root.scrollTop = 0;
    }

    function syncSetupNavFromActivePage() {
      const active = document.querySelector('#setupScroll details.setup-page.is-setup-active');
      const id = active?.id ?? null;
      document.querySelectorAll('.setup-nav-btn[data-setup-jump]').forEach((b) => {
        b.classList.toggle('is-active', id !== null && b.getAttribute('data-setup-jump') === id);
      });
    }

    function initFirstRunUx() {
      const banner = document.getElementById('firstRunHint');
      const dismiss = document.getElementById('btnDismissFirstRun');
      if (!banner || !dismiss) return;
      if (localStorage.getItem(LS_FIRST_RUN_DISMISSED) === '1') {
        banner.hidden = true;
        return;
      }
      banner.hidden = false;
      dismiss.addEventListener('click', () => {
        localStorage.setItem(LS_FIRST_RUN_DISMISSED, '1');
        banner.hidden = true;
      });
    }

    function wireConnectionHelpJump() {
      const b = document.getElementById('btnJumpHelp');
      if (b) b.addEventListener('click', () => showSetupPage('detHow'));
    }

    let setupSectionNavWired = false;
    function wireSetupSectionNav() {
      if (setupSectionNavWired) return;
      setupSectionNavWired = true;
      const buttons = [...document.querySelectorAll('.setup-nav-btn[data-setup-jump]')];
      if (buttons.length === 0) return;
      buttons.forEach((btn) => {
        btn.addEventListener('click', () => {
          const id = btn.getAttribute('data-setup-jump');
          showSetupPage(id);
        });
      });
    }

    async function runGui() {
      document.getElementById('loader').style.display = 'none';
      document.getElementById('shell').style.display = 'flex';
      wireChrome();
      startStatusPoll();

      const d = await invoke('gui_defaults');
      await maybeShowCpuSection();
      wireSetupSectionNav();
      initFirstRunUx();
      wireConnectionHelpJump();
      syncSetupNavFromActivePage();
      applySaved();
      initInstancesEditor();
      if (d.configPath && !document.getElementById('config').value) {
        document.getElementById('config').value = d.configPath;
        document.getElementById('configHint').textContent =
          'Found config.yaml next to the executable. You can change or clear this path.';
      }
      if (d.suggestedAppdir) {
        const ad = document.getElementById('appdir');
        if (!ad.value) ad.placeholder = d.suggestedAppdir;
        document.getElementById('appdirHint').textContent =
          'Suggested: ' + d.suggestedAppdir;
      }

      document.getElementById('btnDocs').addEventListener('click', () => {
        invoke('open_bridge_documentation').catch((e) => alert(String(e)));
      });
      document.getElementById('btnRunNode').addEventListener('click', () => {
        document.getElementById('nodeMode').value = 'inprocess';
        showSetupPage('detGlobal');
        const adv = document.querySelector('#detGlobal .advanced-fold');
        if (adv) adv.open = true;
      });
      document.getElementById('btnSave').addEventListener('click', saveConfiguration);

      const btn = document.getElementById('start');
      btn.addEventListener('click', async () => {
        hideErr();
        btn.disabled = true;
        try {
          const dto = collectDto();
          const url = await invoke('start_bridge', { dto });
          await invoke('wait_for_dashboard_http', { url });
          showRunningDashboard(url);
        } catch (e) {
          document.getElementById('loader').style.display = 'none';
          document.getElementById('shell').style.display = 'flex';
          showErr(String(e && e.message ? e.message : e));
          startStatusPoll();
        } finally {
          btn.disabled = false;
        }
      });
    }

    tickShellLocalTime();
    setInterval(tickShellLocalTime, 1000);

    (async () => {
      const msg = document.getElementById('msg');
      try {
        const alreadyRunning = await invoke('bridge_is_running');
        if (alreadyRunning) {
          let url;
          try {
            url = await invoke('dashboard_default_url');
          } catch (e) {
            document.getElementById('loaderErr').style.display = 'block';
            document.getElementById('loaderErr').textContent = String(e && e.message ? e.message : e);
            msg.textContent = 'Could not read dashboard URL.';
            return;
          }
          showRunningDashboard(url);
          return;
        }

        const cli = await invoke('is_cli_mode');
        if (cli) {
          msg.textContent = 'Command-line mode: starting dashboard…';
          await connectCliMode();
        } else {
          msg.textContent = '';
          await runGui();
        }
      } catch (e) {
        document.getElementById('loaderErr').style.display = 'block';
        document.getElementById('loaderErr').textContent = String(e && e.message ? e.message : e);
        msg.textContent = 'Tauri API unavailable.';
      }
    })();
