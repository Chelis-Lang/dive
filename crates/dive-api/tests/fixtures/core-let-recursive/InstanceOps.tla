---- MODULE InstanceOps ----
EXTENDS Naturals, TLC
VARIABLE x
U == INSTANCE InstanceOpsInner WITH y <- x+1
V == INSTANCE InstanceOpsInner WITH y <- x+4
I(a) == INSTANCE InstanceOpsInner WITH y <- a
E == INSTANCE InstanceOpsInner WITH y <- Print("effect",x)
Init == x=0 /\ U!A(0)=1 /\ V!M(0,7)=4
Next == x'=IF x<2 THEN x+1 ELSE x
Scoped(a) == LET RECURSIVE R(_)
                 R(n) == IF n=0 THEN a ELSE R(n-1)+1
             IN R(2)
LocalGood == Scoped(x)=x+2 /\ Scoped(x+4)=x+6
SelfGood == U!A(2)=x+3 /\ V!A(2)=x+6 /\ LocalGood
SelfOpposite == U!A(2)=x+3 /\ V!A(2)=x+3
SelfWrong == SelfGood /\ V!A(2)=6
MutualGood == U!M(2,2)=x+4 /\ V!M(2,2)=x+7
              /\ U!N(3)=x+4 /\ V!N(3)=x+7 /\ LocalGood
MutualOpposite == U!M(2,2)=x+4 /\ V!M(2,2)=x+4
MutualWrong == MutualGood /\ V!M(2,2)=7
DynamicCorrect == SelfGood /\ I(x+1)!A(2)=x+3
EffectCorrect == SelfGood /\ E!A(2)=x+2
ConstantCorrect == SelfGood /\ I(2)!A(2)=4
====
