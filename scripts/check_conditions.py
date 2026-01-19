#!/usr/bin/env python3
"""
Analyze a GMPL model file to extract param names, var names,
and names referenced in domain conditions.
"""

import re
from pathlib import Path


def extract_params(content: str) -> set[str]:
    """Extract parameter names from 'param Name{...}' or 'param Name, ...' declarations."""
    # Match 'param' followed by name (word chars), stopping at {, comma, :=, or ;
    pattern = r'\bparam\s+(\w+)'
    return set(re.findall(pattern, content))


def extract_vars(content: str) -> set[str]:
    """Extract variable names from 'var Name{...}' declarations."""
    pattern = r'\bvar\s+(\w+)'
    return set(re.findall(pattern, content))


def extract_condition_names(content: str, start_line: int = 220) -> set[str]:
    """
    Extract names referenced in domain conditions of form {domains: conditions}.

    We look for the pattern {<domains>: <conditions>} and extract names
    that are followed by '[' in the condition part (after the colon).

    Only processes content after start_line (constraints section).
    """
    # Split by lines and get content after start_line
    lines = content.split('\n')
    constraints_content = '\n'.join(lines[start_line:])

    # Find all {...:...} blocks - need to handle nested braces
    # Using a simpler approach: find all {...} blocks that contain a colon
    names = set()

    # Pattern to find brace expressions with conditions
    # We'll iterate through and find matching braces
    i = 0
    while i < len(constraints_content):
        if constraints_content[i] == '{':
            # Find the matching closing brace
            depth = 1
            start = i
            j = i + 1
            while j < len(constraints_content) and depth > 0:
                if constraints_content[j] == '{':
                    depth += 1
                elif constraints_content[j] == '}':
                    depth -= 1
                j += 1

            if depth == 0:
                block = constraints_content[start+1:j-1]  # Content inside braces

                # Check if this block has a condition (contains ':')
                if ':' in block:
                    # Find the colon (but not ::)
                    # Split on first ':' that's not part of ':='
                    colon_idx = -1
                    for k, c in enumerate(block):
                        if c == ':' and (k + 1 >= len(block) or block[k+1] not in ':='):
                            colon_idx = k
                            break

                    if colon_idx > 0:
                        condition = block[colon_idx+1:]

                        # Extract names followed by '['
                        # Pattern: word characters followed by '['
                        cond_names = re.findall(r'(\w+)\s*\[', condition)
                        names.update(cond_names)

            i = j
        else:
            i += 1

    return names


def main():
    filepath = Path("examples/a.mod")
    content = filepath.read_text()

    params = extract_params(content)
    vars_ = extract_vars(content)
    condition_names = extract_condition_names(content, start_line=220)

    print("=" * 60)
    print("PARAMETER NAMES")
    print("=" * 60)
    for name in sorted(params):
        print(f"  {name}")
    print(f"\nTotal: {len(params)} params")

    print("\n" + "=" * 60)
    print("VARIABLE NAMES")
    print("=" * 60)
    for name in sorted(vars_):
        print(f"  {name}")
    print(f"\nTotal: {len(vars_)} vars")

    print("\n" + "=" * 60)
    print("NAMES REFERENCED IN CONDITIONS")
    print("=" * 60)
    for name in sorted(condition_names):
        print(f"  {name}")
    print(f"\nTotal: {len(condition_names)} unique names in conditions")

    # Sense checking
    print("\n" + "=" * 60)
    print("SENSE CHECKING")
    print("=" * 60)

    all_known = params | vars_

    # Which condition names are params?
    cond_params = condition_names & params
    print(f"\nCondition names that are params: {len(cond_params)}")
    for name in sorted(cond_params):
        print(f"  {name}")

    # Which condition names are vars?
    cond_vars = condition_names & vars_
    print(f"\nCondition names that are vars: {len(cond_vars)}")
    for name in sorted(cond_vars):
        print(f"  {name}")

    # Which condition names are unknown?
    unknown = condition_names - all_known
    print(f"\nCondition names that are UNKNOWN (not param or var): {len(unknown)}")
    for name in sorted(unknown):
        print(f"  {name}")

    # Final assertion
    print("\n" + "=" * 60)
    print("FINAL CHECK")
    print("=" * 60)
    if condition_names == cond_params:
        print("\n✓ PASS: All condition names are params")
    else:
        print("\n✗ FAIL: Not all condition names are params!")
        if cond_vars:
            print(f"  Found {len(cond_vars)} vars in conditions: {sorted(cond_vars)}")
        if unknown:
            print(f"  Found {len(unknown)} unknown names: {sorted(unknown)}")
        exit(1)


if __name__ == "__main__":
    main()
