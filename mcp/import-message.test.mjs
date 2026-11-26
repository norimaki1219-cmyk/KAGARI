import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, readFileSync, readdirSync, writeFileSync, rmSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

test('mail and Slack messages survive a KAGARI queue round trip', t => {
  const repo = fileURLToPath(new URL('../', import.meta.url));
  const lab = mkdtempSync(path.join(os.tmpdir(), 'kagari-import-'));
  t.after(() => rmSync(lab, {recursive: true, force: true}));
  const root = path.join(lab, 'RustAIWorkspace');
  const project = path.join(lab, 'テスト');
  mkdirSync(root);
  mkdirSync(project);
  const state = path.join(root, 'workspace.tsv');
  writeFileSync(state, 'P\t' + project + '\n');
  function run(cmd, args, input) {
    const proc = spawnSync(cmd, args, {env: {...process.env, LOCALAPPDATA: lab}, input, encoding: 'utf8', cwd: repo});
    assert.ifError(proc.error);
    assert.equal(proc.status, 0, proc.stderr);
    assert.equal(proc.stderr, '');
    return proc.stdout;
  }
  let routingArgs = [];
  function call(method, params = {}, target = project) {
    const req = JSON.stringify({jsonrpc: '2.0', id: 1, method, params});
    return JSON.parse(run(process.execPath, ['mcp/import-message.mjs', target, ...routingArgs], req + '\n')).result;
  }
  assert.ok(call('initialize').capabilities.tools);
  assert.equal(call('tools/list').tools[0].name, 'import_message');
  const mail = {source: 'gmail', scope: 'test@example.invalid', message_id: '123', title: '確認依頼', action: '添付を\n確認する'};
  const slack = {...mail, source: 'slack', scope: 'T01/C02', message_id: '17.00001', url: 'https://example.invalid/msg'};
  const submit = (args, target) => call('tools/call', {name: 'import_message', arguments: args}, target);
  const status = args => JSON.parse(submit(args).content[0].text).status;
  assert.equal(submit({...mail, message_id: ''}).isError, true);
  assert.equal(submit(mail, lab).isError, true);
  assert.equal(status(mail), 'queued');
  assert.equal(status({...mail, title: '更新後'}), 'already_queued');
  assert.equal(status(slack), 'queued');
  const second = path.join(lab, '案件B');
  mkdirSync(second);
  writeFileSync(state, 'P\t' + project + '\nP\t' + second + '\n');
  const config = path.join(lab, 'rules.json');
  routingArgs = [config];
  const setRules = rules => writeFileSync(config, JSON.stringify(rules));
  const routed = {...mail, message_id: '456', sender: 'Client@example.invalid', body: '案件Bの見積について'};
  const decide = args => JSON.parse(submit(args).content[0].text);
  assert.equal(submit(routed).isError, true);
  setRules([{contains: '見積', project: lab}]);
  assert.equal(submit(routed).isError, true);
  setRules([{project: second}]);
  assert.equal(submit(routed).isError, true);
  setRules([
    {source: 'gmail', sender: 'client@example.invalid', contains: '見積', project: second},
    {contains: '見積', project},
    {sender: 'U0123', project: second},
    {contains: '障害', project: second},
  ]);
  assert.equal(decide(routed).project, second);
  assert.equal(decide({...routed, message_id: '457', sender: 'other@example.invalid'}).project, project);
  assert.equal(decide({...slack, message_id: '18', sender: 'U0123'}).project, second);
  assert.equal(decide({...mail, message_id: '458', subject: '障害のお知らせ'}).project, second);
  assert.equal(decide({...mail, message_id: '459'}).project, project);
  setRules([]);
  assert.equal(status(routed), 'already_queued');
  const profileDir = path.join(root, 'message-routing');
  mkdirSync(profileDir);
  const profileKey = createHash('sha256').update(path.toNamespacedPath(project).toLowerCase()).digest('hex');
  const profile = path.join(profileDir, profileKey + '.json');
  writeFileSync(profile, JSON.stringify({defaultProject: second, rules: [{contains: '確認', project}]}));
  assert.equal(decide({...mail, message_id: 'gui-rule'}).project, project);
  assert.equal(decide({...mail, message_id: 'gui-default', title: '次回', action: '返信する'}).project, second);
  assert.equal(status(mail), 'already_queued');
  writeFileSync(profile, '{');
  assert.equal(submit(mail).isError, true);
  writeFileSync(profile, JSON.stringify({defaultProject: second, rules: []}));
  const ps = "$ProgressPreference='SilentlyContinue'\n$ErrorActionPreference='Stop'\n" +
    "Add-Type 'using System.Runtime.InteropServices; public class Core { [DllImport(\"kagari_core.dll\")] public static extern int kagari_apply_pending_offline(); }'\n" +
    "exit [Core]::kagari_apply_pending_offline()";
  run('powershell.exe', ['-NoProfile', '-NonInteractive', '-EncodedCommand', Buffer.from(ps, 'utf16le').toString('base64')]);
  const tasks = readFileSync(state, 'utf8').split(/\r?\n/).filter(r => r.startsWith('T\t')).map(r => r.split('\t'));
  assert.equal(tasks.length, 9);
  assert.equal(tasks.filter(r => r[1] === project).length, 5);
  assert.equal(tasks.filter(r => r[1] === second).length, 4);
  assert.ok(tasks.every(r => r[4] === '0'));
  assert.equal(tasks.filter(r => r[16].includes('添付を 確認する')).length, 8);
  assert.ok(tasks.some(r => r[16].includes(slack.url)));
  assert.ok(tasks.some(r => r[16].includes(mail.scope)));
  assert.equal(status(mail), 'processed');
  assert.equal(status(routed), 'processed');
  assert.deepEqual(readdirSync(path.join(root, 'queue-inbox')), []);
});
