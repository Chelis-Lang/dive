---- MODULE ChooseIntern ----
EXTENDS Integers, TLC
ASSUME PrintT("#choose") /\ PrintT(CHOOSE e \in {"zeta", "alpha", "mid"} : TRUE)
ASSUME PrintT("#set") /\ PrintT({"zeta", "alpha", "mid"})
====
