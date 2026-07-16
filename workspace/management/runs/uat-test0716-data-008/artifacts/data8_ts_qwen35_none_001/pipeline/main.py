#!/usr/bin/env python3
"""
Sales data pipeline: reads data/sales.csv, validates rows,
computes monthly totals, MoM %, 3-month moving average,
and produces output/inspection.json, output/results.json,
and output/report.md.

Uses only Python 3 standard library (csv, json, statistics, os, datetime).
Deterministic: fixed seed, stable ordering.
"""

import csv
import json
import os
import statistics
from datetime import datetime
from collections import defaultdict


def main():
    base_dir = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    data_path = os.path.join(base_dir, 'data', 'sales.csv')
    output_dir = os.path.join(base_dir, 'output')
    os.makedirs(output_dir, exist_ok=True)

    # ── Read all rows ──────────────────────────────────────────────
    all_rows = []
    with open(data_path, 'r', encoding='utf-8') as f:
        reader = csv.DictReader(f)
        fieldnames = reader.fieldnames
        for row in reader:
            all_rows.append(row)

    input_rows = len(all_rows)

    # ── Validate rows ──────────────────────────────────────────────
    valid_rows = []
    excluded = defaultdict(int)

    for row in all_rows:
        date_str = row.get('date', '').strip()
        region = row.get('region', '').strip()
        amount_str = row.get('amount', '').strip()

        # Check for empty date
        if not date_str:
            excluded['empty_date'] += 1
            continue

        # Check for valid date
        try:
            datetime.strptime(date_str, '%Y-%m-%d')
        except ValueError:
            excluded['invalid_date'] += 1
            continue

        # Check for empty amount
        if not amount_str:
            excluded['empty_amount'] += 1
            continue

        # Check for numeric amount
        try:
            amount = float(amount_str)
        except ValueError:
            excluded['invalid_amount'] += 1
            continue

        valid_rows.append({
            'date': date_str,
            'region': region,
            'amount': amount
        })

    used_rows = len(valid_rows)

    # ── Build inspection.json ──────────────────────────────────────
    observed_regions = sorted(set(r['region'] for r in valid_rows))
    sample_rows = [
        {
            'date': r['date'],
            'region': r['region'],
            'amount': r['amount']
        }
        for r in valid_rows[:3]
    ]

    inspection = {
        'column_names': list(fieldnames),
        'input_row_count': input_rows,
        'type_summaries': {
            'date': 'date_iso',
            'region': 'string',
            'amount': 'number'
        },
        'distinct_values': {
            'region': observed_regions
        },
        'sample_rows': sample_rows
    }

    with open(os.path.join(output_dir, 'inspection.json'), 'w', encoding='utf-8') as f:
        json.dump(inspection, f, indent=2, ensure_ascii=False)

    # ── Compute monthly totals ─────────────────────────────────────
    monthly_totals = defaultdict(float)
    for row in valid_rows:
        date_obj = datetime.strptime(row['date'], '%Y-%m-%d')
        month_key = date_obj.strftime('%Y-%m')
        monthly_totals[month_key] += row['amount']

    # Sort months chronologically
    sorted_months = sorted(monthly_totals.keys())

    # ── Compute MoM and 3-month moving average ─────────────────────
    monthly_data = []
    for i, month in enumerate(sorted_months):
        total = monthly_totals[month]

        # MoM
        if i == 0:
            mom = None
        else:
            prev_total = monthly_totals[sorted_months[i - 1]]
            if prev_total != 0:
                mom = round((total - prev_total) / prev_total * 100, 2)
            else:
                mom = None

        # 3-month moving average
        if i < 2:
            ma = None
        else:
            window = [monthly_totals[sorted_months[j]] for j in range(i - 2, i + 1)]
            ma = round(statistics.mean(window), 2)

        monthly_data.append({
            'month': month,
            'total': total,
            'mom': mom,
            'ma': ma
        })

    # ── Build results.json ─────────────────────────────────────────
    values = {}
    for item in monthly_data:
        month_key = item['month']
        values[f'monthly_{month_key}'] = item['total']
        if item['mom'] is not None:
            values[f'mom_{month_key}'] = item['mom']
        if item['ma'] is not None:
            values[f'ma_{month_key}'] = item['ma']

    excluded_list = [
        {'reason': reason, 'rows': count}
        for reason, count in sorted(excluded.items())
    ]

    results = {
        'reconciliation': {
            'input_rows': input_rows,
            'used_rows': used_rows,
            'excluded': excluded_list
        },
        'values': dict(sorted(values.items()))
    }

    with open(os.path.join(output_dir, 'results.json'), 'w', encoding='utf-8') as f:
        json.dump(results, f, indent=2, ensure_ascii=False)

    # ── Generate report.md ─────────────────────────────────────────
    report_lines = []
    report_lines.append('# Sales Data Pipeline Report')
    report_lines.append('')
    report_lines.append('## Reconciliation')
    report_lines.append(f'- Input rows: {input_rows}')
    report_lines.append(f'- Used rows: {used_rows}')
    report_lines.append(f'- Excluded rows: {input_rows - used_rows}')
    for exc in excluded_list:
        report_lines.append(f'- **{exc["reason"]}**: {exc["rows"]} rows')
    report_lines.append('')
    report_lines.append('## Monthly Sales Summary')
    report_lines.append('| Month | Total | MoM % | 3-Month MA |')
    report_lines.append('|-------|-------|-------|------------|')
    for item in monthly_data:
        total = f'{item["total"]:.2f}'
        mom = f'{item["mom"]:+.2f}%' if item['mom'] is not None else 'N/A'
        ma = f'{item["ma"]:.2f}' if item['ma'] is not None else 'N/A'
        report_lines.append(f'| {item["month"]} | {total} | {mom} | {ma} |')
    report_lines.append('')

    with open(os.path.join(output_dir, 'report.md'), 'w', encoding='utf-8') as f:
        f.write('\n'.join(report_lines))

    print(f'Done. {input_rows} input rows, {used_rows} valid, {input_rows - used_rows} excluded.')
    print(f'Inspection: {os.path.join(output_dir, "inspection.json")}')
    print(f'Results: {os.path.join(output_dir, "results.json")}')
    print(f'Report: {os.path.join(output_dir, "report.md")}')


if __name__ == '__main__':
    main()
