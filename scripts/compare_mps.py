#!/usr/bin/env python3
"""Compare two normalized MPS files and generate a diff report."""

import sys
from dataclasses import dataclass, field


@dataclass
class MpsSections:
    """Structured representation of an MPS file's sections."""

    name: str | None = None
    rows: dict[str, str] = field(default_factory=dict)  # row_name -> type (N, E, L, G)
    columns: dict[tuple[str, str], str] = field(
        default_factory=dict
    )  # (col_name, row_name) -> value
    rhs: dict[tuple[str, str], str] = field(
        default_factory=dict
    )  # (rhs_name, row_name) -> value
    bounds: dict[tuple[str, str, str], str | None] = field(
        default_factory=dict
    )  # (type, bnd_name, var_name) -> value or None


def parse_mps(path: str) -> MpsSections:
    """Parse normalized MPS file into sections with structured data."""
    sections = MpsSections()
    current_section: str | None = None
    line_count: int = 0

    with open(path, "r") as f:
        for line in f:
            line_count += 1
            if line_count % 1_000_000 == 0:
                print(f"  ... {line_count:,} lines parsed", flush=True)
            line = line.strip()
            if not line:
                continue

            parts = line.split()
            first_word = parts[0]

            if first_word in {
                "NAME",
                "ROWS",
                "COLUMNS",
                "RHS",
                "RANGES",
                "BOUNDS",
                "ENDATA",
            }:
                current_section = first_word
                if current_section == "NAME" and len(parts) > 1:
                    sections.name = parts[1]
                continue

            if current_section == "ROWS":
                # type row_name
                if len(parts) >= 2:
                    row_type, row_name = parts[0], parts[1]
                    sections.rows[row_name] = row_type

            elif current_section == "COLUMNS":
                # col_name row_name value
                if len(parts) >= 3:
                    col_name, row_name, value = parts[0], parts[1], parts[2]
                    sections.columns[(col_name, row_name)] = value

            elif current_section == "RHS":
                # rhs_name row_name value
                if len(parts) >= 3:
                    rhs_name, row_name, value = parts[0], parts[1], parts[2]
                    sections.rhs[(rhs_name, row_name)] = value

            elif current_section == "BOUNDS":
                # type bnd_name var_name [value]
                if len(parts) >= 3:
                    bnd_type, bnd_name, var_name = parts[0], parts[1], parts[2]
                    value = parts[3] if len(parts) > 3 else None
                    sections.bounds[(bnd_type, bnd_name, var_name)] = value

    return sections


type DictKey = str | tuple[str, ...]
type DictVal = str | None


def compare_dicts(
    truth: dict[DictKey, DictVal],
    attempt: dict[DictKey, DictVal],
) -> tuple[set[DictKey], set[DictKey], dict[DictKey, tuple[DictVal, DictVal]]]:
    """Compare two dictionaries and return differences."""
    truth_keys = set(truth.keys())
    attempt_keys = set(attempt.keys())

    missing = truth_keys - attempt_keys  # In truth but not in attempt
    extra = attempt_keys - truth_keys  # In attempt but not in truth
    common = truth_keys & attempt_keys

    # Find value differences in common keys
    different: dict[DictKey, tuple[DictVal, DictVal]] = {}
    for key in common:
        if truth[key] != attempt[key]:
            different[key] = (truth[key], attempt[key])

    return missing, extra, different


def format_key(key: DictKey) -> str:
    """Format a key for display."""
    if isinstance(key, tuple):
        return " | ".join(str(k) for k in key)
    return str(key)


def get_section(sections: MpsSections, name: str) -> dict[DictKey, DictVal]:
    """Get a section dict by name."""
    return getattr(sections, name.lower())  # pyright: ignore[reportAny]


def main() -> None:
    if len(sys.argv) != 3:
        print("Usage: compare_mps.py <ground_truth.mps> <attempt.mps>")
        sys.exit(1)

    truth_path: str = sys.argv[1]
    attempt_path: str = sys.argv[2]

    print(f"Parsing ground truth: {truth_path}")
    truth: MpsSections = parse_mps(truth_path)

    print(f"Parsing attempt: {attempt_path}")
    attempt: MpsSections = parse_mps(attempt_path)

    print("\n" + "=" * 80)
    print("MPS COMPARISON REPORT")
    print("=" * 80)
    print(f"\nGround Truth: {truth_path}")
    print(f"Attempt:      {attempt_path}")

    # NAME
    print(f"\n{'─' * 80}")
    print("NAME")
    print(f"{'─' * 80}")
    if truth.name != attempt.name:
        print(f"  Ground Truth: {truth.name}")
        print(f"  Attempt:      {attempt.name}")
    else:
        print(f"  Match: {truth.name}")

    section_names: list[str] = ["ROWS", "COLUMNS", "RHS", "BOUNDS"]

    # Compare each section
    for section in section_names:
        print(f"\n{'─' * 80}")
        print(f"{section}")
        print(f"{'─' * 80}")

        truth_dict: dict[DictKey, DictVal] = get_section(truth, section)
        attempt_dict: dict[DictKey, DictVal] = get_section(attempt, section)

        missing, extra, different = compare_dicts(truth_dict, attempt_dict)

        print(f"  Ground Truth entries: {len(truth_dict):,}")
        print(f"  Attempt entries:      {len(attempt_dict):,}")
        print()
        print(f"  Missing (in truth, not in attempt): {len(missing):,}")
        print(f"  Extra (in attempt, not in truth):   {len(extra):,}")
        print(f"  Value differences:                  {len(different):,}")

        # Show samples of differences
        MAX_SAMPLES: int = 10

        if missing:
            print(
                f"\n  Sample MISSING entries (first {min(MAX_SAMPLES, len(missing))}):"
            )
            for i, key in enumerate(sorted(missing)):
                if i >= MAX_SAMPLES:
                    print(f"    ... and {len(missing) - MAX_SAMPLES} more")
                    break
                val = truth_dict[key]
                print(f"    {format_key(key)} = {val}")

        if extra:
            print(f"\n  Sample EXTRA entries (first {min(MAX_SAMPLES, len(extra))}):")
            for i, key in enumerate(sorted(extra)):
                if i >= MAX_SAMPLES:
                    print(f"    ... and {len(extra) - MAX_SAMPLES} more")
                    break
                val = attempt_dict[key]
                print(f"    {format_key(key)} = {val}")

        if different:
            print(
                f"\n  Sample VALUE DIFFERENCES (first {min(MAX_SAMPLES, len(different))}):"
            )
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

    total_missing: int = 0
    total_extra: int = 0
    total_diff: int = 0

    for section in section_names:
        truth_dict = get_section(truth, section)
        attempt_dict = get_section(attempt, section)
        missing, extra, different = compare_dicts(truth_dict, attempt_dict)
        total_missing += len(missing)
        total_extra += len(extra)
        total_diff += len(different)
        if missing or extra or different:
            print(
                f"  {section}: {len(missing)} missing, {len(extra)} extra, {len(different)} different"
            )

    print()
    print(f"  Total missing:    {total_missing:,}")
    print(f"  Total extra:      {total_extra:,}")
    print(f"  Total different:  {total_diff:,}")

    if total_missing == 0 and total_extra == 0 and total_diff == 0:
        print("\n  ✓ Files are semantically identical!")
    else:
        print(
            f"\n  ✗ Files have {total_missing + total_extra + total_diff:,} total discrepancies"
        )


if __name__ == "__main__":
    main()
