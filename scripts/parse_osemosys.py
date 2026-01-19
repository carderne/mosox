#!/usr/bin/env python3
"""Parse GMPL file and analyze constraints."""

import re


def remove_brackets(text):
    """Remove content inside {} and [] including the brackets themselves."""
    # Remove nested braces - repeat until no more matches
    prev = None
    while prev != text:
        prev = text
        text = re.sub(r'\{[^{}]*\}', '', text)
    # Remove nested square brackets
    prev = None
    while prev != text:
        prev = text
        text = re.sub(r'\[[^\[\]]*\]', '', text)
    return text


def collapse_whitespace(text):
    """Collapse multiple whitespace into single space."""
    return re.sub(r'\s+', ' ', text).strip()


def extract_identifiers(text):
    """Extract alphanumeric identifiers from text."""
    return re.findall(r'\b([A-Za-z_][A-Za-z0-9_]*)\b', text)


def strip_comments(text):
    """Remove # comments from each line."""
    lines = text.split('\n')
    result = []
    for line in lines:
        # Find # that's not inside quotes (simple approach - just strip from #)
        idx = line.find('#')
        if idx >= 0:
            line = line[:idx]
        result.append(line)
    return '\n'.join(result)


def main():
    with open('examples/osemosys.mod', 'r') as f:
        content = f.read()

    # Strip comments first
    content = strip_comments(content)

    # Split on semicolons
    statements = content.split(';')

    params = {}
    vars_ = {}
    constraints = []

    for stmt in statements:
        # Normalize whitespace for matching
        stmt_normalized = collapse_whitespace(stmt)

        # Check for param (must be the primary statement, not inside s.t.)
        param_match = re.search(r'\bparam\s+([A-Za-z_][A-Za-z0-9_]*)', stmt_normalized)
        if param_match:
            # Make sure param is the main keyword (comes before s.t. if any)
            if 's.t.' not in stmt_normalized:
                params[param_match.group(1)] = True
                continue

        # Check for var (must be the primary statement)
        var_match = re.search(r'\bvar\s+([A-Za-z_][A-Za-z0-9_]*)', stmt_normalized)
        if var_match:
            if 's.t.' not in stmt_normalized:
                vars_[var_match.group(1)] = True
                continue

        # Check for s.t.
        st_match = re.search(r'\bs\.t\.\s+([A-Za-z_][A-Za-z0-9_]*)', stmt_normalized)
        if st_match:
            name = st_match.group(1)

            # Find the expression after the colon (after the index set)
            # The pattern is: s.t. Name{...}: expression
            # We need to find the colon after the closing brace
            # First, find position after name
            rest = stmt_normalized[st_match.end():]

            # Find the colon that separates index set from expression
            # Could be {index_set}: expr or just : expr if no index set
            brace_depth = 0
            colon_pos = -1
            for i, ch in enumerate(rest):
                if ch == '{':
                    brace_depth += 1
                elif ch == '}':
                    brace_depth -= 1
                elif ch == ':' and brace_depth == 0:
                    colon_pos = i
                    break

            if colon_pos == -1:
                continue

            expression = rest[colon_pos + 1:].strip()

            # Remove content inside {} and []
            expression = remove_brackets(expression)
            expression = collapse_whitespace(expression)

            # Split on equality sign (>=, <=, or =)
            # Need to match >= and <= before =
            eq_match = re.search(r'\s*(>=|<=|=)\s*', expression)
            if not eq_match:
                continue

            lhs = expression[:eq_match.start()].strip()
            rhs = expression[eq_match.end():].strip()

            constraints.append((name, lhs, rhs))

    # Reserved words to ignore
    reserved = {'sum', 'max', 'min', 'in', 'if', 'then', 'else', 'and', 'or', 'not', 'binary', 'integer'}

    # Analyze and print constraints
    for name, lhs, rhs in constraints:
        lhs_identifiers = extract_identifiers(lhs)
        rhs_identifiers = extract_identifiers(rhs)

        lhs_var_count = 0
        lhs_par_count = 0
        for ident in lhs_identifiers:
            if ident in reserved:
                continue
            if ident in vars_:
                lhs_var_count += 1
            elif ident in params:
                lhs_par_count += 1

        rhs_var_count = 0
        rhs_par_count = 0
        for ident in rhs_identifiers:
            if ident in reserved:
                continue
            if ident in vars_:
                rhs_var_count += 1
            elif ident in params:
                rhs_par_count += 1

        print(name)
        print(f"lhs_var_count: {lhs_var_count} / rhs_var_count: {rhs_var_count}")
        print(f"lhs_par_count: {lhs_par_count} / rhs_par_count: {rhs_par_count}")
        print(f"{lhs} / {rhs}")
        print()


if __name__ == '__main__':
    main()
