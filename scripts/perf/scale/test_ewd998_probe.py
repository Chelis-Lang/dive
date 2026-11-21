"""Check the five-node fair token path that leaves the graph constraint."""
from ewd998 import Ring


def main():
    ring = Ring(5)
    values = {"pos": 4, "q": 0, "black": 0}
    for fields, digits in (
        (ring.active, [0] * 5),
        (ring.pending, [0] * 5),
        (ring.color, [0, 0, 0, 0, 1]),
        (ring.counter, [-12, 3, 3, 3, 3]),
    ):
        values.update(zip(fields, digits, strict=True))
    state = ring.conjunction(ring.eq(key, value) for key, value in values.items())
    assert ring.state_count(state) == 1
    assert ring.finish_probe_outside_constraint(state) == 9


if __name__ == "__main__":
    main()
