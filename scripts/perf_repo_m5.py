"""Test repository for the M5 measurements (docs/PERFORMANCE.md, "M5").

    git init -b main big100k && cd big100k
    python3 ../scripts/perf_repo_m5.py | git fast-import --quiet && git checkout main

Writes a git fast-import stream: 100 000 commits on main plus a side
branch every 5 000 commits (merged back), 50 authors, src/f<n>.txt files,
hot.txt changed by 2 000 commits, big.txt (10 000 lines) changed by 200
commits in different regions."""
import sys

N = 100_000
out = sys.stdout.buffer
mark = 0
def w(s):
    out.write(s.encode() if isinstance(s, str) else s)

def blob(data: bytes):
    global mark
    mark += 1
    w(f"blob\nmark :{mark}\ndata {len(data)}\n")
    w(data); w("\n")
    return mark

big = [f"line {i:05d} original content of the big file\n" for i in range(10_000)]
t0 = 1_600_000_000
prev = None
commit_marks = []
for i in range(N):
    files = []
    files.append((f"src/f{i % 50}.txt", blob(f"content {i}\n".encode())))
    if i % 50 == 0:
        files.append(("hot.txt", blob(f"hot change {i}\n".encode() * 3)))
    if i % 500 == 0:
        start = (i // 500 * 47) % 9_900
        for k in range(start, start + 50):
            big[k] = f"line {k:05d} changed in commit {i}\n"
        files.append(("big.txt", blob("".join(big).encode())))
    author = f"Developer {i % 50} <dev{i % 50}@example.invalid>"
    mark += 1
    msg = f"Change {i}: update module {i % 50}\n\nLonger description of change {i}.\n"
    w(f"commit refs/heads/main\nmark :{mark}\nauthor {author} {t0 + i * 60} +0000\ncommitter {author} {t0 + i * 60} +0000\n")
    w(f"data {len(msg)}\n{msg}")
    if prev is not None:
        w(f"from :{prev}\n")
    for path, b in files:
        w(f"M 100644 :{b} {path}\n")
    w("\n")
    prev = mark
    commit_marks.append(mark)
    # A side branch tip every 5 000 commits (for hide/solo).
    if i % 5000 == 2500:
        w(f"reset refs/heads/side-{i}\nfrom :{mark}\n\n")
