#!/usr/bin/env node
/**
 * Push a branch and create PR via GitHub API (bypasses git network issues)
 * Works when github.com is unreachable but api.github.com is reachable
 */
const https = require('https')
const fs = require('fs')
const path = require('path')
const { execSync } = require('child_process')

const TOKEN = new URL(fs.readFileSync(process.env.HOME + '/.git-credentials', 'utf8').trim()).password
const OWNER = 'jadelaglace'
const REPO = 'Compass'
const BRANCH = process.argv[2] // e.g. feat/130-force-graph
const COMMIT_MSG = process.argv[3]
const PR_TITLE = process.argv[4]
const PR_BODY = process.argv[5] || ''
const BASE_BRANCH = 'dev'

if (!BRANCH || !COMMIT_MSG || !PR_TITLE) {
  console.error('Usage: node github-push.js <branch> <commit-msg> <pr-title> [pr-body]')
  process.exit(1)
}

const api = (method, path, data) => new Promise((resolve, reject) => {
  const body = data ? JSON.stringify(data) : undefined
  const opts = {
    hostname: 'api.github.com',
    path,
    method,
    headers: {
      'Authorization': `token ${TOKEN}`,
      'Accept': 'application/vnd.github+json',
      'X-GitHub-Api-Format': '2022-11-28',
      'User-Agent': 'compass-push-script',
      ...(body ? { 'Content-Type': 'application/json', 'Content-Length': Buffer.byteLength(body) } : {})
    }
  }
  const req = https.request(opts, res => {
    let d = ''
    res.on('data', c => d += c)
    res.on('end', () => {
      try { resolve(JSON.parse(d)) }
      catch { resolve(d) }
    })
  })
  req.on('error', reject)
  if (body) req.write(body)
  req.end()
})

async function getLocalRefSha(ref) {
  const refPath = path.join(process.cwd(), '.git', 'refs', 'remotes', 'origin', ref)
  if (fs.existsSync(refPath)) {
    return fs.readFileSync(refPath, 'utf8').trim()
  }
  // Try packed-refs
  const packedRefs = fs.readFileSync(path.join(process.cwd(), '.git', 'packed-refs'), 'utf8')
  const match = packedRefs.match(new RegExp(`^([a-f0-9]+) refs/remotes/origin/${ref}$`, 'm'))
  if (match) return match[1]
  // Fallback: git rev-parse
  return execSync(`git rev-parse origin/${ref}`, { cwd: process.cwd() }).toString().trim()
}

async function getFileBlobs(files) {
  const blobs = {}
  for (const file of files) {
    const content = fs.readFileSync(file)
    const b64 = content.toString('base64')
    // GitHub API rejects base64 that includes newlines in the payload
    const clean = b64.replace(/\n/g, '')
    const res = await api('POST', `/repos/${OWNER}/${REPO}/git/blobs`, {
      content: content.toString('utf8'),
      encoding: 'utf-8'
    })
    if (res.sha) {
      blobs[file] = res.sha
      console.log(`  Blob created: ${file} (${res.sha.substring(0,7)})`)
    } else {
      console.error(`  Blob failed: ${file}`, res)
    }
  }
  return blobs
}

async function run() {
  console.log(`\nPushing branch: ${BRANCH}`)
  console.log(`Commit: ${COMMIT_MSG}\n`)

  // Get parent commit SHA from local origin/dev ref
  const baseSha = await getLocalRefSha(BASE_BRANCH)
  console.log(`Base ${BASE_BRANCH} SHA: ${baseSha.substring(0,8)}`)

  // Get the commit SHA of our branch tip
  const headSha = execSync(`git rev-parse ${BRANCH}`, { cwd: process.cwd() }).toString().trim()
  console.log(`Branch HEAD SHA: ${headSha.substring(0,8)}\n`)

  // Get the list of files changed using GitHub compare API
  const compare = await api('GET', `/repos/${OWNER}/${REPO}/compare/${baseSha}...${headSha}`)
  const diffFiles = compare.files ? compare.files.map(f => f.filename) : []
  // Fallback: scan working directory for known changed files
  if (diffFiles.length === 0) {
    const knownFiles = [
      'compass-vue3/src/views/insights/InsightsView.vue',
      'compass-vue3/src/components/insights/InsightCard.vue',
      'compass-vue3/src/components/insights/InsightForm.vue',
      'compass-vue3/src/stores/insights.ts',
    ].filter(f => fs.existsSync(f))
    diffFiles.push(...knownFiles)
  }

  console.log('Changed files:')
  diffFiles.forEach(f => console.log(`  ${f}`))

  // For each file, create a blob
  console.log('\nCreating blobs...')
  const blobs = {}
  for (const file of diffFiles) {
    if (!fs.existsSync(file)) { console.log(`  SKIP (not found): ${file}`); continue }
    try {
      const content = fs.readFileSync(file)
      const res = await api('POST', `/repos/${OWNER}/${REPO}/git/blobs`, {
        content: content.toString('utf8'),
        encoding: 'utf-8'
      })
      if (res.sha) {
        blobs[file] = res.sha
        console.log(`  ✓ ${file} → ${res.sha.substring(0,7)}`)
      } else {
        console.log(`  ✗ ${file}:`, JSON.stringify(res).substring(0, 200))
      }
    } catch(e) { console.log(`  ✗ ${file}: ${e.message}`) }
  }

  // Create tree
  const treeItems = await Promise.all(diffFiles.map(async file => {
    if (!blobs[file] || !fs.existsSync(file)) return null
    // Get the mode from existing file or default to 100644
    const mode = file.endsWith('.sh') ? '100755' : '100644'
    return { path: file, mode, type: 'blob', sha: blobs[file] }
  }))
  const tree = await api('POST', `/repos/${OWNER}/${REPO}/git/trees`, {
    base_tree: baseSha,
    tree: treeItems.filter(Boolean)
  })
  console.log(`\nTree created: ${tree.sha}`)

  // Create commit
  const commit = await api('POST', `/repos/${OWNER}/${REPO}/git/commits`, {
    message: COMMIT_MSG,
    tree: tree.sha,
    parents: [baseSha]
  })
  console.log(`Commit created: ${commit.sha}`)

  // Create branch ref
  await api('POST', `/repos/${OWNER}/${REPO}/git/refs`, {
    ref: `refs/heads/${BRANCH}`,
    sha: commit.sha
  })
  console.log(`Branch created: refs/heads/${BRANCH}`)

  // Create PR
  const pr = await api('POST', `/repos/${OWNER}/${REPO}/pulls`, {
    title: PR_TITLE,
    head: BRANCH,
    base: BASE_BRANCH,
    body: PR_BODY
  })
  console.log(`\n✅ PR #${pr.number}: ${pr.html_url}`)

  // Auto-merge (squash)
  const merged = await api('PUT', `/repos/${OWNER}/${REPO}/pulls/${pr.number}/merge`, {
    merge_method: 'squash',
    commit_title: COMMIT_MSG
  })
  console.log(`Merge: ${merged.merged ? '✅ MERGED' : '⬜ ' + JSON.stringify(merged)}`)
}

run().catch(e => { console.error(e); process.exit(1) })
