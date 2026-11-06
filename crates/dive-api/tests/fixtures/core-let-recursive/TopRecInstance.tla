---- MODULE TopRecInstance ----
EXTENDS Naturals
VARIABLE x
A == INSTANCE TopRecState WITH y <- x+1
B == INSTANCE TopRecState WITH y <- x+2
Fixed == INSTANCE TopRecState WITH y <- 2
I(a) == INSTANCE TopRecState WITH y <- a
Init == x=0
Next == x' \in 0..3
PlainCorrect == [][A!Read=x+2]_x
Correct == [][/\ A!Read=x+2 /\ A!Read'=x'+2
              /\ B!Read=x+3 /\ B!Read'=x'+3 /\ Fixed!Read=3]_x
PrimeCorrect == [][A!Read'=x'+2 /\ B!Read'=x'+3]_x
PlainWrong == [][A!Read=1]_x
Wrong == [][A!Read=1 /\ A!Read'=x'+2]_x
ParameterCorrect == [][I(x+1)!Read=x+2]_x
====
