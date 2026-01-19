#!/usr/bin/env python3
"""Compare two normalized MPS files and generate a diff report."""
import sys
from collections import defaultdict


def parse_mps(path):
    """Parse normalized MPS file into sections with structured data."""
    sections = {
        'NAME': None,
        'ROWS': {},      # row_name -> type (N, E, L, G)
        'COLUMNS': {},   # (col_name, row_name) -> value
        'RHS': {},       # (rhs_name, row_name) -> value
        'BOUNDS': {},    # (type, bnd_name, var_name) -> value or None
    }
    current_section = None

    with open(path, 'r') as f:
        for line in f:
            line = line.strip()
            if not line:
                continue

            parts = line.split()
            first_word = parts[0]

            if first_word in {'NAME', 'ROWS', 'COLUMNS', 'RHS', 'RANGES', 'BOUNDS', 'ENDATA'}:
                current_section = first_word
                if current_section == 'NAME' and len(parts) > 1:
                    sections['NAME'] = parts[1]
                continue

            if current_section == 'ROWS':
                # type row_name
                if len(parts) >= 2:
                    row_type, row_name = parts[0], parts[1]
                    sections['ROWS'][row_name] = row_type

            elif current_section == 'COLUMNS':
                # col_name row_name value
                if len(parts) >= 3:
                    col_name, row_name, value = parts[0], parts[1], parts[2]
                    sections['COLUMNS'][(col_name, row_name)] = value

            elif current_section == 'RHS':
                # rhs_name row_name value
                if len(parts) >= 3:
                    rhs_name, row_name, value = parts[0], parts[1], parts[2]
                    sections['RHS'][(rhs_name, row_name)] = value

            elif current_section == 'BOUNDS':
                # type bnd_name var_name [value]
                if len(parts) >= 3:
                    bnd_type, bnd_name, var_name = parts[0], parts[1], parts[2]
                    value = parts[3] if len(parts) > 3 else None
                    sections['BOUNDS'][(bnd_type, bnd_name, var_name)] = value

    return sections


def compare_dicts(truth, attempt, section_name, key_formatter=str):
    """Compare two dictionaries and return differences."""
    truth_keys = set(truth.keys())
    attempt_keys = set(attempt.keys())

    missing = truth_keys - attempt_keys  # In truth but not in attempt
    extra = attempt_keys - truth_keys    # In attempt but not in truth
    common = truth_keys & attempt_keys

    # Find value differences in common keys
    different = {}
    for key in common:
        if truth[key] != attempt[key]:
            different[key] = (truth[key], attempt[key])

    return missing, extra, different


def format_key(key):
    """Format a key for display."""
    if isinstance(key, tuple):
        return ' | '.join(str(k) for k in key)
    return str(key)


def main():
    if len(sys.argv) != 3:
        print("Usage: compare_mps.py <ground_truth.mps> <attempt.mps>")
        sys.exit(1)

    truth_path, attempt_path = sys.argv[1], sys.argv[2]

    print(f"Parsing ground truth: {truth_path}")
    truth = parse_mps(truth_path)

    print(f"Parsing attempt: {attempt_path}")
    attempt = parse_mps(attempt_path)

    print("\n" + "=" * 80)
    print("MPS COMPARISON REPORT")
    print("=" * 80)
    print(f"\nGround Truth: {truth_path}")
    print(f"Attempt:      {attempt_path}")

    # NAME
    print(f"\n{'─' * 80}")
    print("NAME")
    print(f"{'─' * 80}")
    if truth['NAME'] != attempt['NAME']:
        print(f"  Ground Truth: {truth['NAME']}")
        print(f"  Attempt:      {attempt['NAME']}")
    else:
        print(f"  Match: {truth['NAME']}")

    # Compare each section
    for section in ['ROWS', 'COLUMNS', 'RHS', 'BOUNDS']:
        print(f"\n{'─' * 80}")
        print(f"{section}")
        print(f"{'─' * 80}")

        missing, extra, different = compare_dicts(
            truth[section], attempt[section], section
        )

        print(f"  Ground Truth entries: {len(truth[section]):,}")
        print(f"  Attempt entries:      {len(attempt[section]):,}")
        print()
        print(f"  Missing (in truth, not in attempt): {len(missing):,}")
        print(f"  Extra (in attempt, not in truth):   {len(extra):,}")
        print(f"  Value differences:                  {len(different):,}")

        # Show samples of differences
        MAX_SAMPLES = 10

        if missing:
            print(f"\n  Sample MISSING entries (first {min(MAX_SAMPLES, len(missing))}):")
            for i, key in enumerate(sorted(missing)):
                if i >= MAX_SAMPLES:
                    print(f"    ... and {len(missing) - MAX_SAMPLES} more")
                    break
                val = truth[section][key]
                print(f"    {format_key(key)} = {val}")

        if extra:
            print(f"\n  Sample EXTRA entries (first {min(MAX_SAMPLES, len(extra))}):")
            for i, key in enumerate(sorted(extra)):
                if i >= MAX_SAMPLES:
                    print(f"    ... and {len(extra) - MAX_SAMPLES} more")
                    break
                val = attempt[section][key]
                print(f"    {format_key(key)} = {val}")

        if different:
            print(f"\n  Sample VALUE DIFFERENCES (first {min(MAX_SAMPLES, len(different))}):")
            for i, key in enumerate(sorted(different)):
                if i >= MAX_SAMPLES:
                    print(f"    ... and {len(different) - MAX_SAMPLES} more")
                    break
                truth_val, attempt_val = different[key]
                print(f"    {format_key(key)}")
                print(f"      Truth:   {truth_val}")
                print(f"      Attempt: {attempt_val}")

    # Summary
    print(f"\n{'=' * 80}")
    print("SUMMARY")
    print("=" * 80)

    total_missing = 0
    total_extra = 0
    total_diff = 0

    for section in ['ROWS', 'COLUMNS', 'RHS', 'BOUNDS']:
        missing, extra, different = compare_dicts(truth[section], attempt[section], section)
        total_missing += len(missing)
        total_extra += len(extra)
        total_diff += len(different)
        if missing or extra or different:
            print(f"  {section}: {len(missing)} missing, {len(extra)} extra, {len(different)} different")

    print()
    print(f"  Total missing:    {total_missing:,}")
    print(f"  Total extra:      {total_extra:,}")
    print(f"  Total different:  {total_diff:,}")

    if total_missing == 0 and total_extra == 0 and total_diff == 0:
        print("\n  ✓ Files are semantically identical!")
    else:
        print(f"\n  ✗ Files have {total_missing + total_extra + total_diff:,} total discrepancies")


if __name__ == '__main__':
    main()
