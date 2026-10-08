---- MODULE ErDivZeroNested ----
EXTENDS Integers, Sequences, FiniteSets, TLC
Op(x) == x \div 0
Wrap(y) == Op(y) + 1
ASSUME Wrap(3) = 1
====
