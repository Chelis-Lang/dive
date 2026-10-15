---- MODULE InstanceEnabled ----
EXTENDS Integers
VARIABLE x
---- MODULE Inner ----
CONSTANT C
VARIABLE x
A==x'=C
Direct==ENABLED A
F==ENABLED A /\ TRUE
L==LET q==ENABLED A IN q /\ TRUE
Alias==F
Neg==~(ENABLED A /\ FALSE)
Or==FALSE \/ ENABLED A
====
I==INSTANCE Inner WITH C<-1
Init==x=0
Next==x'=1
D==I!Direct
F==I!F
L==I!L
Alias==I!Alias
Neg==I!Neg
Or==I!Or
P_D==<>[]D
P_F==<>[]F
P_L==<>[]L
P_Alias==<>[]Alias
P_Neg==<>[]Neg
P_Or==<>[]Or
====