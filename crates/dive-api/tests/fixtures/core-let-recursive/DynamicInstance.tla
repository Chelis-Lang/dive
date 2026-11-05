---- MODULE DynamicInstance ----
EXTENDS Naturals, TLC
VARIABLE x
I(a) == INSTANCE DynamicInner WITH K <- a
Fixed == INSTANCE DynamicInner WITH K <- 2
Effectful == INSTANCE DynamicInner WITH K <- Print(2,"k")
Init == x=0
Next == x'=IF x<2 THEN x+1 ELSE x
Correct == I(x+2)!Read=2*(x+2)
Opposite == I(x+2)!Read#2*(x+2)
Constant == Fixed!Read=4 /\ I(2)!Read=4
Effect == Effectful!Read=4
====
