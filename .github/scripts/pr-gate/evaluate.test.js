const test = require('node:test');
const assert = require('node:assert/strict');

const {
  evaluate,
  parseList,
  parseCodeowners,
  collectTaggedUsers,
} = require('./evaluate');

// Build a fake { github, context, core } trio plus captured outputs.
// Defaults give a not-bot non-trusted user with no issue reference, which
// the evaluator will reject. Override anything per-test.
function makeMocks(overrides = {}) {
  const {
    prAuthor = 'someone',
    prBody = '',
    files = {},
    isCollaborator = false,
    isOrgMember = false,
    issueData = null,
    issueFetchError = null,
    comments = [],
  } = overrides;

  const outputs = {};
  const logs = [];
  const core = {
    setOutput: (k, v) => {
      outputs[k] = String(v);
    },
    info: (m) => logs.push(m),
  };

  const listComments = async () => comments;

  const github = {
    rest: {
      repos: {
        getContent: async ({ path }) => {
          if (!(path in files) || files[path] == null) {
            const err = new Error(`Not found: ${path}`);
            err.status = 404;
            throw err;
          }
          return {
            data: { content: Buffer.from(files[path]).toString('base64') },
          };
        },
        checkCollaborator: async () => {
          if (!isCollaborator) {
            const err = new Error('not a collaborator');
            err.status = 404;
            throw err;
          }
        },
      },
      orgs: {
        checkMembershipForUser: async () => {
          if (!isOrgMember) {
            const err = new Error('not a member');
            err.status = 404;
            throw err;
          }
        },
      },
      issues: {
        get: async () => {
          if (issueFetchError) throw issueFetchError;
          if (!issueData) {
            const err = new Error('issue not found');
            err.status = 404;
            throw err;
          }
          return { data: issueData };
        },
        listComments,
      },
    },
    paginate: async (method) => {
      if (method === listComments) return comments;
      return [];
    },
  };

  const context = {
    repo: { owner: 'circlefin', repo: 'malachite' },
    payload: {
      pull_request: {
        user: { login: prAuthor },
        body: prBody,
        base: { ref: 'main' },
        number: 999,
      },
    },
  };

  return { github, context, core, outputs, logs };
}

// ---------------------------------------------------------------------------
// parseList
// ---------------------------------------------------------------------------

test('parseList: empty content yields empty map', () => {
  assert.equal(parseList('').size, 0);
  assert.equal(parseList(null).size, 0);
  assert.equal(parseList(undefined).size, 0);
});

test('parseList: skips blank lines and # comments', () => {
  const m = parseList('# comment\n\n  \nalice\n# inline-looking\n');
  assert.deepEqual([...m.keys()], ['alice']);
});

test('parseList: usernames are lowercased', () => {
  const m = parseList('Alice\nBOB');
  assert.deepEqual([...m.keys()], ['alice', 'bob']);
});

test('parseList: optional `: reason` captured and trimmed', () => {
  const m = parseList('alice: opens AI slop\nbob');
  assert.equal(m.get('alice'), 'opens AI slop');
  assert.equal(m.get('bob'), '');
});

test('parseList: reason with colon character is preserved', () => {
  const m = parseList('alice: link: https://example.com');
  assert.equal(m.get('alice'), 'link: https://example.com');
});

// ---------------------------------------------------------------------------
// parseCodeowners
// ---------------------------------------------------------------------------

test('parseCodeowners: extracts @username handles, lowercased', () => {
  assert.deepEqual(parseCodeowners('* @Alice @Bob'), ['alice', 'bob']);
});

test('parseCodeowners: skips @org/team handles (not truncated to @org)', () => {
  assert.deepEqual(parseCodeowners('* @circlefin/team @alice'), ['alice']);
});

test('parseCodeowners: trailing hyphens are stripped', () => {
  assert.deepEqual(parseCodeowners('* @alice- @bob'), ['alice', 'bob']);
});

test('parseCodeowners: empty content yields empty array', () => {
  assert.deepEqual(parseCodeowners(''), []);
  assert.deepEqual(parseCodeowners(null), []);
});

// ---------------------------------------------------------------------------
// collectTaggedUsers
// ---------------------------------------------------------------------------

function comment(login, body, url = 'https://example.com/c') {
  return { user: { login }, body, html_url: url };
}

test('collectTaggedUsers: ignores comments from non-codeowners', () => {
  const tagged = collectTaggedUsers(
    [comment('outsider', '/assign @alice')],
    ['codeowner1'],
  );
  assert.equal(tagged.size, 0);
});

test('collectTaggedUsers: picks up single /assign from codeowner', () => {
  const tagged = collectTaggedUsers(
    [comment('codeowner1', '/assign @alice')],
    ['codeowner1'],
  );
  assert.deepEqual([...tagged], ['alice']);
});

test('collectTaggedUsers: multiple users per /assign command', () => {
  const tagged = collectTaggedUsers(
    [comment('codeowner1', '/assign @alice @bob')],
    ['codeowner1'],
  );
  assert.deepEqual([...tagged].sort(), ['alice', 'bob']);
});

test('collectTaggedUsers: composes across multiple codeowner comments', () => {
  // Reproduces the original bug: codeowner A's later /assign @alice should
  // NOT invalidate codeowner B's earlier /assign @bob.
  const tagged = collectTaggedUsers(
    [
      comment('codeowner1', '/assign @bob'),
      comment('codeowner2', '/assign @alice'),
    ],
    ['codeowner1', 'codeowner2'],
  );
  assert.deepEqual([...tagged].sort(), ['alice', 'bob']);
});

test('collectTaggedUsers: multiple /assign lines in one comment both register', () => {
  const tagged = collectTaggedUsers(
    [comment('codeowner1', '/assign @alice\n\nLater: /assign @bob')],
    ['codeowner1'],
  );
  assert.deepEqual([...tagged].sort(), ['alice', 'bob']);
});

// ---------------------------------------------------------------------------
// evaluate (full decision tree)
// ---------------------------------------------------------------------------

test('evaluate: denounced user → close denounced (with reason)', async () => {
  const m = makeMocks({
    prAuthor: 'baduser',
    files: { '.github/DENOUNCED': 'baduser: spammer' },
  });
  await evaluate(m);
  assert.equal(m.outputs.status, 'close');
  assert.equal(m.outputs.reason, 'denounced');
  assert.equal(m.outputs.denounce_reason, 'spammer');
});

test('evaluate: denounce overrides even codeowner status', async () => {
  // If you somehow end up in both files, denounce wins.
  const m = makeMocks({
    prAuthor: 'alice',
    files: {
      '.github/DENOUNCED': 'alice',
      '.github/CODEOWNERS': '* @alice',
    },
  });
  await evaluate(m);
  assert.equal(m.outputs.status, 'close');
  assert.equal(m.outputs.reason, 'denounced');
});

test('evaluate: allowed bot → allow bot', async () => {
  const m = makeMocks({ prAuthor: 'dependabot[bot]' });
  await evaluate(m);
  assert.equal(m.outputs.status, 'allow');
  assert.equal(m.outputs.reason, 'bot');
});

test('evaluate: codeowner → allow codeowner', async () => {
  const m = makeMocks({
    prAuthor: 'alice',
    files: { '.github/CODEOWNERS': '* @alice @bob' },
  });
  await evaluate(m);
  assert.equal(m.outputs.status, 'allow');
  assert.equal(m.outputs.reason, 'codeowner');
});

test('evaluate: collaborator → allow collaborator', async () => {
  const m = makeMocks({ prAuthor: 'outside-helper', isCollaborator: true });
  await evaluate(m);
  assert.equal(m.outputs.status, 'allow');
  assert.equal(m.outputs.reason, 'collaborator');
});

test('evaluate: org member → allow org_member', async () => {
  const m = makeMocks({ prAuthor: 'circle-employee', isOrgMember: true });
  await evaluate(m);
  assert.equal(m.outputs.status, 'allow');
  assert.equal(m.outputs.reason, 'org_member');
});

test('evaluate: vouched user → allow vouched', async () => {
  const m = makeMocks({
    prAuthor: 'trusted-contrib',
    files: { '.github/VOUCHED': 'trusted-contrib' },
  });
  await evaluate(m);
  assert.equal(m.outputs.status, 'allow');
  assert.equal(m.outputs.reason, 'vouched');
});

test('evaluate: no issue ref in body → close no_issue_reference', async () => {
  const m = makeMocks({ prAuthor: 'someone', prBody: 'just a PR' });
  await evaluate(m);
  assert.equal(m.outputs.status, 'close');
  assert.equal(m.outputs.reason, 'no_issue_reference');
});

test('evaluate: issue fetch fails → close issue_not_found', async () => {
  const m = makeMocks({
    prAuthor: 'someone',
    prBody: 'Closes: #42',
    issueData: null, // makeMocks throws 404 when null
  });
  await evaluate(m);
  assert.equal(m.outputs.status, 'close');
  assert.equal(m.outputs.reason, 'issue_not_found');
  assert.equal(m.outputs.issue_number, '42');
});

test('evaluate: PR author assigned to referenced issue → allow assigned', async () => {
  const m = makeMocks({
    prAuthor: 'someone',
    prBody: 'Closes #42',
    issueData: { assignees: [{ login: 'someone' }] },
  });
  await evaluate(m);
  assert.equal(m.outputs.status, 'allow');
  assert.equal(m.outputs.reason, 'assigned');
  assert.equal(m.outputs.issue_number, '42');
});

test('evaluate: codeowner /assign-tagged → allow tagged_by_codeowner', async () => {
  const m = makeMocks({
    prAuthor: 'newcontrib',
    prBody: 'Fixes: #42',
    files: { '.github/CODEOWNERS': '* @maintainer' },
    issueData: { assignees: [] },
    comments: [
      { user: { login: 'maintainer' }, body: '/assign @newcontrib', html_url: 'x' },
    ],
  });
  await evaluate(m);
  assert.equal(m.outputs.status, 'allow');
  assert.equal(m.outputs.reason, 'tagged_by_codeowner');
});

test('evaluate: no trust signal at all → close not_assigned_to_issue', async () => {
  const m = makeMocks({
    prAuthor: 'random',
    prBody: 'closes #42',
    files: { '.github/CODEOWNERS': '* @maintainer' },
    issueData: { assignees: [{ login: 'someone-else' }] },
    comments: [],
  });
  await evaluate(m);
  assert.equal(m.outputs.status, 'close');
  assert.equal(m.outputs.reason, 'not_assigned_to_issue');
  assert.equal(m.outputs.issue_number, '42');
});

test('evaluate: username matching is case-insensitive', async () => {
  // PR author is `Alice` but VOUCHED has `alice` lowercase.
  const m = makeMocks({
    prAuthor: 'Alice',
    files: { '.github/VOUCHED': 'alice' },
  });
  await evaluate(m);
  assert.equal(m.outputs.status, 'allow');
  assert.equal(m.outputs.reason, 'vouched');
});
