# Developer Certificate of Origin

Causari accepts contributions under the Developer Certificate of Origin,
version 1.1, the same text used by the Linux kernel and by Git. There is no
contributor license agreement to sign and nothing to register: you certify
the statement below by adding a `Signed-off-by` line to each commit.

```
git commit -s
```

produces a trailer such as

```
Signed-off-by: Ada Lovelace <ada@example.org>
```

The name and address must be yours (a name you are known by; pseudonyms are
fine, empty or obviously fake identities are not). CI checks that every
commit in a pull request carries the trailer.

Your contribution is licensed to the project and to everyone who receives
it under the [Apache License 2.0](LICENSE), the same terms as the rest of
Causari. Nobody, including the maintainers, receives a broader license from
you than that.

---

```
Developer Certificate of Origin
Version 1.1

Copyright (C) 2004, 2006 The Linux Foundation and its contributors.

Everyone is permitted to copy and distribute verbatim copies of this
license document, but changing it is not allowed.


Developer's Certificate of Origin 1.1

By making a contribution to this project, I certify that:

(a) The contribution was created in whole or in part by me and I
    have the right to submit it under the open source license
    indicated in the file; or

(b) The contribution is based upon previous work that, to the best
    of my knowledge, is covered under an appropriate open source
    license and I have the right under that license to submit that
    work with modifications, whether created in whole or in part
    by me, under the same open source license (unless I am
    permitted to submit under a different license), as indicated
    in the file; or

(c) The contribution was provided directly to me by some other
    person who certified (a), (b) or (c) and I have not modified
    it.

(d) I understand and agree that this project and the contribution
    are public and that a record of the contribution (including all
    personal information I submit with it, including my sign-off) is
    maintained indefinitely and may be redistributed consistent with
    this project or the open source license(s) involved.
```

## Why a DCO and not a CLA

Until 2026-09-20 Causari asked contributors to sign a CLA that let the
maintainers relicense contributions, including under proprietary terms.
That is the wrong shape for a project whose point is that anyone can verify
what it says. Contributions made under the CLA keep the terms they were
made under; everything from this date on is DCO and Apache 2.0, full stop.

## Automated commits

Commits authored by dependency and report bots (`dependabot[bot]`,
`renovate[bot]`, `github-actions[bot]`, `causari-report[bot]`) are not
contributions under this certificate: a bot cannot certify origin, and the
change is a version string or generated data. The `dco` check skips them; a
maintainer reviews and merges them like any other change.
