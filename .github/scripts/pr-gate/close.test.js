const test = require('node:test');
const assert = require('node:assert/strict');

const {
  renderTemplate,
  pickTemplateName,
  buildVars,
  KNOWN_TEMPLATES,
} = require('./close');

test('renderTemplate: substitutes single placeholder', () => {
  assert.equal(
    renderTemplate('Hi @{{prAuthor}}!', { prAuthor: 'alice' }),
    'Hi @alice!',
  );
});

test('renderTemplate: substitutes multiple placeholders, repeated', () => {
  const out = renderTemplate(
    '{{owner}}/{{repo}} — see {{owner}}/{{repo}}/blob/main',
    { owner: 'circlefin', repo: 'malachite' },
  );
  assert.equal(out, 'circlefin/malachite — see circlefin/malachite/blob/main');
});

test('renderTemplate: missing placeholder is left literal', () => {
  // We don't pre-validate — placeholders in templates that aren't in vars
  // pass through, which is intentional so non-overlapping templates can
  // share a single vars object.
  assert.equal(
    renderTemplate('Hi @{{prAuthor}}, issue #{{issueNumber}}', {
      prAuthor: 'alice',
    }),
    'Hi @alice, issue #{{issueNumber}}',
  );
});

test('pickTemplateName: known reason returned verbatim', () => {
  for (const reason of KNOWN_TEMPLATES) {
    assert.equal(pickTemplateName(reason), reason);
  }
});

test('pickTemplateName: unknown reason falls back to default', () => {
  assert.equal(pickTemplateName('something-else'), 'default');
  assert.equal(pickTemplateName(''), 'default');
  assert.equal(pickTemplateName(undefined), 'default');
});

test('buildVars: includes reasonBlock when denounceReason provided', () => {
  const v = buildVars({
    prAuthor: 'a',
    issueNumber: '1',
    owner: 'o',
    repo: 'r',
    denounceReason: 'spammer',
  });
  assert.equal(v.reasonBlock, ' Reason: spammer.');
});

test('buildVars: omits reasonBlock when no denounceReason', () => {
  const v = buildVars({
    prAuthor: 'a',
    issueNumber: '1',
    owner: 'o',
    repo: 'r',
    denounceReason: '',
  });
  assert.equal(v.reasonBlock, '');
});
