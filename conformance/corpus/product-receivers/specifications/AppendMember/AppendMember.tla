---- MODULE AppendMember ----
EXTENDS Integers, FiniteSets, Sequences, TLC
VARIABLE x
Init == x = 0
Next == /\ x < 2 /\ x' = x + 1
Inv == LET p == ((x..(x-1)) \X (x..x))
           c == Append(<<>>, p)
       IN ~(x \in c[1])
====
