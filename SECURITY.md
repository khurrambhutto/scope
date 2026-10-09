# Security Policy

## Supported versions

Security fixes go into the latest release. Please upgrade to the most recent version before reporting an issue.

| Version | Supported |
| ------- | --------- |
| 0.3.x   | Yes       |
| < 0.3   | No        |

## Reporting a vulnerability

Please do not report security vulnerabilities in public issues or pull requests.

Use GitHub's private vulnerability reporting: open the repository's **Security** tab and choose **Report a vulnerability**. If that is unavailable, email the maintainer at khurrambhutto071@gmail.com with the subject line `Scope security`.

Include:

- A description of the issue and its impact
- Steps to reproduce, or a proof of concept
- The affected version and your distribution

You can expect an acknowledgement within a few days. We will keep you informed while we investigate and will credit you in the release notes if you wish.

## Scope of concern

Scope runs privileged commands through `pkexec` and enforces a deny-list for protected packages. Reports involving privilege escalation, bypassing the deny-list or preview/apply revalidation, or running unvalidated commands are especially welcome.
