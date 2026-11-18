// Synthetic application and trusted fixture controller for Proof A.
// No automation endpoint, authenticated application, or provider is involved.
const assert = require('node:assert/strict');
const { execFileSync, spawn } = require('node:child_process');
const fs = require('node:fs');
const https = require('node:https');
const path = require('node:path');
const { createInterface } = require('node:readline');
const { once } = require('node:events');
const { firefox } = require('playwright');

const [driver, output, certutil = 'certutil'] = process.argv.slice(2);
assert(driver && output, 'Usage: node scoped-https.cjs DRIVER NEW_EVIDENCE_DIR [CERTUTIL]');

const pages = {
  '/': `<!doctype html><html lang="en"><meta charset="utf-8"><title>Shared fixture</title>
<main><h1>Shared fixture</h1><p role="status">Pending</p>
<button id="approve">Approve fixture</button> <button id="open">Open details</button>
<a href="/popup" target="_blank" rel="noopener">Open separate page</a>
<dialog><h2>Fixture details</h2><form method="dialog"><button>Close details</button></form></dialog>
</main><script>
document.querySelector('#approve').onclick = () => document.querySelector('[role=status]').textContent = 'Approved';
document.querySelector('#open').onclick = () => document.querySelector('dialog').showModal();
</script>`,
  '/popup': '<!doctype html><title>Ungranted popup</title><h1>Ungranted popup</h1>',
  '/other': '<!doctype html><title>Ungranted tab</title><button>Do not touch</button>',
};

function certificates(dir) {
  const run = (program, args) => execFileSync(program, args, { cwd: dir, stdio: 'pipe' });
  run('openssl', ['req', '-x509', '-newkey', 'rsa:2048', '-nodes', '-days', '1',
    '-subj', '/CN=HyprDeck ephemeral fixture CA', '-addext', 'basicConstraints=critical,CA:TRUE,pathlen:0',
    '-addext', 'keyUsage=critical,keyCertSign,cRLSign', '-keyout', 'ca.key', '-out', 'ca.pem']);
  run('openssl', ['req', '-new', '-newkey', 'rsa:2048', '-nodes', '-subj', '/CN=localhost',
    '-keyout', 'server.key', '-out', 'server.csr']);
  fs.writeFileSync(path.join(dir, 'tls.ext'), 'basicConstraints=critical,CA:FALSE\nextendedKeyUsage=serverAuth\nsubjectAltName=DNS:localhost\n');
  run('openssl', ['x509', '-req', '-days', '1', '-in', 'server.csr', '-CA', 'ca.pem',
    '-CAkey', 'ca.key', '-CAcreateserial', '-extfile', 'tls.ext', '-out', 'server.pem']);

  const nss = path.join(dir, 'trusted', 'profile');
  fs.mkdirSync(nss, { recursive: true });
  run(certutil, ['-N', '-d', `sql:${nss}`, '--empty-password']);
  run(certutil, ['-A', '-d', `sql:${nss}`, '-n', 'HyprDeck fixture CA', '-t', 'C,,', '-i', 'ca.pem']);
}

async function browser(dir, trust) {
  const data = path.join(dir, trust);
  const runtime = path.join(data, 'runtime');
  fs.mkdirSync(runtime, { recursive: true, mode: 0o700 });
  return firefox.launchPersistentContext(path.join(data, 'profile'), {
    viewport: { width: 960, height: 540 },
    ignoreHTTPSErrors: false,
    timeout: 15000,
    firefoxUserPrefs: { 'network.proxy.type': 0 },
    env: { PATH: process.env.PATH, LANG: 'C.UTF-8',
      XDG_CONFIG_HOME: path.join(data, 'config'), XDG_DATA_HOME: data,
      XDG_CACHE_HOME: path.join(data, 'cache'), XDG_RUNTIME_DIR: runtime },
  });
}

async function proof(dir, evidence) {
  certificates(dir);
  const server = https.createServer({ key: fs.readFileSync(path.join(dir, 'server.key')),
    cert: fs.readFileSync(path.join(dir, 'server.pem')) }, (request, response) => {
    const body = Object.hasOwn(pages, request.url) ? pages[request.url] : null;
    response.writeHead(body ? 200 : 404, { 'Content-Type': 'text/html; charset=utf-8' });
    response.end(body);
  });
  const journal = fs.openSync(path.join(evidence, 'journal.jsonl'), 'wx', 0o600);
  const start = performance.now();
  let sequence = 0;
  function record(stage, fields = {}) {
    fs.writeSync(journal, JSON.stringify({ sequence: ++sequence, stage,
      ms: Math.round(performance.now() - start), ...fields }) + '\n');
    fs.fsyncSync(journal);
  }
  let context, child;
  try {
    await new Promise((resolve, reject) => {
      server.once('error', reject);
      server.listen(0, '127.0.0.1', resolve);
    });
    const origin = `https://localhost:${server.address().port}`;
    context = await browser(dir, 'untrusted');
    await assert.rejects(context.pages()[0].goto(origin), /SEC_ERROR_UNKNOWN_ISSUER/);
    record('untrusted_ca_rejected');
    await context.close();

    context = await browser(dir, 'trusted');
    context.setDefaultTimeout(5000);
    const shared = context.pages()[0];
    await assert.rejects(shared.goto(`https://127.0.0.1:${server.address().port}`), /SSL_ERROR_BAD_CERT_DOMAIN/);
    record('incorrect_tls_hostname_rejected');
    await context.route('**/*', route => {
      const url = new URL(route.request().url());
      return url.origin === origin && Object.hasOwn(pages, url.pathname) ? route.continue() : route.abort();
    });
    assert.equal((await shared.goto(origin)).status(), 200);
    record('tls_validated');
    const other = await context.newPage();
    await other.goto(`${origin}/other`);
    child = spawn(path.resolve(driver), { stdio: ['pipe', 'pipe', 'inherit'] });
    const lines = createInterface({ input: child.stdout })[Symbol.asyncIterator]();
    async function command(message) {
      child.stdin.write(JSON.stringify(message) + '\n');
      const { value, done } = await lines.next();
      assert(!done, 'Scoped session driver stopped');
      return JSON.parse(value);
    }
    let revision = 0;
    let action = 0;
    const activate = {
      approve: () => shared.getByRole('button', { name: 'Approve fixture' }).click(),
      dialog: () => shared.getByRole('button', { name: 'Open details' }).click(),
      close: () => shared.getByRole('button', { name: 'Close details' }).click(),
      popup: () => shared.getByRole('link', { name: 'Open separate page' }).click(),
    };
    const semantic = () => shared.locator('main').ariaSnapshot();
    const visual = () => shared.screenshot({ path: path.join(evidence, 'shared.png') });
    const request = (operation, resource = 1, destination = 'local') => ({ operation, destination,
      revision, binding: { runtime: 1, task: 1, resource } });

    async function dispatch(req, name, invoke, expected = null) {
      const id = ++action;
      const fields = { action: id, operation: req.operation, resource: req.binding.resource, target: name };
      record('intent', fields);
      const { denial } = await command({ type: 'check', request: req });
      record('permission', { action: id, denial });
      assert.equal(denial, expected);
      if (denial) return;
      assert.equal(typeof invoke, 'function');
      record('dispatch', { action: id });
      try {
        const result = await invoke();
        record('result', { action: id, disposition: 'completed' });
        return result;
      } catch (error) {
        record('result', { action: id, disposition: 'unknown' });
        throw error;
      }
    }
    async function lifecycle(type, detail) {
      revision = (await command({ type })).revision;
      record('lifecycle', { control: type, detail, revision });
    }

    const snapshot = await dispatch(request('observe_semantic'), 'shared', semantic);
    assert.match(snapshot, /status: Pending/);
    fs.writeFileSync(path.join(evidence, 'shared.aria.txt'), snapshot);
    record('evidence', { action, file: 'shared.aria.txt', bytes: Buffer.byteLength(snapshot) });
    await dispatch(request('observe_visual'), 'shared', visual);
    record('evidence', { action, file: 'shared.png', bytes: fs.statSync(path.join(evidence, 'shared.png')).size });
    await dispatch(request('observe_visual', 1, 'external'), 'shared', visual, 'destination');
    await dispatch(request('activate', 2), 'ungranted_tab', undefined, 'wrong_binding');

    const queued = request('activate');
    await other.bringToFront();
    await lifecycle('pause', 'human_focus_other_tab');
    await dispatch(queued, 'approve', activate.approve, 'paused');
    await shared.bringToFront();
    await lifecycle('resume', 'human_returns_to_shared_tab');
    await dispatch(queued, 'approve', activate.approve, 'stale_revision');
    await dispatch(request('activate'), 'approve', activate.approve);
    assert.equal(await shared.getByRole('status').innerText(), 'Approved');
    record('verification', { action, check: 'status_approved' });

    await dispatch(request('activate'), 'dialog', activate.dialog);
    assert.equal(await shared.getByRole('dialog').isVisible(), true);
    await lifecycle('advance', 'modal_open');
    await dispatch(request('activate'), 'close', activate.close);
    assert.equal(await shared.getByRole('dialog').isVisible(), false);
    await lifecycle('advance', 'modal_closed');
    const popupEvent = context.waitForEvent('page');
    await dispatch(request('activate'), 'popup', activate.popup);
    const popup = await popupEvent;
    await popup.waitForURL(`${origin}/popup`);
    await lifecycle('advance', 'popup_created');
    await dispatch(request('observe_semantic', 3), 'ungranted_popup', undefined, 'wrong_binding');
    await popup.close();
    await shared.bringToFront();

    const delayed = request('activate');
    const revokeStart = performance.now();
    await lifecycle('revoke', 'human_revoked');
    await dispatch(delayed, 'dialog', activate.dialog, 'revoked');
    const revokeMs = Math.round(performance.now() - revokeStart);
    assert.equal(await shared.getByRole('dialog').isVisible(), false);
    assert.equal(await other.getByRole('button').innerText(), 'Do not touch');
    record('verification', { action, check: 'revocation_blocked_dispatch',
      elapsed_ms: revokeMs });
    console.log(`Proof A browser fixture passed. Evidence: ${evidence}`);
  } catch (error) {
    record('failed');
    throw error;
  } finally {
    if (child && child.exitCode === null && child.signalCode === null) {
      const exit = once(child, 'close');
      child.stdin.end();
      await exit;
    }
    try { await context?.close(); } finally {
      server.closeAllConnections();
      server.close();
      fs.closeSync(journal);
    }
  }
}

async function main() {
  const evidence = path.resolve(output);
  fs.mkdirSync(evidence, { mode: 0o700 });
  const scratch = fs.mkdtempSync(path.join(path.dirname(evidence), '.hyprdeck-https-'));
  try { await proof(scratch, evidence); } finally {
    fs.rmSync(scratch, { recursive: true, force: true });
  }
}

main().catch(error => { console.error(error); process.exitCode = 1; });
