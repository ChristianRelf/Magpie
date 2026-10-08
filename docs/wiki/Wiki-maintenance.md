# Wiki maintenance

The canonical handbook source lives in `docs/wiki` in the main repository. GitHub serves a separate wiki Git repository. Edit the checked-in source, validate it, then publish a rendered copy so source reviews and the wiki stay aligned.

## Files and navigation

Each top-level Markdown file becomes one wiki page. Filenames use simple words separated by hyphens. `Home.md` is the landing page, `_Sidebar.md` supplies persistent navigation, and `_Footer.md` supplies the footer. Add every content page to Home and the sidebar.

Source links use ordinary relative `.md` paths so they work in the repository and local readers. The publisher rewrites those links into GitHub wiki URLs, preserving heading fragments. It leaves code examples unchanged. Links to source code and official references remain normal external links.

For GitHub's wiki rules, see [adding/editing wiki pages](https://docs.github.com/en/communities/documenting-your-project-with-wikis/adding-or-editing-wiki-pages) and [sidebar/footer files](https://docs.github.com/en/communities/documenting-your-project-with-wikis/creating-a-footer-or-sidebar-for-your-wiki).

## Check before publishing

From the repository root, with Python 3.10+:

```bash
python3 scripts/wiki.py check
pnpm exec prettier --check README.md 'docs/wiki/*.md'
git diff --check
```

The checker verifies README/wiki local link targets, heading anchors, Home/sidebar coverage, closed code fences, JSON examples, and Bash syntax when Bash is installed. It does not execute the shell examples, call a model, validate a live provider account, typecheck every illustrative TypeScript snippet, or guarantee that an external website is reachable.

If a source/API shape changed, also check the examples against that source or an isolated fixture. A link checker cannot catch a plausible but wrong request field. Keep release-version boundaries explicit and update the corresponding troubleshooting page when behaviour changes.

## Render a local preview

```bash
python3 scripts/wiki.py stage --output /tmp/magpie-wiki-preview
```

Choose a new or empty directory outside the repository. On Windows choose an equivalent absolute temporary path. Staging refuses to overwrite a nonempty directory. Inspect the generated files in a Markdown viewer or Git diff. The 25 content pages plus sidebar/footer are rendered; source files remain unchanged.

## Prepare GitHub once

Enable Wikis in the repository's settings and create the initial Home page in a signed-in browser. GitHub needs this first page before the separate wiki repository can be cloned. Once initialised, the remote is:

```text
https://github.com/ChristianRelf/Magpie.wiki.git
```

Use your existing Git credential helper/`gh` authentication. Do not put a personal access token in a remote URL, shell history, source file, or script argument. Git author name/email must be configured for the source repository before creating a wiki commit.

## Review and publish

First inspect the proposed update without publishing:

```bash
python3 scripts/wiki.py publish
```

The command validates, clones the wiki into a temporary directory, detects its default branch, overlays the managed pages, and prints the change summary. Without `--push`, it makes no remote change.

When ready:

```bash
python3 scripts/wiki.py publish --push
```

This commits only the managed Markdown pages, performs a normal non-forced push to the discovered wiki branch, and checks that its remote head matches the committed result. It does not commit/push the main repository, modify application code, or delete unrelated wiki pages. If someone pushes between cloning and publishing, a conflicting non-fast-forward update fails rather than overwriting their history; review and retry.

For a fork, the repository can be changed explicitly:

```bash
python3 scripts/wiki.py --repo YOUR_OWNER/YOUR_REPOSITORY publish
```

Review external repository/source links when adapting a fork; the script rewrites local navigation, not every mention of the original project.

## Preserve edits made in GitHub

If someone edits a managed page directly on GitHub, reconcile that content into `docs/wiki` before the next publish. The source overlay intentionally replaces managed page content with the reviewed source version. Unmanaged pages are preserved, so page removals/renames need a separate deliberate wiki cleanup after incoming links are updated.

Run the checker and publisher again; unchanged content should produce “already matches” without another commit. Review the live Home page, sidebar links, code fences, tables, and diagrams after a substantial update.

## Documentation review checklist

- Instructions match actual flags, request fields, scopes, defaults, and endpoint paths.
- Published versus unreleased features are labelled.
- Non-inference checks are separated from requests that spend provider allowance.
- Unknown quotas/prices remain distinct from reported values.
- Troubleshooting includes a verification step and does not begin with data deletion.
- Examples use placeholders/private prompts instead of real credentials.
- Navigation and anchors work in both the source and rendered wiki.
- Test/evidence claims describe what actually ran.

Related: [Fixing issues](Fixing-issues.md), [Development and testing](Development-and-testing.md), [Release maintenance](Release-maintenance.md).
