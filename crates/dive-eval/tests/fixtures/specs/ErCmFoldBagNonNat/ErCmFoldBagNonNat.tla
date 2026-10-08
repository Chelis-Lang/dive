---- MODULE ErCmFoldBagNonNat ----
EXTENDS Integers, Sequences, FiniteSets, TLC, BagsExt
ASSUME PrintT(FoldBag(LAMBDA a, x: a, 0, (1 :> 0)))
====
