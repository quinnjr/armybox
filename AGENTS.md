<!-- converted from Cursor rules -->

## Cursor rule: `.cursor/rules/posix-compliance.mdc`

_POSIX compliance standards for armybox applets_

Applies to: `["src/applets/**/*.rs", "src/**/*.rs"]`

# POSIX Compliance Requirements

All armybox applets MUST be 100% POSIX.1-2017 (IEEE Std 1003.1-2017) compliant.

## Core Principles

1. **All POSIX-specified options must be implemented** - Every option defined by POSIX for a utility must be supported with the exact behavior specified.

2. **Exit codes must follow POSIX standards**:
   - 0 = Success
   - 1 = General errors (file not found, permission denied, etc.)
   - 2 = Incorrect usage (invalid options, missing operands)
   - Other codes as specified by individual utility standards

3. **Error messages must go to stderr** - Never write error messages to stdout.

4. **Standard input/output behavior**:
   - When no file operands are given, read from stdin
   - "-" as a filename means stdin/stdout
   - Support reading from multiple files when applicable

## Required Options by Category

### File Test Operators (test/[)
Must support all POSIX primaries:
- -b, -c, -d, -e, -f, -g, -h/-L, -k, -n, -p, -r, -s, -S, -t, -u, -w, -x, -z
- -O, -G (owned by effective UID/GID)
- -nt, -ot, -ef (file comparisons)
- ! (negation)

### Text Utilities
- grep: Must support -E, -F, -v, -c, -i, -l, -n, -q, -s, -x
- sed: Must support s, d, p, a, i, c commands and address ranges
- tr: Must support -c, -C, -d, -s and all POSIX character classes
- sort: Must support -b, -d, -f, -i, -k, -n, -r, -t, -u
- cut: Must support -b, -c, -d, -f, -s

### File Utilities
- ls: Must support -a, -A, -C, -d, -F, -i, -l, -p, -R, -r, -S, -t, -1
- cp: Must support -f, -i, -p, -R/-r
- rm: Must support -f, -i, -r/-R
- mkdir: Must support -m, -p
- chmod: Must support symbolic and octal modes

### Character Classes
All POSIX character classes must be supported in tr, grep, etc:
[:alnum:], [:alpha:], [:blank:], [:cntrl:], [:digit:], [:graph:],
[:lower:], [:print:], [:punct:], [:space:], [:upper:], [:xdigit:]

## Implementation Guidelines

When implementing or modifying applets:
1. Reference the POSIX specification for the utility
2. Ensure all mandatory options are implemented
3. Test with standard POSIX test cases
4. Verify exit codes match specification
5. Check error output goes to stderr

## References

- POSIX.1-2017: https://pubs.opengroup.org/onlinepubs/9699919799/
- Shell & Utilities: https://pubs.opengroup.org/onlinepubs/9699919799/utilities/contents.html

