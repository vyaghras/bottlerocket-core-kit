#!/usr/bin/env bash

# check that all `# headers` are formatted as a version, e.g. v1.2.3
# this should be sufficient to validate the CHANGELOG for our CI, provided that
# every new tag has a corresponding CHANGELOG update, as we always parse the
# CHANGELOG between two headers with a tagged version.
if diff <(grep -ne '^# ' CHANGELOG.md) <(grep -ne '^# v[0-9]\+\.[0-9]\+\.[0-9]\+' CHANGELOG.md); then
  echo "CHANGELOG validation PASSED!"
else
  echo "CHANGELOG validation FAILED! Headers must match the regex '^# v[0-9]\+\.[0-9]\+\.[0-9]\+.'"
  exit 1
fi
