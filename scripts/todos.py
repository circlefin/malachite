#!/usr/bin/env python3

import os
import re
import subprocess
from datetime import datetime

def get_git_root():
    """Get the root directory of the git repository."""
    try:
        root = subprocess.check_output(['git', 'rev-parse', '--show-toplevel'])
        return root.strip().decode('utf-8')
    except subprocess.CalledProcessError:
        print("Error: Not a git repository")
        exit(1)

def get_remote_url():
    """Get the GitHub remote URL of the repository."""
    try:
        remote = subprocess.check_output(['git', 'remote', 'get-url', 'origin'])
        remote = remote.strip().decode('utf-8')
        # Convert SSH URL to HTTPS if necessary
        if remote.startswith('git@github.com:'):
            remote = remote.replace('git@github.com:', 'https://github.com/')
        if remote.endswith('.git'):
            remote = remote[:-4]
        return remote
    except subprocess.CalledProcessError:
        print("Error: Cannot get remote URL")
        exit(1)

def get_current_commit():
    """Get the current commit hash."""
    try:
        commit = subprocess.check_output(['git', 'rev-parse', 'HEAD'])
        return commit.strip().decode('utf-8')
    except subprocess.CalledProcessError:
        print("Error: Cannot get current commit hash")
        exit(1)

def find_todos(root_dir):
    """Find all TODO and FIXME comments in the repository."""
    todos = []

    # File extensions to search
    extensions = ('.rs')

    for root, _, files in os.walk(root_dir):
        if '.git' in root:
            continue

        for file in files:
            if file.endswith(extensions):
                file_path = os.path.join(root, file)
                relative_path = os.path.relpath(file_path, root_dir)

                try:
                    with open(file_path, 'r', encoding='utf-8') as f:
                        for line_num, line in enumerate(f, 1):
                            # Search for TODO or FIXME
                            if re.search(r'\b(TODO|FIXME)\b', line, re.IGNORECASE):
                                todos.append({
                                    'file': relative_path,
                                    'line_num': line_num,
                                    'content': line.strip(),
                                    'type': 'TODO' if 'TODO' in line.upper() else 'FIXME'
                                })
                except UnicodeDecodeError:
                    continue

    return todos

def generate_markdown(todos, remote_url, commit_hash):
    """Generate markdown document from found TODOs and FIXMEs."""
    markdown = f"# TODOs and FIXMEs\n\n"
    markdown += f"Generated on: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}\n"
    markdown += f"Commit: [{commit_hash[:7]}]({remote_url}/commit/{commit_hash})\n\n"

    if not todos:
        markdown += "No TODO or FIXME items found.\n"
        return markdown

    # Separate TODOs and FIXMEs
    todos_list = [t for t in todos if t['type'] == 'TODO']
    fixmes_list = [t for t in todos if t['type'] == 'FIXME']

    if fixmes_list:
        markdown += "## FIXMEs\n\n"
        for fixme in fixmes_list:
            file_link = f"{remote_url}/blob/{commit_hash}/{fixme['file']}#L{fixme['line_num']}"
            markdown += f"- [{fixme['file']}:{fixme['line_num']}]({file_link})\n"
            markdown += f"  ```\n  {fixme['content']}\n  ```\n\n"

    if todos_list:
        markdown += "## TODOs\n\n"
        for todo in todos_list:
            file_link = f"{remote_url}/blob/{commit_hash}/{todo['file']}#L{todo['line_num']}"
            markdown += f"- [{todo['file']}:{todo['line_num']}]({file_link})\n"
            markdown += f"  ```\n  {todo['content']}\n  ```\n\n"

    return markdown

def main():
    root_dir = get_git_root()
    remote_url = get_remote_url()
    commit_hash = get_current_commit()

    todos = find_todos(root_dir)
    markdown = generate_markdown(todos, remote_url, commit_hash)

    # Write to file
    output_file = os.path.join(root_dir, 'TODO.md')
    with open(output_file, 'w', encoding='utf-8') as f:
        f.write(markdown)

    print(f"Generated TODO.md with {len(todos)} items")
    print(f"Current commit: {commit_hash[:7]}")

if __name__ == '__main__':
    main()
