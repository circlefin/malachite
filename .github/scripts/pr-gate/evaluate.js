// Eligibility evaluator for the PR Gate workflow.
//
// Runs inside actions/github-script. Reads every contributor-trust input
// from the base ref via the REST API so a malicious PR can't substitute
// content; the local checkout is only used to locate this script file.
//
// Emits two outputs:
//   - status: 'allow' | 'close'
//   - reason: bot | codeowner | collaborator | org_member | vouched |
//             assigned | tagged_by_codeowner       (when status=allow)
//             denounced | no_issue_reference |
//             not_assigned_to_issue | issue_not_found  (when status=close)
//
// May also emit:
//   - denounce_reason: free-text from the DENOUNCED file
//   - issue_number:    stringified issue number when relevant

const ALLOWED_BOTS = new Set(['dependabot[bot]', 'stepsecurity-app[bot]']);
const ORG = 'circlefin';
const ISSUE_PREFIXES = ['closes', 'fixes', 'fix', 'close', 'resolve', 'resolves'];
const ISSUE_REF_REGEX = new RegExp(`(?:${ISSUE_PREFIXES.join('|')}):?\\s*#(\\d+)`, 'i');
// Match @username; negative lookahead skips @org/team handles so they
// aren't truncated to just @org. \b prevents matches ending in a hyphen.
const CODEOWNER_HANDLE_REGEX = /@[\w-]+(?!\/)\b/g;
const ASSIGN_PATTERN = /\/assign((?:\s+@[\w-]+)+)/gi;
const USER_PATTERN = /@([\w-]+)/g;

// Parse one-username-per-line list with `#` comments and optional `: reason`.
// Returns Map<usernameLower, reasonString>.
function parseList(content) {
  const map = new Map();
  if (!content) return map;
  for (const rawLine of content.split('\n')) {
    const line = rawLine.trim();
    if (!line || line.startsWith('#')) continue;
    const colonIdx = line.indexOf(':');
    const name = (colonIdx === -1 ? line : line.slice(0, colonIdx)).trim().toLowerCase();
    const reason = colonIdx === -1 ? '' : line.slice(colonIdx + 1).trim();
    if (name) map.set(name, reason);
  }
  return map;
}

// Extract individual codeowner handles from a CODEOWNERS file body.
// Team handles (@org/team) are skipped — they'd need separate expansion.
function parseCodeowners(content) {
  if (!content) return [];
  return (content.match(CODEOWNER_HANDLE_REGEX) || []).map((c) => c.substring(1).toLowerCase());
}

// Collect every @user that any codeowner has /assign'd anywhere in the
// thread. Union across comments (A's /assign @alice does not invalidate
// B's earlier /assign @bob) and within each comment (`/assign @alice @bob`
// and multiple /assign lines both contribute).
function collectTaggedUsers(comments, codeowners, log = () => {}) {
  const codeownerSet = new Set(codeowners);
  const tagged = new Set();
  for (const comment of comments) {
    const author = comment.user.login.toLowerCase();
    if (!codeownerSet.has(author)) continue;
    for (const assignMatch of comment.body.matchAll(ASSIGN_PATTERN)) {
      for (const userMatch of assignMatch[1].matchAll(USER_PATTERN)) {
        tagged.add(userMatch[1].toLowerCase());
        log(`/assign @${userMatch[1]} from codeowner @${comment.user.login} (${comment.html_url})`);
      }
    }
  }
  return tagged;
}

async function evaluate({ github, context, core }) {
  const prAuthor = context.payload.pull_request.user.login;
  const prAuthorLower = prAuthor.toLowerCase();
  const prBody = context.payload.pull_request.body || '';
  const { owner, repo } = context.repo;
  const baseRef = context.payload.pull_request.base.ref;

  async function readBaseFile(path) {
    try {
      const response = await github.rest.repos.getContent({ owner, repo, path, ref: baseRef });
      return Buffer.from(response.data.content, 'base64').toString('utf-8');
    } catch (error) {
      core.info(`Could not read ${path} from ${baseRef}: ${error.message}`);
      return null;
    }
  }

  function decide(status, reason, extras = {}) {
    core.info(`Decision: ${status} (${reason})`);
    core.setOutput('status', status);
    core.setOutput('reason', reason);
    for (const [k, v] of Object.entries(extras)) {
      core.setOutput(k, v);
    }
  }

  // 1. Denounce list overrides everything else.
  const denounced = parseList(await readBaseFile('.github/DENOUNCED'));
  if (denounced.has(prAuthorLower)) {
    return decide('close', 'denounced', { denounce_reason: denounced.get(prAuthorLower) });
  }

  // 2. Allowed bots.
  if (ALLOWED_BOTS.has(prAuthor)) {
    return decide('allow', 'bot');
  }

  // 3. Codeowner.
  const codeowners = parseCodeowners(await readBaseFile('.github/CODEOWNERS'));
  if (codeowners.includes(prAuthorLower)) {
    return decide('allow', 'codeowner');
  }

  // 4. Repo collaborator (any access level: read/triage/write/admin).
  try {
    await github.rest.repos.checkCollaborator({ owner, repo, username: prAuthor });
    return decide('allow', 'collaborator');
  } catch (error) {
    core.info(`${prAuthor} is not a collaborator: ${error.status}`);
  }

  // 5. Org member.
  try {
    await github.rest.orgs.checkMembershipForUser({ org: ORG, username: prAuthor });
    return decide('allow', 'org_member');
  } catch (error) {
    core.info(`${prAuthor} is not a member of ${ORG}: ${error.status}`);
  }

  // 6. Vouched list.
  const vouched = parseList(await readBaseFile('.github/VOUCHED'));
  if (vouched.has(prAuthorLower)) {
    return decide('allow', 'vouched');
  }

  // 7. Issue reference + assignment.
  const issueMatch = prBody.match(ISSUE_REF_REGEX);
  if (!issueMatch) {
    return decide('close', 'no_issue_reference');
  }
  const issueNumber = parseInt(issueMatch[1], 10);

  let issue;
  try {
    issue = await github.rest.issues.get({ owner, repo, issue_number: issueNumber });
  } catch (error) {
    core.info(`Could not fetch issue #${issueNumber}: ${error.message}`);
    return decide('close', 'issue_not_found', { issue_number: String(issueNumber) });
  }

  const assignees = issue.data.assignees.map((a) => a.login.toLowerCase());
  if (assignees.includes(prAuthorLower)) {
    return decide('allow', 'assigned', { issue_number: String(issueNumber) });
  }

  // 8. /assign @user from any codeowner.
  if (codeowners.length > 0) {
    try {
      const comments = await github.paginate(github.rest.issues.listComments, {
        owner,
        repo,
        issue_number: issueNumber,
        per_page: 100,
      });
      const tagged = collectTaggedUsers(comments, codeowners, (m) => core.info(m));
      if (tagged.has(prAuthorLower)) {
        return decide('allow', 'tagged_by_codeowner', { issue_number: String(issueNumber) });
      }
      core.info(`No /assign @${prAuthor} from any codeowner. Tagged set: ${[...tagged].join(', ') || '(none)'}`);
    } catch (error) {
      core.info(`Error fetching comments for issue #${issueNumber}: ${error.message}`);
    }
  }

  return decide('close', 'not_assigned_to_issue', { issue_number: String(issueNumber) });
}

module.exports = { evaluate, parseList, parseCodeowners, collectTaggedUsers };
