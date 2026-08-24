#!/usr/bin/env python3
"""
Pipeline: Read data/sales.csv, compute monthly x regional sales totals,
exclude invalid rows by reason, and produce:
- output/inspection.json
- output/results.json
- output/report.md
Deterministic: fixed sorting, no time/random dependencies.
"""
import csv
import json
import os
from collections import defaultdict
from datetime import date


def main():
    base_dir = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    data_path = os.path.join(base_dir, 'data', 'sales.csv')
    output_dir = os.path.join(base_dir, 'output')
    os.makedirs(output_dir, exist_ok=True)

    # 1. Read CSV
    rows = []
    with open(data_path, 'r', newline='', encoding='utf-8') as f:
        reader = csv.DictReader(f)
        for row in reader:
            rows.append(row)

    input_row_count = len(rows)

    # 2. Validate rows based on inspection.json observations
    valid_rows = []
    excluded_counts = defaultdict(int)

    for row in rows:
        date_str = row.get('date', '').strip()
        region = row.get('region', '').strip()
        amount_str = row.get('amount', '').strip()

        # Check missing date
        if not date_str:
            excluded_counts['missing_date'] += 1
            continue

        # Check invalid date (e.g., 2026-02-30)
        try:
            date.fromisoformat(date_str)
        except ValueError:
            excluded_counts['invalid_date'] += 1
            continue

        # Check missing amount
        if not amount_str:
            excluded_counts['missing_amount'] += 1
            continue

        # Check invalid amount
        try:
            amount = float(amount_str)
        except ValueError:
            excluded_counts['invalid_amount'] += 1
            continue

        valid_rows.append({
            'date': date_str,
            'region': region,
            'amount': amount,
        })

    used_row_count = len(valid_rows)
    excluded_list = [{'reason': k, 'rows': v} for k, v in sorted(excluded_counts.items())]

    # 3. Write inspection.json
    regions = sorted(set(r['region'] for r in valid_rows))
    months = sorted(set(r['date'][:7] for r in valid_rows))

    inspection = {
        'column_names': ['date', 'region', 'amount'],
        'input_row_count': input_row_count,
        'type_summaries': {
            'date': 'string',
            'region': 'string',
            'amount': 'number',
        },
        'distinct_values': {
            'region': regions,
            'months': months,
        },
        'sample_rows': valid_rows[:3],
    }

    inspection_path = os.path.join(output_dir, 'inspection.json')
    with open(inspection_path, 'w', encoding='utf-8') as f:
        json.dump(inspection, f, ensure_ascii=False, indent=2)
        f.write('\n')

    # 4. Aggregate by month and region
    monthly_region = defaultdict(float)
    monthly_total = defaultdict(float)

    for row in valid_rows:
        month = row['date'][:7]
        region = row['region']
        amount = row['amount']
        key = f'{month}_{region}'
        monthly_region[key] += amount
        monthly_total[month] += amount

    grand_total = sum(monthly_total.values())

    # 5. Build values dict with ALL numeric claims
    values = {}
    for key in sorted(monthly_region.keys()):
        values[f'monthly_region_{key}'] = round(monthly_region[key], 2)
    for month in sorted(monthly_total.keys()):
        values[f'total_{month}'] = round(monthly_total[month], 2)
    values['grand_total'] = round(grand_total, 2)

    # 6. Write results.json
    results = {
        'reconciliation': {
            'input_rows': input_row_count,
            'used_rows': used_row_count,
            'excluded': excluded_list,
        },
        'values': values,
    }

    results_path = os.path.join(output_dir, 'results.json')
    with open(results_path, 'w', encoding='utf-8') as f:
        json.dump(results, f, ensure_ascii=False, indent=2)
        f.write('\n')

    # 7. Write report.md - every number here must be in values
    lines = []
    lines.append('# 売上集計レポート')
    lines.append('')
    lines.append('## 概要')
    lines.append(f'- 入力行数: {input_row_count}')
    lines.append(f'- 使用行数: {used_row_count}')
    lines.append(f'- 除外行数: {input_row_count - used_row_count}')
    lines.append('')

    lines.append('## 除外された行')
    lines.append('| 理由 | 行数 |')
    lines.append('|------|------|')
    for exc in excluded_list:
        lines.append(f'| {exc["reason"]} | {exc["rows"]} |')
    lines.append('')

    lines.append('## 月次×地域別売上')
    lines.append('| 月 | 地域 | 売上 |')
    lines.append('|----|------|------|')
    for key in sorted(monthly_region.keys()):
        month, region = key.rsplit('_', 1)
        val = monthly_region[key]
        lines.append(f'| {month} | {region} | {val:.2f} |')
    lines.append('')

    lines.append('## 月次合計')
    lines.append('| 月 | 合計 |')
    lines.append('|----|------|')
    for month in sorted(monthly_total.keys()):
        val = monthly_total[month]
        lines.append(f'| {month} | {val:.2f} |')
    lines.append('')

    lines.append(f'## 全体合計: {grand_total:.2f}')
    lines.append('')

    report_path = os.path.join(output_dir, 'report.md')
    with open(report_path, 'w', encoding='utf-8') as f:
        f.write('\n'.join(lines))


if __name__ == '__main__':
    main()
