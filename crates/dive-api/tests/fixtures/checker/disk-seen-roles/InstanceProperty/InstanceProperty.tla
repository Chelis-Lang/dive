---- MODULE InstanceProperty ----
EXTENDS Naturals, TLC
VARIABLE x
R == INSTANCE RolePart WITH v <- x
Init == x = 0
Next == x < 3 /\ x' = x + 1
P == R!P
====
