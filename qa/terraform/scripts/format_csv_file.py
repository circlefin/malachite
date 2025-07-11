import pandas as pd
import argparse
import sys
from pathlib import Path

def normalize_node_names(df):
    unique_nodes = df['node_name'].unique()
    mapping = {name: f"Node{idx + 1} ({name.split('-')[0]})" for idx, name in enumerate(unique_nodes)}
    return df['node_name'].map(mapping)

def process_throughput(df):
    df.columns = ['node_name', 'timestamp', 'throughput']
    df['node_name'] = normalize_node_names(df)
    df['timestamp'] = (df['timestamp'] - df['timestamp'].min()) / 60

    # Check for NaNs, can happen if experiment duration is less than 10 minutes 
    print(f"[i] Initial rows: {len(df)}")
    print(f"[i] NaNs in 'throughput': {df['throughput'].isna().sum()}")
    df = df.dropna(subset=['throughput'])
    print(f"[i] Rows after cleaning: {len(df)}")

    df['throughput'] = df['throughput'].astype(int)
    return df


def process_block_time(df):
    df.columns = ['node_name', 'timestamp', 'latency']
    df['node_name'] = normalize_node_names(df)
    df['timestamp'] = (df['timestamp'] - df['timestamp'].min()) / 60

    # Check for NaNs, can happen if experiment duration is less than 10 minutes 
    print(f"[i] Initial rows: {len(df)}")
    print(f"[i] NaNs in 'latency': {df['latency'].isna().sum()}")
    df = df.dropna(subset=['latency'])
    print(f"[i] Rows after cleaning: {len(df)}")

    df['latency'] = (df['latency'] * 1000).astype(int)
    return df

def main():
    parser = argparse.ArgumentParser(description="Reformat throughput or block-time CSV file in-place.")
    parser.add_argument("input_file", type=str, help="Path to the input CSV file")
    parser.add_argument("--type", type=str, choices=["throughput", "block-time"], required=True,
                        help="Specify the type of the input file")
    args = parser.parse_args()

    file_path = Path(args.input_file)
    if not file_path.exists():
        print(f"Error: File not found: {args.input_file}", file=sys.stderr)
        sys.exit(1)

    df = pd.read_csv(args.input_file, header=None)

    if args.type == "throughput":
        df = process_throughput(df)
    else:
        df = process_block_time(df)

    df.to_csv(args.input_file, index=False, header=False)
    print(f"[✔] Reformatted and overwritten: {args.input_file}")

if __name__ == "__main__":
    main()
