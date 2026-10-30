---- MODULE TupleSubSeq ----
EXTENDS Naturals,Sequences
VARIABLE x
Init == x = <<1,2>>
Next == x'=IF Len(x)>0 THEN SubSeq(x,2,Len(x)) ELSE <<1,2>>
Spec == Init /\ [][Next]_x
P == <>[](Len(x)<=3)
====
