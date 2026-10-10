# GitHub community files

These files belong in `.github/` (`ISSUE_TEMPLATE/` and `pull_request_template.md`). They live here because the
packaging lane does not own `.github/`. Copy them with:

    mkdir -p .github && cp -r packaging/github/ISSUE_TEMPLATE .github/ && cp packaging/github/pull_request_template.md .github/

`scripts/check-github-templates.sh` checks that the forms parse and ask for the required fields. When the files
are in `.github/`, run it with `.github` as the argument.
