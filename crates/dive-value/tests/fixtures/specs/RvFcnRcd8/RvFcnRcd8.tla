---- MODULE RvFcnRcd8 ----
EXTENDS Integers, TLC
ASSUME PrintT([qa |-> 1] = ("qa" :> 1))
ASSUME PrintT(<<1>> \in {("qa" :> 1 @@ "qb" :> 2)})
====
