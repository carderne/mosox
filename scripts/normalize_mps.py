#!/usr/bin/env python3
import sys
import re


def round_number(s, decimals=2):
    """Round a numeric string to specified decimal places."""
    try:
        f = float(s)
        return f'{f:.{decimals}f}'.rstrip('0').rstrip('.')
    except ValueError:
        return s


def normalize_whitespace(line):
    return ' '.join(line.split())


def split_columns_line(line):
    parts = line.split()
    if len(parts) == 5:  # col row1 val1 row2 val2
        return [f"{parts[0]} {parts[1]} {round_number(parts[2])}",
                f"{parts[0]} {parts[3]} {round_number(parts[4])}"]
    elif len(parts) == 3:  # col row val
        return [f"{parts[0]} {parts[1]} {round_number(parts[2])}"]
    return [' '.join(parts)]


def split_rhs_line(line):
    parts = line.split()
    if len(parts) == 5:  # RHS1 row1 val1 row2 val2
        return [f"{parts[0]} {parts[1]} {round_number(parts[2])}",
                f"{parts[0]} {parts[3]} {round_number(parts[4])}"]
    elif len(parts) == 3:  # RHS1 row val
        return [f"{parts[0]} {parts[1]} {round_number(parts[2])}"]
    return [' '.join(parts)]


def normalize_mps(input_path, output_path):
    sections = {'NAME', 'ROWS', 'COLUMNS', 'RHS', 'RANGES', 'BOUNDS', 'ENDATA'}
    current_section = None
    section_lines = []

    with open(input_path, 'r') as f, open(output_path, 'w') as out:
        for line in f:
            line = line.rstrip('\n\r')

            # Skip comment lines
            if line.startswith('*'):
                continue

            # Check for section header (must be exactly the section keyword, optionally followed by more text)
            stripped = line.strip()
            first_word = stripped.split()[0] if stripped else ''
            is_section_header = first_word in sections

            if is_section_header:
                # Flush previous section
                if section_lines:
                    for sl in sorted(section_lines):
                        out.write(sl + '\n')
                    section_lines = []

                current_section = first_word
                out.write(normalize_whitespace(line) + '\n')
            else:
                # Process content line
                normalized = normalize_whitespace(line)
                if not normalized:
                    continue

                if current_section == 'COLUMNS':
                    section_lines.extend(split_columns_line(normalized))
                elif current_section == 'RHS':
                    section_lines.extend(split_rhs_line(normalized))
                elif current_section == 'BOUNDS':
                    # type bnd_name var_name [value]
                    parts = normalized.split()
                    if len(parts) == 4:
                        section_lines.append(f"{parts[0]} {parts[1]} {parts[2]} {round_number(parts[3])}")
                    else:
                        section_lines.append(normalized)
                else:
                    section_lines.append(normalized)

        # Flush final section
        for sl in sorted(section_lines):
            out.write(sl + '\n')


if __name__ == '__main__':
    normalize_mps(sys.argv[1], sys.argv[2])
