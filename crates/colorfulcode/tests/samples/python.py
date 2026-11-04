"""Fetch a few URLs concurrently and report their sizes."""
from __future__ import annotations

import asyncio
from dataclasses import dataclass, field
from typing import Iterable

MAX_WORKERS = 8


@dataclass(frozen=True)
class Result:
    url: str
    size: int = 0
    errors: list[str] = field(default_factory=list)

    @property
    def ok(self) -> bool:
        return not self.errors


class Fetcher:
    def __init__(self, timeout: float = 5.0) -> None:
        self.timeout = timeout

    async def fetch(self, url: str) -> Result:
        try:
            await asyncio.sleep(len(url) / 100)
            return Result(url, size=len(url) * 1_024)
        except Exception as exc:  # noqa: BLE001
            return Result(url, errors=[str(exc)])


async def main(urls: Iterable[str]) -> None:
    fetcher = Fetcher()
    results = await asyncio.gather(*(fetcher.fetch(u) for u in urls))
    for r in sorted(results, key=lambda r: r.size, reverse=True):
        status = "ok" if r.ok else "failed"
        print(f"{r.url:<30} {r.size / 1024:>8.1f} KiB {status!r}")
    raw = rb"\d+\n"
    print(raw, 0xFF, 3.14e-2, None, True)


if __name__ == "__main__":
    asyncio.run(main(["https://example.com", "https://example.org"]))
