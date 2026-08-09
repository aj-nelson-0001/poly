# Branch Protection Rules for Poly Repository

## Recommended Branch Protection Rules

Go to https://github.com/aj-nelson-0001/poly/settings/branches and add the following rules:

### Rule 1: master branch protection

**Branch name pattern:** `master`

**Protect matching branches:**
- ✅ Require a pull request before merging
  - ✅ Require approvals (1)
  - ✅ Dismiss stale pull request approvals when new commits are pushed
- ✅ Require status checks to pass before merging
  - ✅ Require branches to be up to date before merging
  - Required status checks:
    - `Test`
    - `Lint`
    - `Transpile Examples`
- ✅ Require conversation resolution before merging
- ✅ Require linear history (optional, for squash merges)
- ✅ Include administrators (optional)
- ✅ Restrict who can push to matching branches (optional)

### Rule 2: main branch protection (if using main)

**Branch name pattern:** `main`

Same rules as master.

## How to Add Branch Protection Rules

1. Go to your repository: https://github.com/aj-nelson-0001/poly
2. Click **Settings** (tab)
3. In the left sidebar, click **Branches**
4. Under "Branch protection rules", click **Add rule**
5. Fill in the settings as described above
6. Click **Create** or **Save changes**

## Benefits

- Prevents direct pushes to master
- Ensures all changes go through pull requests
- Requires CI to pass before merging
- Requires code review before merging
- Keeps the main branch stable and production-ready
