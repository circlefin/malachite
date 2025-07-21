# calculate_average.py

import argparse
import pandas as pd
import sys

def average_csv_files(input_files, output_file):
    if len(input_files) == 1:
        df = pd.read_csv(input_files[0], header=None)
        df.to_csv(output_file, index=False, header=False)
        print(f"[✔] Only one file provided — copied directly to {output_file}")
        return

    dfs = [pd.read_csv(f, header=None) for f in input_files]

    base = dfs[0].iloc[:, :2]
    for i, df in enumerate(dfs[1:], start=2):
        if not base.equals(df.iloc[:, :2]):
            raise ValueError(f"Mismatch in columns 0 and 1 between file 1 and file {i}.")

    values = [df.iloc[:, 2] for df in dfs]
    avg = sum(values) / len(values)

    result = dfs[0].copy()
    result.iloc[:, 2] = avg.round().astype(int)
    result.to_csv(output_file, index=False, header=False)
    print(f"[✔] Averaged output written to: {output_file}")

def main():
    parser = argparse.ArgumentParser(description="Average the third column across multiple formatted CSV files.")
    parser.add_argument("input_files", nargs="+", help="Input CSV files with identical node_name and timestamp")
    parser.add_argument("--output", required=True, help="Output file for the averaged result")
    args = parser.parse_args()

    average_csv_files(args.input_files, args.output)

if __name__ == "__main__":
    main()
