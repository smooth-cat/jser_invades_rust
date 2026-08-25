#!/usr/bin/env python3
"""Stream GH Archive hourly files and collect repository CreateEvents."""

from __future__ import annotations

import argparse
import concurrent.futures
import gzip
import json
import os
import time
import urllib.request
from datetime import date, timedelta
from pathlib import Path


ARCHIVE_ROOT = "https://data.gharchive.org"


def hours(start: date, end: date):
    day = start
    while day <= end:
        for hour in range(24):
            yield day.isoformat(), hour
        day += timedelta(days=1)


def read_hour(item: tuple[str, int]) -> tuple[str, int, list[dict]]:
    day, hour = item
    url = f"{ARCHIVE_ROOT}/{day}-{hour}.json.gz"
    request = urllib.request.Request(
        url,
        headers={
            "Accept": "application/gzip",
            "Accept-Encoding": "identity",
            "User-Agent": "gharchive-july-analysis/1.0",
        },
    )
    repositories: dict[int, dict] = {}
    for attempt in range(4):
        try:
            with urllib.request.urlopen(request, timeout=180) as response:
                with gzip.GzipFile(fileobj=response) as stream:
                    for line in stream:
                        try:
                            event = json.loads(line)
                        except json.JSONDecodeError:
                            continue
                        if (
                            event.get("type") == "CreateEvent"
                            and event.get("payload", {}).get("ref_type") == "repository"
                        ):
                            repo = event.get("repo") or {}
                            repo_id = repo.get("id")
                            name = repo.get("name")
                            if repo_id and name:
                                repositories[int(repo_id)] = {
                                    "id": int(repo_id),
                                    "name": name,
                                    "created_at": event.get("created_at"),
                                }
            break
        except Exception:
            if attempt == 3:
                raise
            time.sleep(2**attempt)
    return day, hour, list(repositories.values())


def load_checkpoint(path: Path) -> tuple[set[str], dict[int, dict]]:
    if not path.exists():
        return set(), {}
    done: set[str] = set()
    repositories: dict[int, dict] = {}
    with path.open() as handle:
        for line in handle:
            row = json.loads(line)
            done.add(row["hour"])
            for repo in row["repositories"]:
                repositories[repo["id"]] = repo
    return done, repositories


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--start", default="2026-07-01")
    parser.add_argument("--end", default="2026-07-31")
    parser.add_argument("--checkpoint", default="/tmp/gharchive-july-checkpoint.jsonl")
    parser.add_argument("--workers", type=int, default=12)
    parser.add_argument("--limit-hours", type=int, default=0)
    args = parser.parse_args()

    checkpoint = Path(args.checkpoint)
    checkpoint.parent.mkdir(parents=True, exist_ok=True)
    done, repositories = load_checkpoint(checkpoint)
    pending = [item for item in hours(date.fromisoformat(args.start), date.fromisoformat(args.end))
               if f"{item[0]}-{item[1]}" not in done]
    if args.limit_hours:
        pending = pending[: args.limit_hours]

    started = time.time()
    with checkpoint.open("a") as output:
        with concurrent.futures.ThreadPoolExecutor(max_workers=args.workers) as pool:
            futures = {pool.submit(read_hour, item): item for item in pending}
            for index, future in enumerate(concurrent.futures.as_completed(futures), 1):
                day, hour, found = future.result()
                hour_key = f"{day}-{hour}"
                output.write(json.dumps({"hour": hour_key, "repositories": found}) + "\n")
                output.flush()
                for repo in found:
                    repositories[repo["id"]] = repo
                elapsed = max(time.time() - started, 0.001)
                print(
                    f"processed={len(done) + index}/{len(done) + len(pending)} "
                    f"hour={hour_key} repos={len(repositories)} rate={index / elapsed:.2f}/s",
                    flush=True,
                )

    output_path = checkpoint.with_suffix(".repos.json")
    ordered = sorted(repositories.values(), key=lambda item: (item.get("created_at") or "", item["id"]))
    output_path.write_text(json.dumps(ordered, ensure_ascii=False, indent=2) + "\n")
    print(f"unique_repositories={len(ordered)}")
    print(f"output={output_path}")


if __name__ == "__main__":
    main()
