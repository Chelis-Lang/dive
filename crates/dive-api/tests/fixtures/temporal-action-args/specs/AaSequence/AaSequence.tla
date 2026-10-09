---- MODULE AaSequence ----
EXTENDS Naturals, Sequences
VARIABLE x
Init == x=0
Next == x'=1-x
Prefix(a,b) == Len(a)<Len(b) /\ SubSeq(b,1,Len(a))=a
P == []<><<Prefix(IF x=0 THEN <<>> ELSE <<1>>, IF x'=0 THEN <<>> ELSE <<1>>)>>_x
====
