---- MODULE AppliedOverride ----
EXTENDS Naturals,TLC
CONSTANT C(_)
VARIABLE x
Hidden==TLCSet(0,0) /\ TLCSet(0,1)
Target(n)==Hidden
Init==x=0 /\ TLCSet(0,0)
Next==C(0) /\ x'=TLCGet(0)
Inv==x=0
====
