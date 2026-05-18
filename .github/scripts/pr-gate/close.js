// Close-action for the PR Gate workflow.
//
// Reads a markdown template from the base ref of the repo, substitutes
// {{placeholder}} variables, posts the rendered message as a comment,
// labels the PR `need-triage`, then closes the PR.
//
// Templates live under .github/pr-gate-messages/ and the template name
// equals the rejection reason emitted by evaluate.js — falls back to
// `default.md` for unknown reasons.

const FALLBACK_TEMPLATE = 'Hi @{{prAuthor}}, this PR was closed because it does not meet our contribution requirements. See CONTRIBUTING.md.';

const KNOWN_TEMPLATES = new Set([
  'denounced',
  'no_issue_reference',
  'not_assigned_to_issue',
  'issue_not_found',
]);

function renderTemplate(template, vars) {
  return Object.entries(vars).reduce(
    (acc, [k, v]) => acc.replaceAll(`{{${k}}}`, v),
    template,
  );
}

function pickTemplateName(reason) {
  return KNOWN_TEMPLATES.has(reason) ? reason : 'default';
}

function buildVars({ prAuthor, issueNumber, owner, repo, denounceReason }) {
  return {
    prAuthor,
    issueNumber,
    owner,
    repo,
    reasonBlock: denounceReason ? ` Reason: ${denounceReason}.` : '',
  };
}

async function close({ github, context, core, inputs }) {
  const { reason, issueNumber, denounceReason } = inputs;
  const prAuthor = context.payload.pull_request.user.login;
  const { owner, repo } = context.repo;

  const templateName = pickTemplateName(reason);

  let template;
  try {
    const response = await github.rest.repos.getContent({
      owner,
      repo,
      path: `.github/pr-gate-messages/${templateName}.md`,
      ref: context.payload.pull_request.base.ref,
    });
    template = Buffer.from(response.data.content, 'base64').toString('utf-8');
  } catch (error) {
    core.info(`Could not load template ${templateName}.md: ${error.message}`);
    template = FALLBACK_TEMPLATE;
  }

  const message = renderTemplate(
    template,
    buildVars({ prAuthor, issueNumber, owner, repo, denounceReason }),
  );

  await github.rest.issues.createComment({
    owner,
    repo,
    issue_number: context.payload.pull_request.number,
    body: message,
  });

  try {
    await github.rest.issues.addLabels({
      owner,
      repo,
      issue_number: context.payload.pull_request.number,
      labels: ['need-triage'],
    });
  } catch (error) {
    core.info(`Could not add label (may not exist): ${error.message}`);
  }

  await github.rest.pulls.update({
    owner,
    repo,
    pull_number: context.payload.pull_request.number,
    state: 'closed',
  });

  core.info('PR closed successfully');
}

module.exports = { close, renderTemplate, pickTemplateName, buildVars, KNOWN_TEMPLATES };
