# Malachite Terraform – local environment notes

This document gives a short overview of how to approach the Terraform
configuration in this directory when working on Malachite infrastructure.

## 1. Layout

Typical elements you may find here:

- `modules/` – reusable building blocks used by multiple environments.
- `envs/` or `workspaces/` – concrete environments (for example `dev`, `staging`, `prod`).
- `variables.tf` / `terraform.tfvars` – input variables and their default values.

Check the actual files in this directory to confirm the current layout.

## 2. Safe local workflow

When experimenting with Terraform changes:

1. Select or create a dedicated development workspace.
2. Run `terraform fmt` to keep files formatted.
3. Use `terraform validate` to catch obvious issues before planning.
4. Use `terraform plan` to review changes with your team before applying them.

## 3. Contribution tips

- Avoid committing real secrets; use variables and examples instead.
- Add comments near non-obvious resources explaining why they are needed.
- When adding new variables, update any example `tfvars` files to keep them in sync.
