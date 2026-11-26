import { createHash, randomUUID } from 'node:crypto';
import { link, mkdir, readFile, unlink, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { createInterface } from 'node:readline';

const fields = {
  source: '取得元のサービス名（gmail、slack など）',
  scope: 'メールアカウント、または Slack の workspace ID/channel ID',
  message_id: 'メールの固定 ID、または Slack の個別投稿 ts（文字列）',
  title: '作業名',
  action: '連絡から抽出した具体的な依頼内容',
  sender: '元の差出人メールアドレスまたは Slack 投稿者 ID（取得できる場合に指定）',
  subject: '元の件名（取得できる場合に指定）',
  body: '元の本文（取得できる場合に指定、振り分け判定用）',
  url: '元の連絡を開く https URL（任意）',
};
const required = ['source', 'scope', 'message_id', 'title', 'action'];
const tool = {
  name: 'import_message',
  description: '選んだ外部連絡を差出元・内容の設定に従って登録済みプロジェクトに未着手で取り込む。判定用に元の差出人・件名・本文を渡す。',
  inputSchema: {
    type: 'object', required, additionalProperties: false,
    properties: Object.fromEntries(Object.entries(fields).map(([key, description]) =>
      [key, { type: 'string', minLength: 1, description }])),
  },
};

const projectArg = process.argv[2];
if (!projectArg || !path.isAbsolute(projectArg) || !process.env.LOCALAPPDATA) {
  console.error('Usage: node import-message.mjs <absolute registered project path> (LOCALAPPDATA required)');
  process.exit(1);
}
const root = path.join(process.env.LOCALAPPDATA, 'RustAIWorkspace');
const projectKey = p => path.resolve(path.toNamespacedPath(p)).toLowerCase();
const rulesFile = process.argv[3];
const profileId = createHash('sha256').update(projectKey(projectArg).replace(/[\\/]+$/, '')).digest('hex');
const profileFile = path.join(root, 'message-routing', profileId + '.json');

async function selectProject(values, projects) {
  let destination = projectArg;
  let rules;
  let profileText;
  try {
    profileText = await readFile(profileFile, 'utf8');
  } catch (err) {
    if (err.code !== 'ENOENT') throw err;
  }
  if (profileText !== undefined) {
    const profile = JSON.parse(profileText.replace(/^\uFEFF/, ''));
    if (!profile || typeof profile.defaultProject !== 'string' || !path.isAbsolute(profile.defaultProject) ||
        !Array.isArray(profile.rules)) throw new Error('KAGARI の外部連絡取込設定が無効です');
    destination = profile.defaultProject;
    rules = profile.rules;
  } else if (rulesFile) {
    rules = JSON.parse((await readFile(rulesFile, 'utf8')).replace(/^\uFEFF/, ''));
  }
  if (rules !== undefined) {
    if (!Array.isArray(rules)) throw new Error('振り分け設定はルールの配列で指定してください');
    const content = [values.subject, values.body, values.title, values.action]
      .filter(Boolean).map(s => s.toLowerCase());
    let matched = false;
    for (const [index, rule] of rules.entries()) {
      const keys = Object.keys(rule ?? {});
      if (!rule || typeof rule.project !== 'string' || !path.isAbsolute(rule.project) ||
          !keys.some(k => k === 'sender' || k === 'contains') ||
          keys.some(k => !['sender', 'contains', 'source', 'project'].includes(k) ||
            typeof rule[k] !== 'string' || !rule[k].trim())) {
        throw new Error('振り分け設定の ' + (index + 1) + ' 番目のルールが無効です');
      }
      if (!matched && (!rule.source || values.source.toLowerCase() === rule.source.trim().toLowerCase()) &&
          (!rule.sender || values.sender?.toLowerCase() === rule.sender.trim().toLowerCase()) &&
          (!rule.contains || content.some(s => s.includes(rule.contains.trim().toLowerCase())))) {
        destination = rule.project;
        matched = true;
      }
    }
  }
  const project = projects.find(p => projectKey(p) === projectKey(destination));
  if (!project) throw new Error('KAGARI に登録済みのプロジェクトを指定してください: ' + destination);
  return project;
}

async function enqueue(args) {
  if (!args || typeof args !== 'object' || Object.keys(args).some(k => !Object.hasOwn(fields, k))) {
    throw new Error('入力項目を確認してください');
  }
  const values = {};
  for (const key of new Set([...required, ...Object.keys(args)])) {
    if (typeof args[key] !== 'string') throw new Error(key + ' は文字列で指定してください');
    const value = args[key].trim();
    const clean = ['title', 'action', 'body', 'subject'].includes(key) ? value.replace(/[\t\n\r]+/g, ' ') : value;
    if (!clean || /[\x00-\x1f\x7f]/.test(clean)) throw new Error(key + ' が無効です');
    values[key] = clean;
  }
  const {scope, message_id, title, action, url} = values;
  if (url && new URL(url).protocol !== 'https:') throw new Error('元リンクには https を指定してください');
  const source = values.source.toLowerCase();
  const rows = (await readFile(path.join(root, 'workspace.tsv'), 'utf8')).split(/\r?\n/);
  const projects = rows.filter(r => r.startsWith('P\t')).map(r => r.slice(2).replaceAll('%25', '%'));
  const project = await selectProject(values, projects);
  // Keep the server's original import namespace across changes to routing rules.
  const id = createHash('sha256').update(JSON.stringify([projectKey(projectArg), source, scope, message_id])).digest('hex');
  const filename = 'message-' + id + '.queue';
  if (rows.includes('QI\t' + filename)) return {status: 'processed', id};
  // KAGARI also deduplicates by title. Distinguish messages sharing a subject.
  const taskTitle = title + ' [' + id.slice(0, 12) + ']';
  const next = action + ' ｜ 出典 ' + [source, scope, message_id, url].filter(Boolean).join(' ');
  if (taskTitle.length > 300 || next.length > 12000) throw new Error('出典を含む題名は300字、依頼内容は12000字以内です');
  const inbox = path.join(root, 'queue-inbox');
  await mkdir(inbox, {recursive: true});
  const tmp = path.join(inbox, randomUUID() + '.tmp');
  try {
    await writeFile(tmp, [project, 'codex', taskTitle + '\t' + next + '\t'].join('\n'), {flag: 'wx'});
    try {
      await link(tmp, path.join(inbox, filename));
    } catch (err) {
      if (err.code !== 'EEXIST') throw err;
      return {status: 'already_queued', id};
    }
  } finally {
    await unlink(tmp).catch(err => { if (err.code !== 'ENOENT') throw err; });
  }
  return {status: 'queued', id, project};
}

function error(id, code, message) {
  return {jsonrpc: '2.0', id, error: {code, message}};
}
async function handle(msg) {
  if (msg?.jsonrpc !== '2.0' || typeof msg.method !== 'string') return error(null, -32600, 'Invalid request');
  if (!Object.hasOwn(msg, 'id')) return;
  let result;
  switch (msg.method) {
    case 'initialize':
      result = {protocolVersion: '2025-06-18', capabilities: {tools: {}}, serverInfo: {name: 'kagari-import', version: '1.0.0'}};
      break;
    case 'ping': result = {}; break;
    case 'tools/list': result = {tools: [tool]}; break;
    case 'tools/call':
      if (msg.params?.name !== tool.name) return error(msg.id, -32602, 'Unknown tool');
      try {
        result = {content: [{type: 'text', text: JSON.stringify(await enqueue(msg.params.arguments))}]};
      } catch (err) {
        result = {isError: true, content: [{type: 'text', text: err.message}]};
      }
      break;
    default: return error(msg.id, -32601, 'Method not found');
  }
  return {jsonrpc: '2.0', id: msg.id, result};
}

for await (const line of createInterface({input: process.stdin, crlfDelay: Infinity})) {
  let response;
  try { response = await handle(JSON.parse(line)); }
  catch { response = error(null, -32700, 'Parse error'); }
  if (response) process.stdout.write(JSON.stringify(response) + '\n');
}
