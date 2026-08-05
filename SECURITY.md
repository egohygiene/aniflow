# Security Policy

## Reporting

Report suspected vulnerabilities privately through GitHub Security Advisories.
Do not open a public issue for an unpatched vulnerability.

## Execution model

Pipeline files can name external executables and arguments. Treat an untrusted
pipeline as executable code. Review it before running it.

`aniflow` invokes configured programs directly rather than through a shell, but
the selected executable still receives the current user's permissions.

