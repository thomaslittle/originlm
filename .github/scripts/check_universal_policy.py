#!/usr/bin/env python3
"""Refuse application- or business-specific logic in OriginLM pull requests.

OriginLM is a universal product. This guard scans added lines for known
consumer/business names and fails with the refusal reason. It is a guard, not
the whole policy: reviewers apply the broader rule in
docs/POLICY-UNIVERSAL-PRODUCT.md.
"""
from __future__ import annotations

import argparse
import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]

# Policy files are allowed to name the consumers they are protecting against.
SKIP = {
    '.github/app-specific-terms.txt',
    '.github/scripts/check_universal_policy.py',
    '.github/workflows/policy.yml',
    '.github/PULL_REQUEST_TEMPLATE.md',
    'CONTRIBUTING.md',
    'docs/POLICY-UNIVERSAL-PRODUCT.md',
}


def read_terms(path: pathlib.Path) -> list[str]:
    terms = []
    for line in path.read_text(encoding='utf-8').splitlines():
        line = line.strip()
        if line and not line.startswith('#'):
            terms.append(line)
    return terms


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--base', required=True)
    parser.add_argument('--head', default='HEAD')
    parser.add_argument('--terms', type=pathlib.Path, default=ROOT / '.github/app-specific-terms.txt')
    args = parser.parse_args()

    terms = read_terms(args.terms)
    if not terms:
        print('Universal product policy: no terms configured; skipping.')
        return 0

    diff = subprocess.run(
        ['git', 'diff', '--unified=0', '--no-color', f'{args.base}...{args.head}'],
        cwd=ROOT, text=True, capture_output=True,
    )
    if diff.returncode != 0:
        print(diff.stderr, file=sys.stderr)
        return 2

    current: str | None = None
    new_line = 0
    matches: list[tuple[str, int, str, str]] = []
    for line in diff.stdout.splitlines():
        if line.startswith('+++ b/'):
            current = line[6:]
            continue
        if line.startswith('@@'):
            found = re.search(r'\+(\d+)', line)
            new_line = int(found.group(1)) if found else 0
            continue
        if line.startswith('+') and not line.startswith('+++'):
            if current is not None and current not in SKIP:
                text = line[1:]
                for term in terms:
                    pattern = r'(?<![A-Za-z0-9_])' + re.escape(term) + r'(?![A-Za-z0-9_])'
                    if re.search(pattern, text, re.IGNORECASE):
                        matches.append((current, new_line, term, text.strip()[:160]))
            new_line += 1
        elif line.startswith('-') or line.startswith('\\'):
            continue
        else:
            new_line += 1

    if matches:
        print(
            'OriginLM is a universal product. Application-specific and '
            'business-specific logic is not accepted in this repository.\n',
            file=sys.stderr,
        )
        for file, line, term, text in matches[:50]:
            print(f'  {file}:{line}: matched "{term}" in: {text}', file=sys.stderr)
        print(
            '\nMove this logic to the consuming application, or propose a generic '
            'adapter/configuration. See docs/POLICY-UNIVERSAL-PRODUCT.md.',
            file=sys.stderr,
        )
        return 1

    print('Universal product policy: no application/business-specific terms found.')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
