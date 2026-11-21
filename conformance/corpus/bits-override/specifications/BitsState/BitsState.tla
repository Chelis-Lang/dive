---- MODULE BitsState ----
\* Probe: Bits!And is overridden by the local Bits.class (the class of the
\* Examples Hanoi model), whose body is Java's `x & y` on two's-complement
\* 32-bit ints. The calls depend on the state, so they are evaluated during the
\* search, with negative operands. A tool that evaluates the placeholder TLA+
\* body (99) instead of the override violates Inv in the first state.
\* (Oracle fact: a non-integer argument, such as TRUE & 3, makes TLC fail with
\* error 2154, "Cannot cast tlc2.value.impl.BoolValue to
\* tlc2.value.impl.IntValue".)
EXTENDS Integers, Bits
VARIABLE x
Init == x \in {-6, -1, 0, 5}
Next == x' = (x & 7) - 3
Inv == /\ (x & (-1)) = x
       /\ ((-1) & x) = x
       /\ (x & 0) = 0
       /\ (x & 7) \in 0..7
       /\ And(x, -3, -1, 2) = x - (x & 2)
====
