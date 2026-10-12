"""Sums up a strike sweep report: runs in band, real-life mismatches, and the
counts of bone outside the body (closed breaks' pieces and whole bones outside
the skin, and broken bone thrown clear), so two sweeps can be compared at a
glance.

    python tools\\sweep_summary.py [output\\strike_sweep_report.txt ...]
"""
import re
import sys


def summarize(path):
    text = open(path, encoding='utf-8').read()
    in_band = runs = closed = whole = loose = 0
    problems = []
    scenario = '?'
    for line in text.splitlines():
        header = re.match(r'^(\w+): in band (\d+)/(\d+) runs', line)
        if header:
            scenario = header.group(1)
            in_band += int(header.group(2))
            runs += int(header.group(3))
            continue
        found = re.search(r"closed break's piece ends outside the skin in (\d+)", line)
        if found:
            closed += int(found.group(1))
            problems.append(f'{scenario}: closed pieces outside the skin in {found.group(1)}')
        found = re.search(r'a whole bone ends outside the skin in (\d+)', line)
        if found:
            whole += int(found.group(1))
            problems.append(f'{scenario}: whole bone outside the skin in {found.group(1)}')
        if 'out of band' in line:
            loose += sum(int(n) for n in re.findall(r'bone_pieces_out in (\d+)', line))
            problems.append(f'{scenario}: {line.strip()}')
        if 'runs, but' in line:
            problems.append(f'real life: {line.strip()}')
    mismatches = sum(1 for p in problems if p.startswith('real life'))
    print(path)
    print(f'  runs in band {in_band}/{runs}, real-life mismatches {mismatches}, '
          f'closed-break pieces outside the skin {closed}, whole bones outside the skin {whole}, '
          f'bone thrown out {loose}')
    for problem in problems:
        print('   ', problem)


for path in sys.argv[1:] or ['output/strike_sweep_report.txt']:
    summarize(path)
