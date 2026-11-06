---- MODULE Inner ----
EXTENDS Naturals
CONSTANT Start(_), Step(_, _), Unused(_)
VARIABLE x
Init == Start(x)
Next == Step(x, x')
Inv == x \in 0..2
Wrong == x = 0
WrongInitially == x # 0
Spec == Init /\ [][Next]_x
====
