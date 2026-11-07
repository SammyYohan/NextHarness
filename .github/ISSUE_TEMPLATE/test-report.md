---
name: Test report
about: Tell us what Orion did on your machine
title: "[test] "
labels: test-report
---

Thanks for testing. A report is useful even when everything worked: the
catalogue's `verified` column can only improve with real machines.

## What happened

<!-- one or two sentences: which harness, what you expected, what happened -->

## Machine

- OS and version:
- Shell / terminal:
- Orion version (`orion --version`):

## Doctor report

Run this and paste the result **as is**. It contains no account name, no path
with your user name and no credential:

```sh
orion doctor --report
```

```markdown
<paste here>
```

## Run output

If a run misbehaved, paste the end of the transcript. Remove anything private
before posting:

```text
<paste here>
```
