from collections.abc import Set
import random

random.seed(201)
BASE_PAGE_SIZE = 4096


def random_alignment():
    # return 4096 << random.randint(1, 20)
    return random.choice([BASE_PAGE_SIZE * i for i in range(1, 4)])


def random_size(max):
    return random.randint(1, max + BASE_PAGE_SIZE)


N_REGIONS = 6
N_OPS = 200

regions = []


base = random.randint(2, 1 << 20) * BASE_PAGE_SIZE

for _ in range(N_REGIONS):
    size = random.randint(2, 128) * BASE_PAGE_SIZE
    offset = random.randint(0, 128) * BASE_PAGE_SIZE

    regions.append((base, size))

    base += size + offset

random.shuffle(regions)

free_bytes = 0

for base, size in regions:
    print(f"add {base} {base} {size}")
    free_bytes += size

allocated: dict[int, int] = {}
id = 0

for _ in range(N_OPS):
    if random.random() < 0.5 and len(allocated) > 0:
        allocation = random.choice(tuple(allocated))
        print(f"free {allocation}")
        free_bytes += allocated[allocation]
        del allocated[allocation]
        continue
    id += 1
    size = random_size(free_bytes)
    alignment = random_alignment()
    allocated[id] = size
    fail = "fail" if size > free_bytes else ""
    free_bytes -= size
    print(f"alloc {id} {size} {alignment} {fail}")
